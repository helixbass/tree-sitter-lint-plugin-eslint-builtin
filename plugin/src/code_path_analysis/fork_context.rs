use std::rc::Rc;

use id_arena::{Arena, Id};
use itertools::Itertools;
use squalid::{OptionExt, VecExt};

use super::{code_path_segment::CodePathSegment, id_generator::IdGenerator};

#[derive(Debug)]
pub struct ForkContext<'a> {
    id_generator: Rc<IdGenerator>,
    pub upper: Option<Id<Self>>,
    pub split_depth: usize,
    pub segments_list: Vec<Rc<SingleOrSplitSegment<'a>>>,
}

impl<'a> ForkContext<'a> {
    pub fn new(
        arena: &mut Arena<Self>,
        id_generator: Rc<IdGenerator>,
        upper: Option<Id<Self>>,
        split_depth: usize,
    ) -> Id<Self> {
        arena.alloc(Self {
            id_generator,
            upper,
            split_depth,
            segments_list: Default::default(),
        })
    }

    pub fn maybe_head(&self) -> Option<Rc<SingleOrSplitSegment<'a>>> {
        self.segments_list.last().cloned()
    }

    pub fn head(&self) -> Rc<SingleOrSplitSegment<'a>> {
        self.segments_list.last().cloned().unwrap()
    }

    pub fn empty(&self) -> bool {
        self.segments_list.is_empty()
    }

    pub fn reachable(&self, arena: &Arena<CodePathSegment<'a>>) -> bool {
        self.maybe_head().matches(|head| head.reachable(arena))
    }

    fn make_segments(
        &self,
        begin: isize,
        end: isize,
        mut create: impl FnMut(String, &[Id<CodePathSegment<'a>>]) -> Id<CodePathSegment<'a>>,
    ) -> SingleOrSplitSegment<'a> {
        let list = &self.segments_list;

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

        SingleOrSplitSegment::reduce(list, self.split_depth, &mut |segments| {
            create(
                self.id_generator.next(),
                &segments[normalized_begin..=normalized_end],
            )
        })
    }

    #[allow(non_snake_case)]
    fn make_segments__missing_begin_end(
        &self,
        mut create: impl FnMut(String, &[Id<CodePathSegment<'a>>]) -> Id<CodePathSegment<'a>>,
    ) -> SingleOrSplitSegment<'a> {
        SingleOrSplitSegment::reduce(&self.segments_list, self.split_depth, &mut |_| {
            create(self.id_generator.next(), &[])
        })
    }

    pub fn make_next(
        &self,
        arena: &mut Arena<CodePathSegment<'a>>,
        begin: isize,
        end: isize,
    ) -> Rc<SingleOrSplitSegment<'a>> {
        Rc::new(self.make_segments(
            begin,
            end,
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_next(arena, id, all_prev_segments)
            },
        ))
    }

    pub fn make_unreachable(
        &self,
        arena: &mut Arena<CodePathSegment<'a>>,
        begin: isize,
        end: isize,
    ) -> Rc<SingleOrSplitSegment<'a>> {
        Rc::new(self.make_segments(
            begin,
            end,
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_unreachable(arena, id, all_prev_segments)
            },
        ))
    }

    #[allow(non_snake_case)]
    pub fn make_unreachable__missing_begin_end(
        &self,
        arena: &mut Arena<CodePathSegment<'a>>,
    ) -> Rc<SingleOrSplitSegment<'a>> {
        Rc::new(self.make_segments__missing_begin_end(
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_unreachable(arena, id, all_prev_segments)
            },
        ))
    }

    pub fn make_disconnected(
        &self,
        arena: &mut Arena<CodePathSegment<'a>>,
        begin: isize,
        end: isize,
    ) -> Rc<SingleOrSplitSegment<'a>> {
        Rc::new(self.make_segments(
            begin,
            end,
            |id: String, all_prev_segments: &[Id<CodePathSegment>]| {
                CodePathSegment::new_disconnected(arena, id, all_prev_segments)
            },
        ))
    }

    fn merge_extra_segments(
        &self,
        arena: &mut Arena<CodePathSegment<'a>>,
        segments: Rc<SingleOrSplitSegment<'a>>,
    ) -> Rc<SingleOrSplitSegment<'a>> {
        if segments.split_depth() == self.split_depth {
            return segments;
        }

        let mut current_segments = segments;
        while current_segments.split_depth() > self.split_depth {
            current_segments = current_segments.unsplit(&mut |a, b| {
                CodePathSegment::new_next(arena, self.id_generator.next(), &[a, b])
            });
        }
        current_segments
    }

    pub fn add(
        &mut self,
        arena: &mut Arena<CodePathSegment<'a>>,
        segments: Rc<SingleOrSplitSegment<'a>>,
    ) {
        assert!(
            segments.split_depth() >= self.split_depth,
            "{} >= {}",
            segments.split_depth(),
            self.split_depth
        );

        self.segments_list
            .push(self.merge_extra_segments(arena, segments));
    }

    pub fn replace_head(
        &mut self,
        arena: &mut Arena<CodePathSegment<'a>>,
        segments: Rc<SingleOrSplitSegment<'a>>,
    ) {
        assert!(
            segments.split_depth() >= self.split_depth,
            "{} >= {}",
            segments.split_depth(),
            self.split_depth,
        );

        *self.segments_list.last_mut().unwrap() = self.merge_extra_segments(arena, segments);
    }

    pub fn add_all(arena: &mut Arena<Self>, self_: Id<Self>, context: Id<Self>) {
        assert!(arena[context].split_depth == arena[self_].split_depth);

        let source = arena[context].segments_list.clone();

        for source_item in source {
            arena[self_].segments_list.push(source_item);
        }
    }

    pub fn clear(&mut self) {
        self.segments_list.clear();
    }

    pub fn new_root(
        arena: &mut Arena<Self>,
        code_path_segment_arena: &mut Arena<CodePathSegment<'a>>,
        id_generator: Rc<IdGenerator>,
    ) -> Id<Self> {
        let context = Self::new(arena, id_generator.clone(), None, 0);

        let segment = CodePathSegment::new_root(code_path_segment_arena, id_generator.next());
        arena[context].add(
            code_path_segment_arena,
            Rc::new(SingleOrSplitSegment::Single(segment)),
        );

        context
    }

    pub fn new_empty(
        arena: &mut Arena<Self>,
        parent_context: Id<Self>,
        fork_leaving_path: Option<bool>,
    ) -> Id<Self> {
        let id_generator = arena[parent_context].id_generator.clone();
        let parent_context_split_depth = arena[parent_context].split_depth;
        Self::new(
            arena,
            id_generator,
            Some(parent_context),
            if fork_leaving_path.unwrap_or_default() {
                parent_context_split_depth + 1
            } else {
                parent_context_split_depth
            },
        )
    }
}

