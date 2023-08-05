use id_arena::{Arena, Id};
use std::rc::Rc;

use crate::kind::Kind;

use super::{
    code_path_segment::CodePathSegment, fork_context::ForkContext, id_generator::IdGenerator,
};

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
        }
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

    fn push_chain_context(&mut self) {
        self.chain_context = Some(ChainContext {
            upper: self.chain_context.take().map(Box::new),
            count_choice_contexts: Default::default(),
        });
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
