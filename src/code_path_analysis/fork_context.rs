use std::rc::Rc;

use id_arena::{Arena, Id};
use squalid::OptionExt;

use super::{code_path_segment::CodePathSegment, id_generator::IdGenerator};

fn make_segments(
    context: &ForkContext,
    begin: isize,
    end: isize,
    mut create: impl FnMut(String, &[Id<CodePathSegment>]) -> Id<CodePathSegment>,
) -> Vec<Id<CodePathSegment>> {
    let list = &context.segments_list;

    let normalized_begin = if begin >= 0 {
        begin as usize
    } else {
        ((list.len() as isize) + begin) as usize
    };
    let normalized_end = if end >= 0 {
        end as usize
    } else {
        ((list.len() as isize) + end) as usize
    };

    (0..context.count)
        .map(|i| {
            let all_prev_segments = (normalized_begin..=normalized_end)
                .map(|j| list[j][i])
                .collect::<Vec<_>>();

            create(context.id_generator.next(), &all_prev_segments)
        })
        .collect()
}

fn make_segments__missing_begin_end(
    context: &ForkContext,
    mut create: impl FnMut(String, &[Id<CodePathSegment>]) -> Id<CodePathSegment>,
) -> Vec<Id<CodePathSegment>> {
    (0..context.count)
        .map(|_| create(context.id_generator.next(), &[]))
        .collect()
}

fn merge_extra_segments(
    arena: &mut Arena<CodePathSegment>,
    context: &ForkContext,
    segments: Vec<Id<CodePathSegment>>,
) -> Vec<Id<CodePathSegment>> {
    let mut current_segments = segments;

    while current_segments.len() > context.count {
        let length = current_segments.len() / 2;
        let merged = (0..length)
            .map(|i| {
                CodePathSegment::new_next(
                    arena,
                    context.id_generator.next(),
                    &[current_segments[i], current_segments[i + length]],
                )
            })
            .collect::<Vec<_>>();
        current_segments = merged;
    }
    current_segments
}

pub struct ForkContext {
    id_generator: Rc<IdGenerator>,
    pub upper: Option<Id<Self>>,
    pub count: usize,
    pub segments_list: Vec<Vec<Id<CodePathSegment>>>,
}

impl ForkContext {
    pub fn new(
        arena: &mut Arena<Self>,
        id_generator: Rc<IdGenerator>,
        upper: Option<Id<Self>>,
        count: usize,
    ) -> Id<Self> {
        arena.alloc(Self {
            id_generator,
            upper,
            count,
            segments_list: Default::default(),
        })
    }

    fn maybe_head(&self) -> Option<&Vec<Id<CodePathSegment>>> {
        self.segments_list.last()
    }

    pub fn head(&self) -> &Vec<Id<CodePathSegment>> {
        self.maybe_head().unwrap()
    }

    pub fn empty(&self) -> bool {
        self.segments_list.is_empty()
    }

    pub fn reachable(&self, arena: &Arena<CodePathSegment>) -> bool {
        self.maybe_head().matches(|head| {
            !head.is_empty()
                && head
                    .into_iter()
                    .any(|&segment| arena.get(segment).unwrap().reachable)
        })
    }

    pub fn make_next(
        &self,
        arena: &mut Arena<CodePathSegment>,
        begin: isize,
        end: isize,
    ) -> Vec<Id<CodePathSegment>> {
        make_segments(
            self,
            begin,
            end,
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_next(arena, id, all_prev_segments)
            },
        )
    }

    pub fn make_unreachable(
        &self,
        arena: &mut Arena<CodePathSegment>,
        begin: isize,
        end: isize,
    ) -> Vec<Id<CodePathSegment>> {
        make_segments(
            self,
            begin,
            end,
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_unreachable(arena, id, all_prev_segments)
            },
        )
    }

    pub fn make_unreachable__missing_begin_end(
        &self,
        arena: &mut Arena<CodePathSegment>,
    ) -> Vec<Id<CodePathSegment>> {
        make_segments__missing_begin_end(
            self,
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_unreachable(arena, id, all_prev_segments)
            },
        )
    }

    pub fn make_disconnected(
        &self,
        arena: &mut Arena<CodePathSegment>,
        begin: isize,
        end: isize,
    ) -> Vec<Id<CodePathSegment>> {
        make_segments(
            self,
            begin,
            end,
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_disconnected(arena, id, all_prev_segments)
            },
        )
    }

    pub fn add(&mut self, arena: &mut Arena<CodePathSegment>, segments: Vec<Id<CodePathSegment>>) {
        assert!(
            segments.len() >= self.count,
            "{} >= {}",
            segments.len(),
            self.count
        );

        self.segments_list
            .push(merge_extra_segments(arena, self, segments));
    }

    pub fn replace_head(
        &mut self,
        arena: &mut Arena<CodePathSegment>,
        segments: Vec<Id<CodePathSegment>>,
    ) {
        assert!(
            segments.len() >= self.count,
            "{} >= {}",
            segments.len(),
            self.count,
        );

        self.segments_list.splice(
            self.segments_list.len() - 1..,
            [merge_extra_segments(arena, self, segments)],
        );
    }

    pub fn add_all(arena: &mut Arena<Self>, self_: Id<Self>, context: Id<Self>) {
        assert!(arena.get(context).unwrap().count == arena.get(self_).unwrap().count);

        let source = arena.get(context).unwrap().segments_list.clone();

        let self_value = arena.get_mut(self_).unwrap();
        for source_item in source {
            self_value.segments_list.push(source_item);
        }
    }

    pub fn clear(&mut self) {
        self.segments_list.clear();
    }

    pub fn new_root(
        arena: &mut Arena<Self>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        id_generator: Rc<IdGenerator>,
    ) -> Id<Self> {
        let context = Self::new(arena, id_generator.clone(), None, 1);

        let segment = CodePathSegment::new_root(code_path_segment_arena, id_generator.next());
        arena
            .get_mut(context)
            .unwrap()
            .add(code_path_segment_arena, vec![segment]);

        context
    }

    pub fn new_empty(
        arena: &mut Arena<Self>,
        parent_context: Id<Self>,
        fork_leaving_path: Option<bool>,
    ) -> Id<Self> {
        let id_generator = arena.get(parent_context).unwrap().id_generator.clone();
        let parent_context_count = arena.get(parent_context).unwrap().count;
        Self::new(
            arena,
            id_generator,
            Some(parent_context),
            if fork_leaving_path.unwrap_or_default() {
                2
            } else {
                1
            } * parent_context_count,
        )
    }
}
