use std::{cmp::Ordering, collections::HashMap, rc::Rc};

use derive_builder::Builder;
use id_arena::{Arena, Id};
use squalid::OptionExt;

use super::{
    code_path_segment::CodePathSegment, code_path_state::CodePathState, fork_context::ForkContext,
    id_generator::IdGenerator,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CodePathOrigin {
    Program,
    Function,
    ClassFieldInitializer,
    ClassStaticBlock,
}

pub struct CodePath {
    id: String,
    origin: CodePathOrigin,
    upper: Option<Id<Self>>,
    child_code_paths: Vec<Id<Self>>,
    state: CodePathState,
}

impl CodePath {
    pub fn new(
        arena: &mut Arena<Self>,
        fork_context_arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        id: String,
        origin: CodePathOrigin,
        upper: Option<Id<Self>>,
        on_looped: Rc<dyn Fn(Id<CodePathSegment>, Id<CodePathSegment>)>,
    ) -> Id<Self> {
        let id_generator = Rc::new(IdGenerator::new(format!("{id}_")));
        let ret = arena.alloc(Self {
            id,
            origin,
            upper,
            child_code_paths: Default::default(),
            state: CodePathState::new(
                fork_context_arena,
                code_path_segment_arena,
                id_generator,
                on_looped,
            ),
        });
        if let Some(upper) = upper {
            arena[upper].child_code_paths.push(ret);
        }
        ret
    }

    fn get_state(&self) -> &CodePathState {
        &self.state
    }

    fn initial_segment(&self) -> Id<CodePathSegment> {
        self.state.initial_segment
    }

    fn final_segments(&self) -> &[Id<CodePathSegment>] {
        &self.state.final_segments
    }

    fn returned_segments(&self) -> &[Id<CodePathSegment>] {
        &self.state.returned_fork_context
    }

    fn thrown_segments(&self) -> &[Id<CodePathSegment>] {
        &self.state.thrown_fork_context
    }

    fn current_segments(&self) -> &[Id<CodePathSegment>] {
        &self.state.current_segments
    }

    fn traverse_segments(
        &self,
        arena: &Arena<CodePathSegment>,
        options: Option<TraverseSegmentsOptions>,
        mut callback: impl FnMut(&Self, Id<CodePathSegment>, &TraverseSegmentsController),
    ) {
        let options = options.unwrap_or_default();
        let start_segment = options.first.unwrap_or(self.state.initial_segment);
        let last_segment = options.last;

        let mut visited: HashMap<Id<CodePathSegment>, bool> = Default::default();
        let mut stack: Vec<(Id<CodePathSegment>, usize)> = vec![(start_segment, 0)];
        let mut skipped_segment: Option<Id<CodePathSegment>> = Default::default();
        let mut broken: bool = Default::default();

        while !stack.is_empty() {
            let (segment, index) = stack.last().copied().unwrap();
            if index == 0 {
                if visited.get(&segment).copied() == Some(true) {
                    stack.pop().unwrap();
                    continue;
                }

                if segment != start_segment
                    && !arena[segment].prev_segments.is_empty()
                    && !arena[segment]
                        .prev_segments
                        .iter()
                        .all(|&prev_segment| (is_visited(arena, &visited, segment, prev_segment)))
                {
                    stack.pop().unwrap();
                    continue;
                }

                if skipped_segment.matches(|skipped_segment| {
                    arena[segment].prev_segments.contains(&skipped_segment)
                }) {
                    skipped_segment = None;
                }
                visited.insert(segment, true);

                if skipped_segment.is_none() {
                    let mut controller =
                        TraverseSegmentsController::new(&mut broken, &mut skipped_segment, &stack);
                    callback(self, segment, &controller);
                    if Some(segment) == last_segment {
                        controller.skip();
                    }
                    if broken {
                        break;
                    }
                }
            }

            let end = arena[segment].next_segments.len() - 1;
            match index.cmp(&end) {
                Ordering::Less => {
                    stack.last_mut().unwrap().1 += 1;
                    stack.push((arena[segment].next_segments[index], 0));
                }
                Ordering::Equal => {
                    stack.last_mut().unwrap().0 = arena[segment].next_segments[index];
                    stack.last_mut().unwrap().1 = 0;
                }
                Ordering::Greater => {
                    stack.pop().unwrap();
                }
            }
        }
    }
}

#[derive(Builder, Default)]
#[builder(default, setter(strip_option))]
struct TraverseSegmentsOptions {
    first: Option<Id<CodePathSegment>>,
    last: Option<Id<CodePathSegment>>,
}

struct TraverseSegmentsController<'a> {
    broken: &'a mut bool,
    skipped_segment: &'a mut Option<Id<CodePathSegment>>,
    stack: &'a [(Id<CodePathSegment>, usize)],
}

impl<'a> TraverseSegmentsController<'a> {
    pub fn new(
        broken: &'a mut bool,
        skipped_segment: &'a mut Option<Id<CodePathSegment>>,
        stack: &'a [(Id<CodePathSegment>, usize)],
    ) -> Self {
        Self {
            broken,
            skipped_segment,
            stack,
        }
    }

    pub fn skip(&mut self) {
        if self.stack.len() <= 1 {
            *self.broken = true;
        } else {
            *self.skipped_segment = Some(self.stack[self.stack.len() - 2].0);
        }
    }

    pub fn break_(&mut self) {
        *self.broken = true;
    }
}

fn is_visited(
    arena: &Arena<CodePathSegment>,
    visited: &HashMap<Id<CodePathSegment>, bool>,
    segment: Id<CodePathSegment>,
    prev_segment: Id<CodePathSegment>,
) -> bool {
    visited.get(&prev_segment).copied() == Some(true)
        || arena[segment].is_looped_prev_segment(prev_segment)
}
