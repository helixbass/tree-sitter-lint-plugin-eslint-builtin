use id_arena::{Arena, Id};
use std::rc::Rc;

use crate::kind::Kind;

use super::{
    code_path_segment::CodePathSegment, fork_context::ForkContext, id_generator::IdGenerator,
};

fn add_to_returned_or_thrown(
    dest: &mut Vec<Id<CodePathSegment>>,
    others: &[Id<CodePathSegment>],
    all: &mut Vec<Id<CodePathSegment>>,
    segments: &[Id<CodePathSegment>],
) {
    for &segment in segments {
        dest.push(segment);
        if !others.contains(&segment) {
            all.push(segment);
        }
    }
}

pub struct CodePathState {
    id_generator: Rc<IdGenerator>,
    notify_looped: Rc<dyn Fn(Id<CodePathSegment>, Id<CodePathSegment>)>,
    fork_context: Id<ForkContext>,
    choice_context: Option<ChoiceContext>,
    switch_context: Option<SwitchContext>,
    try_context: Option<TryContext>,
    loop_context: Option<LoopContext>,
    break_context: Option<BreakContext>,
    chain_context: Option<ChainContext>,
    current_segments: Vec<Id<CodePathSegment>>,
    initial_segment: Id<CodePathSegment>,
    final_segments: Vec<Id<CodePathSegment>>,
    returned_fork_context: Vec<Id<CodePathSegment>>,
    thrown_fork_context: Vec<Id<CodePathSegment>>,
}

impl CodePathState {
    pub fn new(
        fork_context_arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        id_generator: Rc<IdGenerator>,
        on_looped: Rc<dyn Fn(Id<CodePathSegment>, Id<CodePathSegment>)>,
    ) -> Self {
        let fork_context = ForkContext::new_root(
            fork_context_arena,
            code_path_segment_arena,
            id_generator.clone(),
        );
        let initial_segment = fork_context_arena.get(fork_context).unwrap().head()[0];
        Self {
            id_generator,
            notify_looped: on_looped,
            fork_context,
            choice_context: Default::default(),
            switch_context: Default::default(),
            try_context: Default::default(),
            loop_context: Default::default(),
            break_context: Default::default(),
            chain_context: Default::default(),
            current_segments: Default::default(),
            initial_segment,
            final_segments: Default::default(),
            returned_fork_context: Default::default(),
            thrown_fork_context: Default::default(),
        }
    }

    fn returned_fork_context_add(&mut self, segments: &[Id<CodePathSegment>]) {
        add_to_returned_or_thrown(
            &mut self.returned_fork_context,
            &self.thrown_fork_context,
            &mut self.final_segments,
            segments,
        );
    }

    fn thrown_fork_context_add(&mut self, segments: &[Id<CodePathSegment>]) {
        add_to_returned_or_thrown(
            &mut self.thrown_fork_context,
            &self.returned_fork_context,
            &mut self.final_segments,
            segments,
        );
    }

