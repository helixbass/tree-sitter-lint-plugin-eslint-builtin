use id_arena::{Arena, Id};
use squalid::return_if_none;
use std::{iter, rc::Rc};
use tree_sitter_lint::tree_sitter::Node;

use crate::kind::{DoStatement, ForInStatement, ForStatement, Kind, WhileStatement};

use super::{
    code_path_analyzer::{Event, OnLooped},
    code_path_segment::CodePathSegment,
    fork_context::ForkContext,
    id_generator::IdGenerator,
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

fn get_continue_context<'a>(
    state: &'a CodePathState,
    label: Option<&str>,
) -> Option<&'a LoopContext> {
    let label = match label {
        None => return state.loop_context.as_ref(),
        Some(label) => label,
    };

    let mut context = state.loop_context.as_ref();
    while let Some(context_present) = context {
        if context_present.label() == Some(label) {
            return Some(context_present);
        }
        context = context_present.upper();
    }

    None
}

fn get_break_context<'a>(
    state: &'a CodePathState,
    label: Option<&str>,
) -> Option<&'a BreakContext> {
    let mut context = state.break_context.as_ref();

    while let Some(context_present) = context {
        if match label {
            Some(label) => context_present.label.as_deref() == Some(label),
            None => context_present.breakable,
        } {
            return Some(context_present);
        }
        context = context_present.upper.as_deref();
    }

    None
}

fn get_return_context(state: &CodePathState) -> Option<Id<ForkContext>> {
    let mut context = state.try_context.as_ref();

    while let Some(context_present) = context {
        if context_present.has_finalizer && context_present.position != TryContextPosition::Finally
        {
            return Some(context_present.returned_fork_context.unwrap());
        }
        context = context_present.upper.as_deref();
    }

    None
}

fn get_throw_context(state: &CodePathState) -> Option<(Id<ForkContext>, TryContextPosition)> {
    let mut context = state.try_context.as_ref();

    while let Some(context_present) = context {
        if context_present.position == TryContextPosition::Try
            || context_present.has_finalizer
                && context_present.position == TryContextPosition::Catch
        {
            return Some((
                context_present.thrown_fork_context,
                context_present.position,
            ));
        }
        context = context_present.upper.as_deref();
    }

    None
}

fn remove<T: PartialEq>(xs: &mut Vec<T>, x: &T) {
    if let Some(found_index) = xs.iter().position(|item| item == x) {
        xs.remove(found_index);
    }
}

fn remove_connection(
    arena: &mut Arena<CodePathSegment>,
    prev_segments: &[Id<CodePathSegment>],
    next_segments: &[Id<CodePathSegment>],
) {
    for (i, &prev_segment) in prev_segments.into_iter().enumerate() {
        let next_segment = next_segments[i];

        remove(
            &mut arena.get_mut(prev_segment).unwrap().next_segments,
            &next_segment,
        );
        remove(
            &mut arena.get_mut(prev_segment).unwrap().all_next_segments,
            &next_segment,
        );
        remove(
            &mut arena.get_mut(next_segment).unwrap().prev_segments,
            &prev_segment,
        );
        remove(
            &mut arena.get_mut(next_segment).unwrap().all_prev_segments,
            &prev_segment,
        );
    }
}

fn make_looped<'a>(
    arena: &mut Arena<CodePathSegment>,
    current_node: Node<'a>,
    pending_events: &mut Vec<Event<'a>>,
    state: &CodePathState,
    unflattened_from_segments: &[Id<CodePathSegment>],
    unflattened_to_segments: &[Id<CodePathSegment>],
) {
    let from_segments = CodePathSegment::flatten_unused_segments(arena, unflattened_from_segments);
    let to_segments = CodePathSegment::flatten_unused_segments(arena, unflattened_to_segments);

    for (from_segment, to_segment) in iter::zip(from_segments, to_segments) {
        if arena.get(to_segment).unwrap().reachable {
            arena
                .get_mut(from_segment)
                .unwrap()
                .next_segments
                .push(to_segment);
        }
        if arena.get(from_segment).unwrap().reachable {
            arena
                .get_mut(to_segment)
                .unwrap()
                .prev_segments
                .push(from_segment);
        }
        arena
            .get_mut(from_segment)
            .unwrap()
            .all_next_segments
            .push(to_segment);
        arena
            .get_mut(to_segment)
            .unwrap()
            .all_prev_segments
            .push(from_segment);

        if arena.get(to_segment).unwrap().all_prev_segments.len() >= 2 {
            CodePathSegment::mark_prev_segment_as_looped(arena, to_segment, from_segment);
        }

        state.notify_looped.on_looped(
            arena,
            current_node,
            pending_events,
            from_segment,
            to_segment,
        );
    }
}