#[derive(Clone, Debug)]
pub enum SingleOrSplitSegment<'a> {
    Single(Id<CodePathSegment<'a>>),
    Split(SplitSegment<'a>),
}

impl<'a> SingleOrSplitSegment<'a> {
    pub fn split_depth(&self) -> usize {
        match self {
            SingleOrSplitSegment::Single(_) => 0,
            SingleOrSplitSegment::Split(split_segment) => split_segment.split_depth,
        }
    }

    pub fn reduce(
        // list: impl IntoIterator<Item = &'b Self>,
        list: &[Rc<Self>],
        split_depth: usize,
        create: &mut dyn FnMut(&[Id<CodePathSegment<'a>>]) -> Id<CodePathSegment<'a>>,
    ) -> Self {
        if split_depth == 0 {
            Self::Single(create(
                &list
                    .into_iter()
                    .map(|item| match &**item {
                        SingleOrSplitSegment::Single(item) => *item,
                        SingleOrSplitSegment::Split(_) => panic!("Not of expected split depth"),
                    })
                    .collect_vec(),
            ))
        } else {
            Self::Split(SplitSegment::new(
                Rc::new(Self::reduce(
                    &list
                        .into_iter()
                        .map(|item| match &**item {
                            SingleOrSplitSegment::Split(split_segment) => {
                                split_segment.segments.0.clone()
                            }
                            SingleOrSplitSegment::Single(_) => {
                                panic!("Not of expected split depth")
                            }
                        })
                        .collect_vec(),
                    split_depth - 1,
                    create,
                )),
                Rc::new(Self::reduce(
                    &list
                        .into_iter()
                        .map(|item| match &**item {
                            SingleOrSplitSegment::Split(split_segment) => {
                                split_segment.segments.1.clone()
                            }
                            SingleOrSplitSegment::Single(_) => {
                                panic!("Not of expected split depth")
                            }
                        })
                        .collect_vec(),
                    split_depth - 1,
                    create,
                )),
            ))
        }
    }

    pub fn unsplit(
        self: Rc<Self>,
        merge: &mut dyn FnMut(
            Id<CodePathSegment<'a>>,
            Id<CodePathSegment<'a>>,
        ) -> Id<CodePathSegment<'a>>,
    ) -> Rc<Self> {
        match &*self {
            SingleOrSplitSegment::Single(_) => self.clone(),
            SingleOrSplitSegment::Split(split_segment) => Rc::new(split_segment.unsplit(merge)),
        }
    }

    pub fn reachable(&self, arena: &Arena<CodePathSegment<'a>>) -> bool {
        match self {
            SingleOrSplitSegment::Single(segment) => arena[*segment].reachable,
            SingleOrSplitSegment::Split(split_segment) => split_segment.reachable(arena),
        }
    }

    pub fn segments(&self) -> Vec<Id<CodePathSegment<'a>>> {
        match self {
            SingleOrSplitSegment::Single(segment) => {
                vec![*segment]
            }
            SingleOrSplitSegment::Split(split_segment) => split_segment.segments(),
        }
    }

    pub fn map(
        &self,
        mapper: &mut dyn FnMut(Id<CodePathSegment<'a>>) -> Id<CodePathSegment<'a>>,
    ) -> Self {
        match self {
            SingleOrSplitSegment::Single(segment) => Self::Single(mapper(*segment)),
            SingleOrSplitSegment::Split(split_segment) => Self::Split(SplitSegment::new(
                Rc::new(split_segment.segments.0.map(mapper)),
                Rc::new(split_segment.segments.1.map(mapper)),
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SplitSegment<'a> {
    split_depth: usize,
    pub segments: (Rc<SingleOrSplitSegment<'a>>, Rc<SingleOrSplitSegment<'a>>),
}

impl<'a> SplitSegment<'a> {
    pub fn new(left: Rc<SingleOrSplitSegment<'a>>, right: Rc<SingleOrSplitSegment<'a>>) -> Self {
        assert!(left.split_depth() == right.split_depth());
        Self {
            split_depth: left.split_depth() + 1,
            segments: (left, right),
        }
    }

    pub fn unsplit(
        &self,
        merge: &mut dyn FnMut(
            Id<CodePathSegment<'a>>,
            Id<CodePathSegment<'a>>,
        ) -> Id<CodePathSegment<'a>>,
    ) -> SingleOrSplitSegment<'a> {
        match self.split_depth {
            1 => SingleOrSplitSegment::Single(merge(
                match &*self.segments.0 {
                    SingleOrSplitSegment::Single(segment) => *segment,
                    SingleOrSplitSegment::Split(_) => unreachable!(),
                },
                match &*self.segments.1 {
                    SingleOrSplitSegment::Single(segment) => *segment,
                    SingleOrSplitSegment::Split(_) => unreachable!(),
                },
            )),
            _ => SingleOrSplitSegment::Split(SplitSegment::new(
                Rc::new(
                    Self::new(
                        match &*self.segments.0 {
                            SingleOrSplitSegment::Split(split_segment) => {
                                split_segment.segments.0.clone()
                            }
                            SingleOrSplitSegment::Single(_) => unreachable!(),
                        },
                        match &*self.segments.1 {
                            SingleOrSplitSegment::Split(split_segment) => {
                                split_segment.segments.0.clone()
                            }
                            SingleOrSplitSegment::Single(_) => unreachable!(),
                        },
                    )
                    .unsplit(merge),
                ),
                Rc::new(
                    Self::new(
                        match &*self.segments.0 {
                            SingleOrSplitSegment::Split(split_segment) => {
                                split_segment.segments.1.clone()
                            }
                            SingleOrSplitSegment::Single(_) => unreachable!(),
                        },
                        match &*self.segments.1 {
                            SingleOrSplitSegment::Split(split_segment) => {
                                split_segment.segments.1.clone()
                            }
                            SingleOrSplitSegment::Single(_) => unreachable!(),
                        },
                    )
                    .unsplit(merge),
                ),
            )),
        }
    }

    pub fn reachable(&self, arena: &Arena<CodePathSegment<'a>>) -> bool {
        self.segments.0.reachable(arena) || self.segments.1.reachable(arena)
    }

    pub fn segments(&self) -> Vec<Id<CodePathSegment<'a>>> {
        self.segments
            .0
            .segments()
            .and_extend(self.segments.1.segments())
    }
}