    fn head_segments<'a>(&self, arena: &'a Arena<ForkContext>) -> &'a [Id<CodePathSegment>] {
        arena.get(self.fork_context).unwrap().head()
    }

    fn maybe_parent_fork_context(&self, arena: &Arena<ForkContext>) -> Option<Id<ForkContext>> {
        let current = self.fork_context;

        /*current &&*/
        arena.get(current).unwrap().upper
    }

    fn parent_fork_context(&self, arena: &Arena<ForkContext>) -> Id<ForkContext> {
        self.maybe_parent_fork_context(arena).unwrap()
    }

    fn push_fork_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        fork_leaving_path: Option<bool>,
    ) -> Id<ForkContext> {
        self.fork_context = ForkContext::new_empty(arena, self.fork_context, fork_leaving_path);

        self.fork_context
    }

    fn pop_fork_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) -> Id<ForkContext> {
        let last_context = self.fork_context;

        self.fork_context = arena.get(last_context).unwrap().upper.unwrap();
        let segments = arena
            .get(last_context)
            .unwrap()
            .make_next(code_path_segment_arena, 0, -1);
        arena
            .get_mut(self.fork_context)
            .unwrap()
            .replace_head(code_path_segment_arena, segments);

        last_context
    }

    fn fork_path(
        &self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let segments = arena
            .get(self.parent_fork_context(arena))
            .unwrap()
            .make_next(code_path_segment_arena, -1, -1);

        arena
            .get_mut(self.fork_context)
            .unwrap()
            .add(code_path_segment_arena, segments)
    }

    fn fork_bypass_path(
        &self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let segments = arena
            .get(self.parent_fork_context(arena))
            .unwrap()
            .head()
            .clone();

        arena
            .get_mut(self.fork_context)
            .unwrap()
            .add(code_path_segment_arena, segments)
    }

    fn push_choice_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        kind: ChoiceContextKind,
        is_forking_as_result: bool,
    ) {
        self.choice_context = Some(ChoiceContext {
            upper: self.choice_context.take().map(Box::new),
            kind,
            is_forking_as_result,
            true_fork_context: ForkContext::new_empty(arena, self.fork_context, None),
            false_fork_context: ForkContext::new_empty(arena, self.fork_context, None),
            qq_fork_context: ForkContext::new_empty(arena, self.fork_context, None),
            processed: Default::default(),
        });
    }

    fn pop_choice_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) -> ChoiceContext {
        let mut context = self.choice_context.take().unwrap();

        self.choice_context = context.upper.take().map(|box_| *box_);

        let fork_context = self.fork_context;
        let head_segments = arena.get(fork_context).unwrap().head().clone();

        match context.kind {
            ChoiceContextKind::LogicalAnd
            | ChoiceContextKind::LogicalOr
            | ChoiceContextKind::LogicalNullCoalesce => {
                if !context.processed {
                    arena
                        .get_mut(context.true_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, head_segments.clone());
                    arena
                        .get_mut(context.false_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, head_segments.clone());
                    arena
                        .get_mut(context.qq_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, head_segments);
                }

                if context.is_forking_as_result {
                    let parent_context = self.choice_context.as_mut().unwrap();

                    ForkContext::add_all(
                        arena,
                        parent_context.true_fork_context,
                        context.true_fork_context,
                    );
                    ForkContext::add_all(
                        arena,
                        parent_context.false_fork_context,
                        context.false_fork_context,
                    );
                    ForkContext::add_all(
                        arena,
                        parent_context.qq_fork_context,
                        context.qq_fork_context,
                    );
                    parent_context.processed = true;

                    return context;
                }
            }
            ChoiceContextKind::Test => {
                if !context.processed {
                    arena.get_mut(context.true_fork_context).unwrap().clear();
                    arena
                        .get_mut(context.true_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, head_segments);
                } else {
                    arena.get_mut(context.false_fork_context).unwrap().clear();
                    arena
                        .get_mut(context.false_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, head_segments);
                }
            }
            ChoiceContextKind::Loop => return context,
        }

        let prev_fork_context = context.true_fork_context;

        ForkContext::add_all(arena, prev_fork_context, context.false_fork_context);
        let segments =
            arena
                .get(prev_fork_context)
                .unwrap()
                .make_next(code_path_segment_arena, 0, -1);
        arena
            .get_mut(fork_context)
            .unwrap()
            .replace_head(code_path_segment_arena, segments);

        context
    }

    fn make_logical_right(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self.choice_context.as_mut().unwrap();
        let fork_context = self.fork_context;

        if context.processed {
            let prev_fork_context = match context.kind {
                ChoiceContextKind::LogicalAnd => context.true_fork_context,
                ChoiceContextKind::LogicalOr => context.false_fork_context,
                ChoiceContextKind::LogicalNullCoalesce => context.qq_fork_context,
                _ => unreachable!(),
            };

            let segments =
                arena
                    .get(prev_fork_context)
                    .unwrap()
                    .make_next(code_path_segment_arena, 0, -1);
            arena
                .get_mut(fork_context)
                .unwrap()
                .replace_head(code_path_segment_arena, segments);
            arena.get_mut(fork_context).unwrap().clear();
            context.processed = false;
        } else {
            match context.kind {
                ChoiceContextKind::LogicalAnd => {
                    let segments = arena.get(fork_context).unwrap().head().clone();
                    arena
                        .get_mut(context.false_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, segments);
                }
                ChoiceContextKind::LogicalOr => {
                    let segments = arena.get(fork_context).unwrap().head().clone();
                    arena
                        .get_mut(context.true_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, segments);
                }
                ChoiceContextKind::LogicalNullCoalesce => {
                    let segments = arena.get(fork_context).unwrap().head().clone();
                    arena
                        .get_mut(context.true_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, segments.clone());
                    arena
                        .get_mut(context.false_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, segments);
                }
                _ => unreachable!(),
            }

            let segments =
                arena
                    .get(fork_context)
                    .unwrap()
                    .make_next(code_path_segment_arena, -1, -1);
            arena
                .get_mut(fork_context)
                .unwrap()
                .replace_head(code_path_segment_arena, segments);
        }
    }

    fn make_if_consequent(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self.choice_context.as_mut().unwrap();
        let fork_context = self.fork_context;

        if !context.processed {
            let segments = arena.get(fork_context).unwrap().head().clone();
            arena
                .get_mut(context.true_fork_context)
                .unwrap()
                .add(code_path_segment_arena, segments.clone());
            arena
                .get_mut(context.false_fork_context)
                .unwrap()
                .add(code_path_segment_arena, segments.clone());
            arena
                .get_mut(context.qq_fork_context)
                .unwrap()
                .add(code_path_segment_arena, segments);
        }

        context.processed = false;

        let segments =
            arena
                .get(context.true_fork_context)
                .unwrap()
                .make_next(code_path_segment_arena, 0, -1);
        arena
            .get_mut(fork_context)
            .unwrap()
            .replace_head(code_path_segment_arena, segments);
    }

    fn make_if_alternate(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self.choice_context.as_mut().unwrap();
        let fork_context = self.fork_context;

        arena.get_mut(context.true_fork_context).unwrap().clear();
        let segments = arena.get(fork_context).unwrap().head().clone();
        arena
            .get_mut(context.true_fork_context)
            .unwrap()
            .add(code_path_segment_arena, segments);
        context.processed = true;

        let segments = arena.get(context.false_fork_context).unwrap().make_next(
            code_path_segment_arena,
            0,
            -1,
        );
        arena
            .get_mut(fork_context)
            .unwrap()
            .replace_head(code_path_segment_arena, segments);
    }

    fn push_chain_context(&mut self) {
        self.chain_context = Some(ChainContext {
            upper: self.chain_context.take().map(Box::new),
            count_choice_contexts: Default::default(),
        });
    }

    fn pop_chain_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let mut context = self.chain_context.take().unwrap();

        self.chain_context = context.upper.take().map(|box_| *box_);

        for _ in 0..context.count_choice_contexts {
            self.pop_choice_context(arena, code_path_segment_arena);
        }
    }

    fn make_optional_node(&mut self, arena: &mut Arena<ForkContext>) {
        if let Some(chain_context) = self.chain_context.as_mut() {
            chain_context.count_choice_contexts += 1;
            self.push_choice_context(arena, ChoiceContextKind::LogicalNullCoalesce, false);
        }
    }

    fn make_optional_right(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        if self.chain_context.is_some() {
            self.make_logical_right(arena, code_path_segment_arena);
        }
    }

    fn push_switch_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        has_case: bool,
        label: Option<String>,
    ) {
        self.switch_context = Some(SwitchContext {
            upper: self.switch_context.take().map(Box::new),
            has_case,
            default_segments: Default::default(),
            default_body_segments: Default::default(),
            found_default: Default::default(),
            last_is_default: Default::default(),
            count_forks: Default::default(),
        });

        self.push_break_context(arena, true, label);
    }

    fn pop_switch_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let mut context = self.switch_context.take().unwrap();

        self.switch_context = context.upper.take().map(|box_| *box_);

        let fork_context = self.fork_context;
        let broken_fork_context = self
            .pop_break_context(arena, code_path_segment_arena)
            .broken_fork_context;

        unimplemented!()
    }

    fn push_try_context(&mut self, arena: &mut Arena<ForkContext>, has_finalizer: bool) {
        self.try_context = Some(TryContext {
            upper: self.try_context.take().map(Box::new),
            position: TryContextPosition::Try,
            has_finalizer,
            returned_fork_context: has_finalizer
                .then(|| ForkContext::new_empty(arena, self.fork_context, None)),
            thrown_fork_context: ForkContext::new_empty(arena, self.fork_context, None),
            last_of_try_is_reachable: Default::default(),
            last_of_catch_is_reachable: Default::default(),
        });
    }

    fn push_break_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        breakable: bool,
        label: Option<String>,
    ) -> Id<ForkContext> {
        self.break_context = Some(BreakContext {
            upper: self.break_context.take().map(Box::new),
            breakable,
            label,
            broken_fork_context: ForkContext::new_empty(arena, self.fork_context, None),
        });
        self.break_context.as_ref().unwrap().broken_fork_context
    }

    fn pop_break_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) -> BreakContext {
        let mut context = self.break_context.take().unwrap();
        let fork_context = self.fork_context;

        self.break_context = context.upper.take().map(|box_| *box_);

        if !context.breakable {
            let broken_fork_context = context.broken_fork_context;

            if !arena.get(broken_fork_context).unwrap().empty() {
                let segments = arena.get(fork_context).unwrap().head().clone();
                arena
                    .get_mut(broken_fork_context)
                    .unwrap()
                    .add(code_path_segment_arena, segments);
                let segments = arena.get(broken_fork_context).unwrap().make_next(
                    code_path_segment_arena,
                    0,
                    -1,
                );
                arena
                    .get_mut(fork_context)
                    .unwrap()
                    .replace_head(code_path_segment_arena, segments);
            }
        }

        context
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ChoiceContextKind {
    LogicalAnd,
    LogicalOr,
    LogicalNullCoalesce,
    Test,
    Loop,
}

struct ChoiceContext {
    upper: Option<Box<ChoiceContext>>,
    kind: ChoiceContextKind,
    is_forking_as_result: bool,
    true_fork_context: Id<ForkContext>,
    false_fork_context: Id<ForkContext>,
    qq_fork_context: Id<ForkContext>,
    processed: bool,
}

struct SwitchContext {
    upper: Option<Box<SwitchContext>>,
    has_case: bool,
    default_segments: Option<Vec<Id<CodePathSegment>>>,
    default_body_segments: Option<Vec<Id<CodePathSegment>>>,
    found_default: bool,
    last_is_default: bool,
    count_forks: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TryContextPosition {
    Try,
    Catch,
    Finally,
}

struct TryContext {
    upper: Option<Box<TryContext>>,
    position: TryContextPosition,
    has_finalizer: bool,
    returned_fork_context: Option<Id<ForkContext>>,
    thrown_fork_context: Id<ForkContext>,
    last_of_try_is_reachable: bool,
    last_of_catch_is_reachable: bool,
}

enum LoopContext {
    While(WhileLoopContext),
    Do(DoLoopContext),
    For(ForLoopContext),
    ForIn(ForInLoopContext),
}

struct WhileLoopContext {
    upper: Option<Box<LoopContext>>,
    label: Option<String>,
    test: Option<bool>,
    continue_dest_segments: Option<Vec<Id<CodePathSegment>>>,
    broken_fork_context: Id<ForkContext>,
}

struct DoLoopContext {
    upper: Option<Box<LoopContext>>,
    label: Option<String>,
    test: Option<bool>,
    entry_segments: Option<Vec<Id<CodePathSegment>>>,
    continue_fork_context: Id<ForkContext>,
    broken_fork_context: Id<ForkContext>,
}

struct ForLoopContext {
    upper: Option<Box<LoopContext>>,
    label: Option<String>,
    test: Option<bool>,
    end_of_init_segments: Option<Vec<Id<CodePathSegment>>>,
    test_segments: Option<Vec<Id<CodePathSegment>>>,
    end_of_test_segments: Option<Vec<Id<CodePathSegment>>>,
    update_segments: Option<Vec<Id<CodePathSegment>>>,
    end_of_update_segments: Option<Vec<Id<CodePathSegment>>>,
    continue_dest_segments: Option<Vec<Id<CodePathSegment>>>,
    broken_fork_context: Id<ForkContext>,
}

struct ForInLoopContext {
    upper: Option<Box<LoopContext>>,
    type_: Kind,
    label: Option<String>,
    prev_segments: Option<Vec<Id<CodePathSegment>>>,
    left_segments: Option<Vec<Id<CodePathSegment>>>,
    end_of_left_segments: Option<Vec<Id<CodePathSegment>>>,
    continue_dest_segments: Option<Vec<Id<CodePathSegment>>>,
    broken_fork_context: Id<ForkContext>,
}

struct BreakContext {
    upper: Option<Box<BreakContext>>,
    breakable: bool,
    label: Option<String>,
    broken_fork_context: Id<ForkContext>,
}

struct ChainContext {
    upper: Option<Box<ChainContext>>,
    count_choice_contexts: usize,
}