fn finalize_test_segments_of_for(
    arena: &mut Arena<ForkContext>,
    code_path_segment_arena: &mut Arena<CodePathSegment>,
    context: &mut ForLoopContext,
    choice_context: &ChoiceContext,
    head: Vec<Id<CodePathSegment>>,
) {
    if !choice_context.processed {
        arena[choice_context.true_fork_context].add(code_path_segment_arena, head.clone());
        arena[choice_context.false_fork_context].add(code_path_segment_arena, head.clone());
        arena[choice_context.qq_fork_context].add(code_path_segment_arena, head.clone());
    }

    if context.test != Some(true) {
        ForkContext::add_all(
            arena,
            context.broken_fork_context,
            choice_context.false_fork_context,
        );
    }
    context.end_of_test_segments =
        Some(arena[choice_context.true_fork_context].make_next(code_path_segment_arena, 0, -1));
}

pub struct CodePathState {
    id_generator: Rc<IdGenerator>,
    notify_looped: OnLooped,
    fork_context: Id<ForkContext>,
    choice_context: Option<ChoiceContext>,
    switch_context: Option<SwitchContext>,
    try_context: Option<TryContext>,
    loop_context: Option<LoopContext>,
    break_context: Option<BreakContext>,
    chain_context: Option<ChainContext>,
    pub current_segments: Vec<Id<CodePathSegment>>,
    pub initial_segment: Id<CodePathSegment>,
    pub final_segments: Vec<Id<CodePathSegment>>,
    pub returned_fork_context: Vec<Id<CodePathSegment>>,
    pub thrown_fork_context: Vec<Id<CodePathSegment>>,
}

impl CodePathState {
    pub fn new(
        fork_context_arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        id_generator: Rc<IdGenerator>,
        on_looped: OnLooped,
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

    pub fn head_segments<'a>(&self, arena: &'a Arena<ForkContext>) -> &'a [Id<CodePathSegment>] {
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

