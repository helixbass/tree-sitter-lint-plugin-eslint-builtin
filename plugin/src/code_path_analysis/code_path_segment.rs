use std::collections::HashSet;

use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

#[derive(Debug)]
pub struct CodePathSegment<'a> {
    // TODO: can I just use the id_arena::Id for this?
    pub id: String,
    pub next_segments: Vec<Id<Self>>,
    pub prev_segments: Vec<Id<Self>>,
    pub all_next_segments: Vec<Id<Self>>,
    pub all_prev_segments: Vec<Id<Self>>,
    pub reachable: bool,
    used: bool,
    looped_prev_segments: Vec<Id<Self>>,
    pub nodes: Vec<(EnterOrExit, Node<'a>)>,
}

impl<'a> CodePathSegment<'a> {
    pub fn new(
        arena: &mut Arena<Self>,
        id: String,
        all_prev_segments: Vec<Id<Self>>,
        reachable: bool,
    ) -> Id<Self> {
        let segment = Self {
            id,
            next_segments: Default::default(),
            prev_segments: all_prev_segments
                .iter()
                .filter(|segment| arena.get(**segment).unwrap().reachable)
                .copied()
                .collect(),
            all_next_segments: Default::default(),
            all_prev_segments,
            reachable,
            used: Default::default(),
            looped_prev_segments: Default::default(),
            nodes: Default::default(),
        };

        arena.alloc(segment)
    }

    pub fn is_looped_prev_segment(&self, segment: Id<Self>) -> bool {
        self.looped_prev_segments.contains(&segment)
    }

    pub fn new_root(arena: &mut Arena<Self>, id: String) -> Id<Self> {
        Self::new(arena, id, Default::default(), true)
    }

    pub fn new_next(
        arena: &mut Arena<Self>,
        id: String,
        all_prev_segments: &[Id<Self>],
    ) -> Id<Self> {
        let reachable = all_prev_segments
            .into_iter()
            .any(|segment| arena.get(*segment).unwrap().reachable);
        Self::new(
            arena,
            id,
            Self::flatten_unused_segments(arena, all_prev_segments),
            reachable,
        )
    }

    pub fn new_unreachable(
        arena: &mut Arena<Self>,
        id: String,
        all_prev_segments: &[Id<Self>],
    ) -> Id<Self> {
        let segment = Self::new(
            arena,
            id,
            Self::flatten_unused_segments(arena, all_prev_segments),
            false,
        );

        Self::mark_used(arena, segment);

        segment
    }

    pub fn new_disconnected(
        arena: &mut Arena<Self>,
        id: String,
        all_prev_segments: &[Id<Self>],
    ) -> Id<Self> {
        let reachable = all_prev_segments
            .into_iter()
            .any(|segment| arena.get(*segment).unwrap().reachable);
        Self::new(arena, id, Default::default(), reachable)
    }

    pub fn mark_used(arena: &mut Arena<Self>, segment: Id<Self>) {
        if arena[segment].used {
            return;
        }
        arena[segment].used = true;

        if arena[segment].reachable {
            for prev_segment in arena[segment].all_prev_segments.clone() {
                let prev_segment_value = &mut arena[prev_segment];
                prev_segment_value.all_next_segments.push(segment);
                prev_segment_value.next_segments.push(segment);
            }
        } else {
            for prev_segment in arena[segment].all_prev_segments.clone() {
                arena[prev_segment].all_next_segments.push(segment);
            }
        }
    }

    pub fn mark_prev_segment_as_looped(
        arena: &mut Arena<Self>,
        segment: Id<Self>,
        prev_segment: Id<Self>,
    ) {
        arena[segment].looped_prev_segments.push(prev_segment);
    }

    pub fn flatten_unused_segments(arena: &Arena<Self>, segments: &[Id<Self>]) -> Vec<Id<Self>> {
        let mut done: HashSet<Id<Self>> = Default::default();
        let mut retv: Vec<Id<Self>> = Default::default();

        for &segment in segments {
            if done.contains(&segment) {
                continue;
            }

            let segment_value = &arena[segment];

            if !segment_value.used {
                for prev_segment in &segment_value.all_prev_segments {
                    if !done.contains(prev_segment) {
                        done.insert(*prev_segment);
                        retv.push(*prev_segment);
                    }
                }
            } else {
                done.insert(segment);
                retv.push(segment);
            }
        }

        retv
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EnterOrExit {
    Enter,
    Exit,
}