    pub fn push_fork_context(
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

    pub fn fork_path(
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

    pub fn fork_bypass_path(
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

    pub fn make_logical_right(
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

    pub fn make_if_consequent(
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

    pub fn make_if_alternate(
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

    pub fn make_optional_right(
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

    fn pop_switch_context<'a>(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        current_node: Node<'a>,
        pending_events: &mut Vec<Event<'a>>,
    ) {
        let mut context = self.switch_context.take().unwrap();

        self.switch_context = context.upper.take().map(|box_| *box_);

        let fork_context = self.fork_context;
        let broken_fork_context = self
            .pop_break_context(arena, code_path_segment_arena)
            .broken_fork_context;

        if context.count_forks == 0 {
            if !arena.get(broken_fork_context).unwrap().empty() {
                let segments =
                    arena
                        .get(fork_context)
                        .unwrap()
                        .make_next(code_path_segment_arena, -1, -1);
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

            return;
        }

        let last_segments = arena.get(fork_context).unwrap().head().clone();

        self.fork_bypass_path(arena, code_path_segment_arena);
        let last_case_segments = arena.get(fork_context).unwrap().head().clone();

        arena
            .get_mut(broken_fork_context)
            .unwrap()
            .add(code_path_segment_arena, last_segments);

        if !context.last_is_default {
            if let Some(default_body_segments) = context.default_body_segments.as_ref() {
                remove_connection(
                    code_path_segment_arena,
                    context.default_segments.as_ref().unwrap(),
                    default_body_segments,
                );
                make_looped(
                    code_path_segment_arena,
                    current_node,
                    pending_events,
                    self,
                    &last_case_segments,
                    default_body_segments,
                );
            } else {
                arena
                    .get_mut(broken_fork_context)
                    .unwrap()
                    .add(code_path_segment_arena, last_case_segments);
            }
        }

        for _ in 0..context.count_forks {
            self.fork_context = arena.get(self.fork_context).unwrap().upper.unwrap();
        }

        let segments =
            arena
                .get(broken_fork_context)
                .unwrap()
                .make_next(code_path_segment_arena, 0, -1);
        arena
            .get_mut(self.fork_context)
            .unwrap()
            .replace_head(code_path_segment_arena, segments);
    }

    pub fn make_switch_case_body(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        is_empty: bool,
        is_default: bool,
    ) {
        if !self.switch_context.as_ref().unwrap().has_case {
            return;
        }

        let parent_fork_context = self.fork_context;
        let fork_context = self.push_fork_context(arena, None);

        let segments =
            arena
                .get(parent_fork_context)
                .unwrap()
                .make_next(code_path_segment_arena, 0, -1);
        arena
            .get_mut(fork_context)
            .unwrap()
            .add(code_path_segment_arena, segments);

        #[allow(clippy::collapsible_else_if)]
        if is_default {
            self.switch_context.as_mut().unwrap().default_segments =
                Some(arena.get(parent_fork_context).unwrap().head().clone());
            if is_empty {
                self.switch_context.as_mut().unwrap().found_default = true;
            } else {
                self.switch_context.as_mut().unwrap().default_body_segments =
                    Some(arena.get(fork_context).unwrap().head().clone());
            }
        } else {
            if !is_empty && self.switch_context.as_ref().unwrap().found_default {
                self.switch_context.as_mut().unwrap().found_default = false;
                self.switch_context.as_mut().unwrap().default_body_segments =
                    Some(arena.get(fork_context).unwrap().head().clone());
            }
        }

        self.switch_context.as_mut().unwrap().last_is_default = is_default;
        self.switch_context.as_mut().unwrap().count_forks += 1;
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

    fn pop_try_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let mut context = self.try_context.take().unwrap();

        self.try_context = context.upper.take().map(|box_| *box_);

        if context.position == TryContextPosition::Catch {
            self.pop_fork_context(arena, code_path_segment_arena);
            return;
        }

        let returned = context.returned_fork_context.unwrap();
        let thrown = context.thrown_fork_context;

        if arena.get(returned).unwrap().empty() && arena.get(thrown).unwrap().empty() {
            return;
        }

        let head_segments = arena.get(self.fork_context).unwrap().head().clone();

        self.fork_context = arena.get(self.fork_context).unwrap().upper.unwrap();
        let normal_segments = &head_segments[..head_segments.len() / 2];
        let leaving_segments = &head_segments[head_segments.len() / 2..];

        if !arena.get(returned).unwrap().empty() {
            match get_return_context(self) {
                Some(returned_fork_context) => {
                    arena
                        .get_mut(returned_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, leaving_segments.to_owned());
                }
                None => {
                    self.returned_fork_context_add(leaving_segments);
                }
            }
        }
        if !arena.get(thrown).unwrap().empty() {
            match get_throw_context(self) {
                Some((thrown_fork_context, _)) => {
                    arena
                        .get_mut(thrown_fork_context)
                        .unwrap()
                        .add(code_path_segment_arena, leaving_segments.to_owned());
                }
                None => {
                    self.thrown_fork_context_add(leaving_segments);
                }
            }
        }

        arena
            .get_mut(self.fork_context)
            .unwrap()
            .replace_head(code_path_segment_arena, normal_segments.to_owned());

        if !context.last_of_try_is_reachable && !context.last_of_catch_is_reachable {
            unreachable!("maybe? looks like passing no arguments to makeUnreachable() would result in some NaN's in makeSegments()");
            // arena.get_mut(self.fork_context).unwrap().make_unreachable();
        }
    }

    pub fn make_catch_block(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let fork_context = self.fork_context;
        let thrown = self.try_context.as_ref().unwrap().thrown_fork_context;

        self.try_context.as_mut().unwrap().position = TryContextPosition::Catch;
        self.try_context.as_mut().unwrap().thrown_fork_context =
            ForkContext::new_empty(arena, fork_context, None);
        self.try_context.as_mut().unwrap().last_of_try_is_reachable = arena
            .get(fork_context)
            .unwrap()
            .reachable(code_path_segment_arena);

        let segments = arena.get(fork_context).unwrap().head().clone();
        arena
            .get_mut(thrown)
            .unwrap()
            .add(code_path_segment_arena, segments);
        let thrown_segments = arena
            .get(thrown)
            .unwrap()
            .make_next(code_path_segment_arena, 0, -1);

        self.push_fork_context(arena, None);
        self.fork_bypass_path(arena, code_path_segment_arena);
        arena
            .get_mut(self.fork_context)
            .unwrap()
            .add(code_path_segment_arena, thrown_segments);
    }

    pub fn make_finally_block(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let mut fork_context = self.fork_context;
        let returned = self
            .try_context
            .as_ref()
            .unwrap()
            .returned_fork_context
            .unwrap();
        let thrown = self.try_context.as_ref().unwrap().thrown_fork_context;
        let head_of_leaving_segments = arena.get(fork_context).unwrap().head().clone();

        if self.try_context.as_ref().unwrap().position == TryContextPosition::Catch {
            self.pop_fork_context(arena, code_path_segment_arena);
            fork_context = self.fork_context;

            self.try_context
                .as_mut()
                .unwrap()
                .last_of_catch_is_reachable = arena
                .get(fork_context)
                .unwrap()
                .reachable(code_path_segment_arena);
        } else {
            self.try_context.as_mut().unwrap().last_of_try_is_reachable = arena
                .get(fork_context)
                .unwrap()
                .reachable(code_path_segment_arena);
        }
        self.try_context.as_mut().unwrap().position = TryContextPosition::Finally;

        if arena.get(returned).unwrap().empty() && arena.get(thrown).unwrap().empty() {
            return;
        }

        let mut segments =
            arena
                .get(fork_context)
                .unwrap()
                .make_next(code_path_segment_arena, -1, -1);

        for i in 0..arena.get(fork_context).unwrap().count {
            let mut prev_segs_of_leaving_segment = vec![head_of_leaving_segments[i]];

            for segments in &arena.get(returned).unwrap().segments_list {
                prev_segs_of_leaving_segment.push(segments[i]);
            }
            for segments in &arena.get(thrown).unwrap().segments_list {
                prev_segs_of_leaving_segment.push(segments[i]);
            }

            segments.push(CodePathSegment::new_next(
                code_path_segment_arena,
                self.id_generator.next(),
                &prev_segs_of_leaving_segment,
            ));
        }

        self.push_fork_context(arena, Some(true));

        arena
            .get_mut(self.fork_context)
            .unwrap()
            .add(code_path_segment_arena, segments);
    }

    fn make_first_throwable_path_in_try_block(
        &self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let fork_context = self.fork_context;

        if !arena[fork_context].reachable(code_path_segment_arena) {
            return;
        }

        let (thrown_fork_context, position) = return_if_none!(get_throw_context(self));
        if position != TryContextPosition::Try {
            return;
        }
        if !arena[thrown_fork_context].empty() {
            return;
        }

        let segments = arena[fork_context].head().clone();
        arena[thrown_fork_context].add(code_path_segment_arena, segments);
        let segments = arena[fork_context].make_next(code_path_segment_arena, -1, -1);
        arena[fork_context].replace_head(code_path_segment_arena, segments);
    }

    fn push_loop_context(
        &mut self,
        arena: &mut Arena<ForkContext>,
        current_node: Node,
        pending_events: &mut Vec<Event>,
        type_: Kind,
        label: Option<String>,
        is_for_of: bool,
    ) {
        let fork_context = self.fork_context;
        let break_context_broken_fork_context = self.push_break_context(arena, true, label.clone());

        match type_ {
            WhileStatement => {
                self.push_choice_context(arena, ChoiceContextKind::Loop, false);
                self.loop_context = Some(LoopContext::While(WhileLoopContext {
                    upper: self.loop_context.take().map(Box::new),
                    label,
                    test: Default::default(),
                    continue_dest_segments: Default::default(),
                    broken_fork_context: break_context_broken_fork_context,
                }));
            }
            DoStatement => {
                self.push_choice_context(arena, ChoiceContextKind::Loop, false);
                self.loop_context = Some(LoopContext::Do(DoLoopContext {
                    upper: self.loop_context.take().map(Box::new),
                    label,
                    test: Default::default(),
                    entry_segments: Default::default(),
                    continue_fork_context: ForkContext::new_empty(arena, fork_context, None),
                    broken_fork_context: break_context_broken_fork_context,
                }));
            }
            ForStatement => {
                self.push_choice_context(arena, ChoiceContextKind::Loop, false);
                self.loop_context = Some(LoopContext::For(ForLoopContext {
                    upper: self.loop_context.take().map(Box::new),
                    label,
                    test: Default::default(),
                    end_of_init_segments: Default::default(),
                    test_segments: Default::default(),
                    end_of_test_segments: Default::default(),
                    update_segments: Default::default(),
                    end_of_update_segments: Default::default(),
                    continue_dest_segments: Default::default(),
                    broken_fork_context: break_context_broken_fork_context,
                }));
            }
            ForInStatement => {
                self.loop_context = Some(LoopContext::ForIn(ForInLoopContext {
                    upper: self.loop_context.take().map(Box::new),
                    is_for_of,
                    label,
                    prev_segments: Default::default(),
                    left_segments: Default::default(),
                    end_of_left_segments: Default::default(),
                    continue_dest_segments: Default::default(),
                    broken_fork_context: break_context_broken_fork_context,
                }));
            }
            _ => unreachable!(),
        }
    }

    fn pop_loop_context<'a>(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        current_node: Node<'a>,
        pending_events: &mut Vec<Event<'a>>,
    ) {
        let mut context = self.loop_context.take().unwrap();

        self.loop_context = context.take_upper().map(|box_| *box_);

        let fork_context = self.fork_context;
        let broken_fork_context = self
            .pop_break_context(arena, code_path_segment_arena)
            .broken_fork_context;

        match context {
            LoopContext::While(context) => {
                self.pop_choice_context(arena, code_path_segment_arena);
                make_looped(
                    code_path_segment_arena,
                    current_node,
                    pending_events,
                    self,
                    arena[fork_context].head(),
                    context.continue_dest_segments.as_ref().unwrap(),
                );
            }
            LoopContext::For(context) => {
                self.pop_choice_context(arena, code_path_segment_arena);
                make_looped(
                    code_path_segment_arena,
                    current_node,
                    pending_events,
                    self,
                    arena[fork_context].head(),
                    context.continue_dest_segments.as_ref().unwrap(),
                );
            }
            LoopContext::Do(context) => {
                let choice_context = self.pop_choice_context(arena, code_path_segment_arena);

                if !choice_context.processed {
                    let segments = arena[fork_context].head().clone();
                    arena[choice_context.true_fork_context]
                        .add(code_path_segment_arena, segments.clone());
                    arena[choice_context.false_fork_context].add(code_path_segment_arena, segments);
                }
                if context.test != Some(true) {
                    ForkContext::add_all(
                        arena,
                        broken_fork_context,
                        choice_context.false_fork_context,
                    );
                }

                let segments_list = &arena[choice_context.true_fork_context].segments_list;

                for segments in segments_list {
                    make_looped(
                        code_path_segment_arena,
                        current_node,
                        pending_events,
                        self,
                        segments,
                        context.entry_segments.as_ref().unwrap(),
                    );
                }
            }
            LoopContext::ForIn(context) => {
                let segments = arena[fork_context].head().clone();
                arena[broken_fork_context].add(code_path_segment_arena, segments);
                make_looped(
                    code_path_segment_arena,
                    current_node,
                    pending_events,
                    self,
                    arena[fork_context].head(),
                    context.left_segments.as_ref().unwrap(),
                );
            }
        }

        if arena[broken_fork_context].empty() {
            let segments = arena[fork_context].make_unreachable(code_path_segment_arena, -1, -1);
            arena[fork_context].replace_head(code_path_segment_arena, segments);
        } else {
            let segments =
                arena[broken_fork_context].make_unreachable(code_path_segment_arena, 0, -1);
            arena[fork_context].replace_head(code_path_segment_arena, segments);
        }
    }

    pub fn make_while_test(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        test: Option<bool>,
    ) {
        let context = self
            .loop_context
            .as_mut()
            .unwrap()
            .as_while_loop_context_mut();
        let fork_context = self.fork_context;
        let test_segments = arena[fork_context].make_next(code_path_segment_arena, 0, -1);

        context.test = test;
        context.continue_dest_segments = Some(test_segments.clone());
        arena[fork_context].replace_head(code_path_segment_arena, test_segments);
    }

    pub fn make_while_body(
        &self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self.loop_context.as_ref().unwrap().as_while_loop_context();
        let choice_context = self.choice_context.as_ref().unwrap();
        let fork_context = self.fork_context;

        if !choice_context.processed {
            let segments = arena[fork_context].head().clone();
            arena[choice_context.true_fork_context].add(code_path_segment_arena, segments.clone());
            arena[choice_context.false_fork_context].add(code_path_segment_arena, segments);
        }

        if context.test != Some(true) {
            ForkContext::add_all(
                arena,
                context.broken_fork_context,
                choice_context.false_fork_context,
            );
        }
        let segments =
            arena[choice_context.true_fork_context].make_next(code_path_segment_arena, 0, -1);
        arena[fork_context].replace_head(code_path_segment_arena, segments);
    }

    pub fn make_do_while_body(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self.loop_context.as_mut().unwrap().as_do_loop_context_mut();
        let fork_context = self.fork_context;
        let body_segments = arena[fork_context].make_next(code_path_segment_arena, -1, -1);

        context.entry_segments = Some(body_segments.clone());
        arena[fork_context].replace_head(code_path_segment_arena, body_segments);
    }

    pub fn make_do_while_test(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        test: Option<bool>,
    ) {
        let context = self.loop_context.as_mut().unwrap().as_do_loop_context_mut();
        let fork_context = self.fork_context;

        context.test = test;

        if !arena[context.continue_fork_context].empty() {
            let segments = arena[fork_context].head().clone();
            arena[context.continue_fork_context].add(code_path_segment_arena, segments);
            let test_segments =
                arena[context.continue_fork_context].make_next(code_path_segment_arena, 0, -1);

            arena[fork_context].replace_head(code_path_segment_arena, test_segments);
        }
    }

    pub fn make_for_test(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        test: Option<bool>,
    ) {
        let context = self
            .loop_context
            .as_mut()
            .unwrap()
            .as_for_loop_context_mut();
        let fork_context = self.fork_context;
        let end_of_init_segments = arena[fork_context].head().clone();
        let test_segments = arena[fork_context].make_next(code_path_segment_arena, -1, -1);

        context.test = test;
        context.end_of_init_segments = Some(end_of_init_segments);
        context.test_segments = Some(test_segments.clone());
        context.continue_dest_segments = Some(test_segments.clone());
        arena[fork_context].replace_head(code_path_segment_arena, test_segments);
    }

    pub fn make_for_update(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self
            .loop_context
            .as_mut()
            .unwrap()
            .as_for_loop_context_mut();
        let choice_context = self.choice_context.as_ref().unwrap();
        let fork_context = self.fork_context;

        if context.test_segments.is_some() {
            let segments = arena[fork_context].head().clone();
            finalize_test_segments_of_for(
                arena,
                code_path_segment_arena,
                context,
                choice_context,
                segments,
            );
        } else {
            context.end_of_init_segments = Some(arena[fork_context].head().clone());
        }

        let update_segments =
            arena[fork_context].make_disconnected(code_path_segment_arena, -1, -1);

        context.update_segments = Some(update_segments.clone());
        context.continue_dest_segments = Some(update_segments.clone());
        arena[fork_context].replace_head(code_path_segment_arena, update_segments);
    }

    pub fn make_for_body<'a>(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        current_node: Node<'a>,
        pending_events: &mut Vec<Event<'a>>,
    ) {
        let choice_context = self.choice_context.as_ref().unwrap();
        let fork_context = self.fork_context;

        if self
            .loop_context
            .as_ref()
            .unwrap()
            .as_for_loop_context()
            .update_segments
            .is_some()
        {
            self.loop_context
                .as_mut()
                .unwrap()
                .as_for_loop_context_mut()
                .end_of_update_segments = Some(arena[fork_context].head().clone());

            if let Some(test_segments) = self
                .loop_context
                .as_ref()
                .unwrap()
                .as_for_loop_context()
                .test_segments
                .as_ref()
            {
                make_looped(
                    code_path_segment_arena,
                    current_node,
                    pending_events,
                    self,
                    self.loop_context
                        .as_ref()
                        .unwrap()
                        .as_for_loop_context()
                        .end_of_update_segments
                        .as_ref()
                        .unwrap(),
                    test_segments,
                );
            }
        } else if self
            .loop_context
            .as_ref()
            .unwrap()
            .as_for_loop_context()
            .test_segments
            .is_some()
        {
            finalize_test_segments_of_for(
                arena,
                code_path_segment_arena,
                self.loop_context
                    .as_mut()
                    .unwrap()
                    .as_for_loop_context_mut(),
                choice_context,
                arena[fork_context].head().clone(),
            );
        } else {
            self.loop_context
                .as_mut()
                .unwrap()
                .as_for_loop_context_mut()
                .end_of_init_segments = Some(arena[fork_context].head().clone());
        }

        let body_segments = self
            .loop_context
            .as_ref()
            .unwrap()
            .as_for_loop_context()
            .end_of_test_segments
            .clone();

        let body_segments = body_segments.unwrap_or_else(|| {
            let prev_fork_context = ForkContext::new_empty(arena, fork_context, None);

            arena[prev_fork_context].add(
                code_path_segment_arena,
                self.loop_context
                    .as_ref()
                    .unwrap()
                    .as_for_loop_context()
                    .end_of_init_segments
                    .clone()
                    .unwrap(),
            );
            if let Some(end_of_update_segments) = self
                .loop_context
                .as_ref()
                .unwrap()
                .as_for_loop_context()
                .end_of_update_segments
                .clone()
            {
                arena[prev_fork_context].add(code_path_segment_arena, end_of_update_segments);
            }

            arena[prev_fork_context].make_next(code_path_segment_arena, 0, -1)
        });
        if self
            .loop_context
            .as_ref()
            .unwrap()
            .as_for_loop_context()
            .continue_dest_segments
            .is_none()
        {
            self.loop_context
                .as_mut()
                .unwrap()
                .as_for_loop_context_mut()
                .continue_dest_segments = Some(body_segments.clone());
        }
        arena[fork_context].replace_head(code_path_segment_arena, body_segments);
    }

    pub fn make_for_in_of_left(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self
            .loop_context
            .as_mut()
            .unwrap()
            .as_for_in_loop_context_mut();
        let fork_context = self.fork_context;
        let left_segments = arena[fork_context].make_disconnected(code_path_segment_arena, -1, -1);

        context.prev_segments = Some(arena[fork_context].head().clone());
        context.continue_dest_segments = Some(left_segments.clone());
        context.left_segments = Some(left_segments.clone());
        arena[fork_context].replace_head(code_path_segment_arena, left_segments);
    }

    pub fn make_for_in_of_right(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let context = self
            .loop_context
            .as_mut()
            .unwrap()
            .as_for_in_loop_context_mut();
        let fork_context = self.fork_context;
        let temp = ForkContext::new_empty(arena, fork_context, None);

        arena[temp].add(
            code_path_segment_arena,
            context.prev_segments.clone().unwrap(),
        );
        let right_segments = arena[temp].make_next(code_path_segment_arena, -1, -1);

        context.end_of_left_segments = Some(arena[fork_context].head().clone());
        arena[fork_context].replace_head(code_path_segment_arena, right_segments);
    }

    pub fn make_for_in_of_body<'a>(
        &self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        current_node: Node<'a>,
        pending_events: &mut Vec<Event<'a>>,
    ) {
        let context = self.loop_context.as_ref().unwrap().as_for_in_loop_context();
        let fork_context = self.fork_context;
        let temp = ForkContext::new_empty(arena, fork_context, None);

        arena[temp].add(
            code_path_segment_arena,
            context.end_of_left_segments.clone().unwrap(),
        );
        let body_segments = arena[temp].make_next(code_path_segment_arena, -1, -1);

        make_looped(
            code_path_segment_arena,
            current_node,
            pending_events,
            self,
            arena[fork_context].head(),
            context.left_segments.as_ref().unwrap(),
        );

        let segments = arena[fork_context].head().clone();
        arena[context.broken_fork_context].add(code_path_segment_arena, segments);
        arena[fork_context].replace_head(code_path_segment_arena, body_segments);
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

    fn make_break(
        &self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        label: Option<&str>,
    ) {
        let fork_context = self.fork_context;

        if !arena[fork_context].reachable(code_path_segment_arena) {
            return;
        }

        let context = get_break_context(self, label);

        if let Some(context) = context {
            let segments = arena[fork_context].head().clone();
            arena[context.broken_fork_context].add(code_path_segment_arena, segments);
        }

        let segments = arena[fork_context].make_unreachable(code_path_segment_arena, -1, -1);
        arena[fork_context].replace_head(code_path_segment_arena, segments);
    }

    fn make_continue<'a>(
        &self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
        current_node: Node<'a>,
        pending_events: &mut Vec<Event<'a>>,
        label: Option<&str>,
    ) {
        let fork_context = self.fork_context;

        if !arena[fork_context].reachable(code_path_segment_arena) {
            return;
        }

        let context = get_continue_context(self, label);

        if let Some(context) = context {
            if let Some(continue_dest_segments) = context.continue_dest_segments() {
                make_looped(
                    code_path_segment_arena,
                    current_node,
                    pending_events,
                    self,
                    arena[fork_context].head(),
                    continue_dest_segments,
                );

                if let LoopContext::ForIn(context) = context {
                    let segments = arena[fork_context].head().clone();
                    arena[context.broken_fork_context].add(code_path_segment_arena, segments);
                }
            } else {
                let segments = arena[fork_context].head().clone();
                arena[context.as_do_loop_context().continue_fork_context]
                    .add(code_path_segment_arena, segments);
            }
        }
        let segments = arena[fork_context].make_unreachable(code_path_segment_arena, -1, -1);
        arena[fork_context].replace_head(code_path_segment_arena, segments);
    }

    fn make_return(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let fork_context = self.fork_context;

        if arena[fork_context].reachable(code_path_segment_arena) {
            let segments = arena[fork_context].head().clone();
            match get_return_context(self) {
                Some(returned_fork_context) => {
                    arena[returned_fork_context].add(code_path_segment_arena, segments);
                }
                None => {
                    self.returned_fork_context_add(&segments);
                }
            }
            let segments = arena[fork_context].make_unreachable(code_path_segment_arena, -1, -1);
            arena[fork_context].replace_head(code_path_segment_arena, segments);
        }
    }

    fn make_throw(
        &mut self,
        arena: &mut Arena<ForkContext>,
        code_path_segment_arena: &mut Arena<CodePathSegment>,
    ) {
        let fork_context = self.fork_context;

        if arena[fork_context].reachable(code_path_segment_arena) {
            let segments = arena[fork_context].head().clone();
            match get_throw_context(self) {
                Some((thrown_fork_context, _)) => {
                    arena[thrown_fork_context].add(code_path_segment_arena, segments);
                }
                None => {
                    self.thrown_fork_context_add(&segments);
                }
            }
            let segments = arena[fork_context].make_unreachable(code_path_segment_arena, -1, -1);
            arena[fork_context].replace_head(code_path_segment_arena, segments);
        }
    }

    fn make_final(&mut self, code_path_segment_arena: &Arena<CodePathSegment>) {
        let segments = self.current_segments.clone();

        if !segments.is_empty() && code_path_segment_arena[segments[0]].reachable {
            self.returned_fork_context_add(&segments);
        }
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

impl LoopContext {
    pub fn label(&self) -> Option<&str> {
        match self {
            LoopContext::While(value) => value.label.as_deref(),
            LoopContext::Do(value) => value.label.as_deref(),
            LoopContext::For(value) => value.label.as_deref(),
            LoopContext::ForIn(value) => value.label.as_deref(),
        }
    }

    pub fn upper(&self) -> Option<&Self> {
        match self {
            LoopContext::While(value) => value.upper.as_deref(),
            LoopContext::Do(value) => value.upper.as_deref(),
            LoopContext::For(value) => value.upper.as_deref(),
            LoopContext::ForIn(value) => value.upper.as_deref(),
        }
    }

    pub fn take_upper(&mut self) -> Option<Box<Self>> {
        match self {
            LoopContext::While(value) => value.upper.take(),
            LoopContext::Do(value) => value.upper.take(),
            LoopContext::For(value) => value.upper.take(),
            LoopContext::ForIn(value) => value.upper.take(),
        }
    }

    pub fn continue_dest_segments(&self) -> Option<&[Id<CodePathSegment>]> {
        match self {
            LoopContext::While(value) => value.continue_dest_segments.as_deref(),
            LoopContext::Do(value) => None,
            LoopContext::For(value) => value.continue_dest_segments.as_deref(),
            LoopContext::ForIn(value) => value.continue_dest_segments.as_deref(),
        }
    }

    pub fn as_do_loop_context(&self) -> &DoLoopContext {
        match self {
            Self::Do(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_do_loop_context_mut(&mut self) -> &mut DoLoopContext {
        match self {
            Self::Do(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_for_in_loop_context(&self) -> &ForInLoopContext {
        match self {
            Self::ForIn(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_for_in_loop_context_mut(&mut self) -> &mut ForInLoopContext {
        match self {
            Self::ForIn(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_for_loop_context(&self) -> &ForLoopContext {
        match self {
            Self::For(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_for_loop_context_mut(&mut self) -> &mut ForLoopContext {
        match self {
            Self::For(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_while_loop_context(&self) -> &WhileLoopContext {
        match self {
            Self::While(value) => value,
            _ => unreachable!(),
        }
    }

    pub fn as_while_loop_context_mut(&mut self) -> &mut WhileLoopContext {
        match self {
            Self::While(value) => value,
            _ => unreachable!(),
        }
    }
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
    is_for_of: bool,
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
