use std::{borrow::Cow, ops, rc::Rc};

use id_arena::{Arena, Id};
use itertools::{EitherOrBoth, Itertools};
use squalid::OptionExt;
use tree_sitter_lint::{
    better_any::tid,
    tree_sitter::Node,
    tree_sitter_grep::{RopeOrSlice, SupportedLanguage},
    FileRunContext, FromFileRunContext, NodeExt, SourceTextProvider,
};

use super::{
    code_path::{CodePath, CodePathOrigin},
    code_path_segment::CodePathSegment,
    code_path_state::ChoiceContextKind,
    debug_helpers as debug,
    fork_context::ForkContext,
    id_generator::IdGenerator,
};
use crate::{
    ast_helpers::{
        get_num_call_expression_arguments, is_outermost_chain_expression, NodeExtJs, Number,
    },
    kind::{
        self, is_literal_kind, Arguments, ArrayPattern, ArrowFunction, AssignmentPattern,
        AugmentedAssignmentExpression, BinaryExpression, BreakStatement, CallExpression,
        CatchClause, Class, ClassDeclaration, ClassStaticBlock, Comment, ContinueStatement,
        DoStatement, EmptyStatement, ExpressionStatement, False, FieldDefinition, ForInStatement,
        ForStatement, Function, FunctionDeclaration, GeneratorFunction,
        GeneratorFunctionDeclaration, Identifier, IfStatement, ImportClause, ImportSpecifier,
        LabeledStatement, MemberExpression, MethodDefinition, NamespaceImport, NewExpression, Null,
        ObjectAssignmentPattern, Pair, PairPattern, ParenthesizedExpression, Program,
        PropertyIdentifier, RestPattern, ReturnStatement, ShorthandPropertyIdentifier,
        SubscriptExpression, SwitchBody, SwitchCase, SwitchDefault, SwitchStatement,
        TernaryExpression, ThrowStatement, True, TryStatement, VariableDeclarator, WhileStatement,
        YieldExpression,
    },
    utils::ast_utils::BREAKABLE_TYPE_PATTERN,
    visit::{walk_tree, TreeEnterLeaveVisitor},
    EnterOrExit,
};

fn is_property_definition_value(node: Node) -> bool {
    let parent = node.parent();

    parent.matches(|parent| {
        parent.kind() == FieldDefinition && parent.child_by_field_name("value") == Some(node)
    })
}

fn is_handled_logical_operator_str(operator: &str) -> bool {
    matches!(operator, "&&" | "||" | "??")
}

fn is_handled_logical_operator(node: Node) -> bool {
    is_handled_logical_operator_str(node.field("operator").kind())
}

fn is_logical_assignment_operator(operator: &str) -> bool {
    matches!(operator, "&&=" | "||=" | "??=")
}

fn get_label<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Option<Cow<'a, str>> {
    node.parent()
        .unwrap()
        .when_kind(LabeledStatement)
        .map(|parent| parent.field("label").text(source_text_provider))
}

fn is_forking_by_true_or_false(node: Node) -> bool {
    let parent = node.next_non_parentheses_ancestor();

    if parent.kind() == ExpressionStatement && {
        let parent_parent = parent.parent().unwrap();
        parent_parent.kind() == ForStatement && parent_parent.field("condition") == parent
    } {
        return true;
    }

    match parent.kind() {
        TernaryExpression | IfStatement | WhileStatement | DoStatement | ForStatement => {
            parent.field("condition").skip_parentheses() == node
        }
        BinaryExpression => is_handled_logical_operator(parent),
        AugmentedAssignmentExpression => {
            is_logical_assignment_operator(parent.field("operator").kind())
        }
        _ => false,
    }
}

fn get_boolean_value_if_simple_constant<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Option<bool> {
    is_literal_kind(node.kind()).then(|| match node.kind() {
        kind::String => !node.text(source_text_provider).is_empty(),
        kind::Number => Number::from(&*node.text(source_text_provider)).is_truthy(),
        kind::Regex => true,
        Null => false,
        True => true,
        False => false,
        _ => unreachable!(),
    })
}

fn is_identifier_reference(node: Node) -> bool {
    let parent = node.parent().unwrap();

    match parent.kind() {
        LabeledStatement | BreakStatement | ContinueStatement | ArrayPattern | RestPattern
        | ImportClause | ImportSpecifier | NamespaceImport | CatchClause => false,
        FunctionDeclaration
        | GeneratorFunctionDeclaration
        | Function
        | GeneratorFunction
        | ArrowFunction
        | ClassDeclaration
        | Class
        | VariableDeclarator
        | MethodDefinition => !parent
            .child_by_field_name("name")
            .matches(|name| name == node),
        FieldDefinition => parent.field("property") != node,
        Pair | PairPattern => parent.field("key") != node,
        AssignmentPattern | ObjectAssignmentPattern => parent.field("left") != node,
        ForInStatement => {
            !(parent.field("left") == node && parent.child_by_field_name("kind").is_some())
        }
        _ => true,
    }
}

pub struct CodePathAnalyzer<'a> {
    pub code_paths: Vec<Id<CodePath<'a>>>,
    active_code_path: Option<Id<CodePath<'a>>>,
    id_generator: Rc<IdGenerator>,
    current_node: Option<Node<'a>>,
    pub code_path_arena: Arena<CodePath<'a>>,
    pub fork_context_arena: Arena<ForkContext<'a>>,
    pub code_path_segment_arena: Arena<CodePathSegment<'a>>,
    file_contents: RopeOrSlice<'a>,
}

impl<'a> CodePathAnalyzer<'a> {
    pub fn new(file_contents: RopeOrSlice<'a>) -> Self {
        Self {
            code_paths: Default::default(),
            active_code_path: Default::default(),
            id_generator: Rc::new(IdGenerator::new("s")),
            current_node: Default::default(),
            code_path_arena: Default::default(),
            fork_context_arena: Default::default(),
            code_path_segment_arena: Default::default(),
            file_contents,
        }
    }

    fn maybe_code_path(&self) -> Option<Id<CodePath>> {
        self.active_code_path
    }

    fn code_path(&self) -> Id<CodePath> {
        self.maybe_code_path().unwrap()
    }

    fn forward_current_to_head(&mut self, _node: Node<'a>) {
        let code_path = self.active_code_path.unwrap();
        let state = &mut self.code_path_arena[code_path].state;
        let current_segments = state
            .current_segments
            .as_ref()
            .map_or_default(|current_segments| current_segments.segments());
        let head_segments = state.head_segments(&self.fork_context_arena);
        let head_segments_segments = head_segments.segments();

        for either_or_both in current_segments
            .iter()
            .zip_longest(&*head_segments_segments)
        {
            match either_or_both {
                EitherOrBoth::Both(current_segment, head_segment)
                    if current_segment != head_segment =>
                {
                    debug::dump(&format!(
                        "onCodePathSegmentEnd {}",
                        self.code_path_segment_arena[*current_segment].id
                    ));

                    // if self.code_path_segment_arena[*current_segment].reachable {
                    //     self.current_events
                    //         .push(Event::OnCodePathSegmentEnd(*current_segment, node));
                    // }
                }
                EitherOrBoth::Left(current_segment) => {
                    debug::dump(&format!(
                        "onCodePathSegmentEnd {}",
                        self.code_path_segment_arena[*current_segment].id
                    ));

                    // if self.code_path_segment_arena[*current_segment].reachable {
                    //     self.current_events
                    //         .push(Event::OnCodePathSegmentEnd(*current_segment, node));
                    // }
                }
                _ => (),
            }
        }

        state.current_segments = Some(head_segments.clone());

        for either_or_both in current_segments
            .iter()
            .zip_longest(&*head_segments_segments)
        {
            match either_or_both {
                EitherOrBoth::Both(current_segment, head_segment)
                    if current_segment != head_segment =>
                {
                    debug::dump(&format!(
                        "onCodePathSegmentStart {}",
                        self.code_path_segment_arena[*head_segment].id
                    ));

                    CodePathSegment::mark_used(&mut self.code_path_segment_arena, *head_segment);
                    // if self.code_path_segment_arena[*head_segment].reachable {
                    //     self.current_events
                    //         .push(Event::OnCodePathSegmentStart(*head_segment, node));
                    // }
                }
                EitherOrBoth::Right(head_segment) => {
                    debug::dump(&format!(
                        "onCodePathSegmentStart {}",
                        self.code_path_segment_arena[*head_segment].id
                    ));

                    CodePathSegment::mark_used(&mut self.code_path_segment_arena, *head_segment);
                    // if self.code_path_segment_arena[*head_segment].reachable {
                    //     self.current_events
                    //         .push(Event::OnCodePathSegmentStart(*head_segment, node));
                    // }
                }
                _ => (),
            }
        }
    }

    fn leave_from_current_segment(&mut self, _node: Node<'a>) {
        self.code_path_arena[self.code_path()]
            .state
            .current_segments
            .as_ref()
            .map_or_default(|current_segments| current_segments.segments())
            .into_iter()
            .for_each(|current_segment| {
                debug::dump(&format!(
                    "onCodePathSegmentEnd {}",
                    self.code_path_segment_arena[current_segment].id
                ));
                // if self.code_path_segment_arena[current_segment].reachable {
                //     self.current_events
                //         .push(Event::OnCodePathSegmentEnd(current_segment, node));
                // }
            });

        self.code_path_arena[self.active_code_path.unwrap()]
            .state
            .current_segments = Default::default();
    }

    fn preprocess(&mut self, node: Node<'a>) {
        let code_path = self.active_code_path.unwrap();
        let state = &mut self.code_path_arena[code_path].state;
        let parent = node.parent().unwrap();

        match parent.kind() {
            CallExpression => {
                if parent.child_by_field_name("optional_chain").is_some()
                    && node.kind() == Arguments
                    && node.has_non_comment_named_children(SupportedLanguage::Javascript)
                {
                    state.make_optional_right(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            MemberExpression => {
                if parent.child_by_field_name("optional_chain").is_some()
                    && parent.field("property") == node
                {
                    state.make_optional_right(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            SubscriptExpression => {
                if parent.child_by_field_name("optional_chain").is_some()
                    && parent.field("index") == node
                {
                    state.make_optional_right(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            BinaryExpression => {
                if parent.field("right") == node && is_handled_logical_operator(parent) {
                    state.make_logical_right(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            AugmentedAssignmentExpression => {
                if parent.field("right") == node
                    && is_logical_assignment_operator(parent.field("operator").kind())
                {
                    state.make_logical_right(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            IfStatement | TernaryExpression => {
                if parent.field("consequence") == node {
                    state.make_if_consequent(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                } else if parent.child_by_field_name("alternative") == Some(node) {
                    state.make_if_alternate(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            SwitchCase | SwitchDefault => {
                if parent.child_by_field_name("body") == Some(node) {
                    state.make_switch_case_body(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        false,
                        parent.kind() == SwitchDefault,
                    );
                }
            }
            TryStatement => {
                if parent.child_by_field_name("handler") == Some(node) {
                    state.make_catch_block(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                } else if parent.child_by_field_name("finalizer") == Some(node) {
                    state.make_finally_block(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            WhileStatement => {
                if parent.field("condition") == node {
                    state.make_while_test(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        get_boolean_value_if_simple_constant(
                            node.skip_parentheses(),
                            &self.file_contents,
                        ),
                    );
                } else {
                    assert!(parent.field("body") == node);
                    state.make_while_body(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            DoStatement => {
                if parent.field("body") == node {
                    state.make_do_while_body(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                } else {
                    // TODO: I have to be careful about these type of things
                    // because there could always be error nodes "floating around"?
                    assert!(parent.field("condition") == node);
                    state.make_do_while_test(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        get_boolean_value_if_simple_constant(
                            node.skip_parentheses(),
                            &self.file_contents,
                        ),
                    );
                }
            }
            ForStatement => {
                if parent.field("condition") == node && node.kind() != EmptyStatement {
                    state.make_for_test(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        get_boolean_value_if_simple_constant(
                            node.skip_parentheses().skip_nodes_of_type(
                                ExpressionStatement,
                                SupportedLanguage::Javascript,
                            ),
                            &self.file_contents,
                        ),
                    );
                } else if parent.child_by_field_name("increment") == Some(node) {
                    state.make_for_update(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                } else if parent.field("body") == node {
                    state.make_for_body(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        self.current_node.unwrap(),
                    );
                }
            }
            ForInStatement => {
                if parent.field("left") == node {
                    state.make_for_in_of_left(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                } else if parent.field("right") == node {
                    state.make_for_in_of_right(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                } else {
                    assert!(parent.field("body") == node);
                    state.make_for_in_of_body(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        self.current_node.unwrap(),
                    );
                }
            }
            AssignmentPattern | ObjectAssignmentPattern => {
                if parent.field("right") == node {
                    state.push_fork_context(&mut self.fork_context_arena, None);
                    state.fork_bypass_path(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                    state.fork_path(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            _ => (),
        }
    }

    fn process_code_path_to_enter(&mut self, node: Node<'a>) {
        if is_property_definition_value(node) {
            self.start_code_path(node, CodePathOrigin::ClassFieldInitializer);
        }

        match node.kind() {
            Program => {
                self.start_code_path(node, CodePathOrigin::Program);
            }
            FunctionDeclaration
            | GeneratorFunctionDeclaration
            | Function
            | GeneratorFunction
            | ArrowFunction
            | MethodDefinition => {
                self.start_code_path(node, CodePathOrigin::Function);
            }
            ClassStaticBlock => {
                self.start_code_path(node, CodePathOrigin::ClassStaticBlock);
            }
            CallExpression | MemberExpression | SubscriptExpression => {
                if is_outermost_chain_expression(node) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .push_chain_context();
                }
                if node.child_by_field_name("optional_chain").is_some() {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .make_optional_node(&mut self.fork_context_arena);
                }
            }
            BinaryExpression => {
                let operator = node.field("operator").kind();
                if is_handled_logical_operator_str(operator) {
                    let is_forking_as_result = is_forking_by_true_or_false(node);
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .push_choice_context(
                            &mut self.fork_context_arena,
                            match operator {
                                "&&" => ChoiceContextKind::LogicalAnd,
                                "||" => ChoiceContextKind::LogicalOr,
                                "??" => ChoiceContextKind::LogicalNullCoalesce,
                                _ => unreachable!(),
                            },
                            is_forking_as_result,
                        );
                }
            }
            AugmentedAssignmentExpression => {
                let operator = node.field("operator").text(self);
                if is_logical_assignment_operator(&operator) {
                    let is_forking_as_result = is_forking_by_true_or_false(node);
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .push_choice_context(
                            &mut self.fork_context_arena,
                            match operator.strip_suffix('=').unwrap() {
                                "&&" => ChoiceContextKind::LogicalAnd,
                                "||" => ChoiceContextKind::LogicalOr,
                                "??" => ChoiceContextKind::LogicalNullCoalesce,
                                _ => unreachable!(),
                            },
                            is_forking_as_result,
                        );
                }
            }
            TernaryExpression | IfStatement => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .push_choice_context(
                        &mut self.fork_context_arena,
                        ChoiceContextKind::Test,
                        false,
                    );
            }
            SwitchStatement => {
                let label = get_label(node, self).map(Cow::into_owned);
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .push_switch_context(
                        &mut self.fork_context_arena,
                        node.field("body").has_child_of_kind(SwitchCase),
                        label,
                    );
            }
            TryStatement => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .push_try_context(
                        &mut self.fork_context_arena,
                        node.child_by_field_name("finalizer").is_some(),
                    );
            }
            SwitchCase | SwitchDefault => {
                if !node.is_first_non_comment_named_child(SupportedLanguage::Javascript) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .fork_path(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                        );
                }
            }
            WhileStatement | DoStatement | ForStatement | ForInStatement => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .push_loop_context(
                        &mut self.fork_context_arena,
                        node.kind(),
                        get_label(node, &self.file_contents).map(Cow::into_owned),
                    );
            }
            LabeledStatement => {
                if !BREAKABLE_TYPE_PATTERN.is_match(node.field("body").kind()) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .push_break_context(
                            &mut self.fork_context_arena,
                            false,
                            Some(node.field("label").text(&self.file_contents).into_owned()),
                        );
                }
            }
            _ => (),
        }

        self.forward_current_to_head(node);
        debug::dump_state(
            &mut self.code_path_segment_arena,
            node,
            &self.code_path_arena[self.active_code_path.unwrap()].state,
            false,
        );
    }

    fn start_code_path(&mut self, node: Node<'a>, origin: CodePathOrigin) {
        if let Some(code_path) = self.active_code_path {
            self.forward_current_to_head(node);
            debug::dump_state(
                &mut self.code_path_segment_arena,
                node,
                &self.code_path_arena[code_path].state,
                false,
            );
        }

        let upper = self.active_code_path;
        self.code_paths.push(CodePath::new(
            &mut self.code_path_arena,
            &mut self.fork_context_arena,
            &mut self.code_path_segment_arena,
            self.id_generator.next(),
            origin,
            upper,
            OnLooped,
        ));
        self.active_code_path = Some(*self.code_paths.last().unwrap());

        debug::dump(&format!(
            "onCodePathStart {}",
            self.code_path_arena[self.code_path()].id
        ));
        // self.current_events.push(Event::OnCodePathStart(node));
    }

    fn process_code_path_to_exit(&mut self, node: Node<'a>) {
        let mut dont_forward = false;

        match node.kind() {
            IfStatement | TernaryExpression => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .pop_choice_context(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
            }
            BinaryExpression => {
                if is_handled_logical_operator(node) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .pop_choice_context(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                        );
                }
            }
            AugmentedAssignmentExpression => {
                if is_logical_assignment_operator(&node.field("operator").text(self)) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .pop_choice_context(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                        );
                }
            }
            SwitchStatement => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .pop_switch_context(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        self.current_node.unwrap(),
                    );
            }
            SwitchCase | SwitchDefault => {
                if node.child_by_field_name("body").is_none() {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .make_switch_case_body(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                            true,
                            node.kind() == SwitchDefault,
                        );
                }
                if self.fork_context_arena
                    [self.code_path_arena[self.code_path()].state.fork_context]
                    .reachable(&self.code_path_segment_arena)
                {
                    dont_forward = true;
                }
            }
            TryStatement => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .pop_try_context(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
            }
            BreakStatement => {
                self.forward_current_to_head(node);
                let label = node
                    .child_by_field_name("label")
                    .map(|label| label.text(self));
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .make_break(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        label.as_deref(),
                    );
                dont_forward = true;
            }
            ContinueStatement => {
                self.forward_current_to_head(node);
                let label = node
                    .child_by_field_name("label")
                    .map(|label| label.text(self));
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .make_continue(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        self.current_node.unwrap(),
                        label.as_deref(),
                    );
                dont_forward = true;
            }
            ReturnStatement => {
                self.forward_current_to_head(node);
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .make_return(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                dont_forward = true;
            }
            ThrowStatement => {
                self.forward_current_to_head(node);
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .make_throw(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                dont_forward = true;
            }
            Identifier | PropertyIdentifier | ShorthandPropertyIdentifier => {
                if is_identifier_reference(node) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .make_first_throwable_path_in_try_block(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                        );
                    dont_forward = true;
                }
            }
            CallExpression | MemberExpression | SubscriptExpression | NewExpression
            | YieldExpression => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .make_first_throwable_path_in_try_block(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                if is_outermost_chain_expression(node) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .pop_chain_context(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                        );
                }
            }
            WhileStatement | DoStatement | ForStatement | ForInStatement => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .pop_loop_context(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        self.current_node.unwrap(),
                    );
            }
            AssignmentPattern | ObjectAssignmentPattern => {
                self.code_path_arena[self.active_code_path.unwrap()]
                    .state
                    .pop_fork_context(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
            }
            LabeledStatement => {
                if !BREAKABLE_TYPE_PATTERN.is_match(node.field("body").kind()) {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .pop_break_context(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                        );
                }
            }
            // TODO: at least the ParenthesizedExpression here looked like
            // kind of a hack where I could have just updated the test files
            // to have a different expected path (looked correct, just different)?
            SwitchBody | ParenthesizedExpression => {
                dont_forward = true;
            }
            _ => (),
        }

        if !dont_forward {
            self.forward_current_to_head(node);
        }
        debug::dump_state(
            &mut self.code_path_segment_arena,
            node,
            &self.code_path_arena[self.active_code_path.unwrap()].state,
            true,
        );
    }

    fn postprocess(&mut self, node: Node<'a>) {
        match node.kind() {
            Program
            | FunctionDeclaration
            | GeneratorFunctionDeclaration
            | Function
            | GeneratorFunction
            | ArrowFunction
            | MethodDefinition
            | ClassStaticBlock => {
                self.end_code_path(node);
            }
            CallExpression => {
                if node.child_by_field_name("optional_chain").is_some()
                    && get_num_call_expression_arguments(node) == Some(0)
                {
                    self.code_path_arena[self.active_code_path.unwrap()]
                        .state
                        .make_optional_right(
                            &mut self.fork_context_arena,
                            &mut self.code_path_segment_arena,
                        );
                }
            }
            _ => (),
        }

        if is_property_definition_value(node) {
            self.end_code_path(node);
        }
    }

    fn end_code_path(&mut self, node: Node<'a>) {
        self.code_path_arena[self.active_code_path.unwrap()]
            .state
            .make_final(&self.code_path_segment_arena);

        self.leave_from_current_segment(node);

        debug::dump(&format!(
            "onCodePathEnd {}",
            self.code_path_arena[self.code_path()].id
        ));
        // self.current_events
        //     .push(Event::OnCodePathEnd(self.code_path.unwrap(), node));
        debug::dump_dot(
            &self.code_path_segment_arena,
            &self.code_path_arena[self.code_path()],
            &self.file_contents,
        );

        self.active_code_path = self.code_path_arena[self.code_path()].upper;
        if let Some(code_path) = self.active_code_path {
            debug::dump_state(
                &mut self.code_path_segment_arena,
                node,
                &self.code_path_arena[code_path].state,
                true,
            );
        }
    }

    pub fn get_innermost_code_path(&self, node: Node<'a>) -> Id<CodePath<'a>> {
        self.code_paths
            .iter()
            .find(|&&code_path| {
                let code_path = &self.code_path_arena[code_path];
                node.is_same_or_descendant_of(code_path.root_node(&self.code_path_segment_arena))
                    && !code_path.child_code_paths.iter().any(|&child_code_path| {
                        node.is_same_or_descendant_of(
                            self.code_path_arena[child_code_path]
                                .root_node(&self.code_path_segment_arena),
                        )
                    })
            })
            .copied()
            .unwrap()
    }

    pub fn get_segments_that_include_node_exit(
        &self,
        node: Node<'a>,
    ) -> Vec<Id<CodePathSegment<'a>>> {
        let mut segments: Vec<Id<CodePathSegment<'a>>> = Default::default();
        for &code_path in &self.code_paths {
            self.code_path_arena[code_path].traverse_all_segments(
                &self.code_path_segment_arena,
                None,
                |_, segment, _| {
                    if self.code_path_segment_arena[segment].nodes.iter().any(
                        |(enter_or_exit, segment_node)| {
                            *segment_node == node && matches!(enter_or_exit, EnterOrExit::Exit,)
                        },
                    ) {
                        segments.push(segment);
                    }
                },
            );
        }
        segments
    }

    pub fn get_segments_that_include_node_enter(
        &self,
        node: Node<'a>,
    ) -> Vec<Id<CodePathSegment<'a>>> {
        let mut segments: Vec<Id<CodePathSegment<'a>>> = Default::default();
        for &code_path in &self.code_paths {
            self.code_path_arena[code_path].traverse_all_segments(
                &self.code_path_segment_arena,
                None,
                |_, segment, _| {
                    if self.code_path_segment_arena[segment].nodes.iter().any(
                        |(enter_or_exit, segment_node)| {
                            *segment_node == node && matches!(enter_or_exit, EnterOrExit::Enter)
                        },
                    ) {
                        segments.push(segment);
                    }
                },
            );
        }
        segments
    }
}

tid! { impl<'a> TidAble<'a> for CodePathAnalyzer<'a> }

impl<'a> SourceTextProvider<'a> for CodePathAnalyzer<'a> {
    fn node_text(&self, node: Node) -> Cow<'a, str> {
        self.file_contents.node_text(node)
    }

    fn slice(&self, range: ops::Range<usize>) -> Cow<'a, str> {
        self.file_contents.slice(range)
    }
}

impl<'a> FromFileRunContext<'a> for CodePathAnalyzer<'a> {
    fn from_file_run_context(file_run_context: FileRunContext<'a, '_>) -> Self {
        let mut code_path_analyzer = CodePathAnalyzer::new(file_run_context.file_contents);
        walk_tree(file_run_context.tree, &mut code_path_analyzer);
        code_path_analyzer
    }
}

impl<'a> TreeEnterLeaveVisitor<'a> for CodePathAnalyzer<'a> {
    fn enter_node(&mut self, node: Node<'a>) {
        if !node.is_named() || node.kind() == Comment {
            return;
        }

        self.current_node = Some(node);

        if node.parent().is_some() {
            self.preprocess(node);
        }

        self.process_code_path_to_enter(node);

        self.current_node = None;
    }

    fn leave_node(&mut self, node: Node<'a>) {
        if !node.is_named() || node.kind() == Comment {
            return;
        }

        self.current_node = Some(node);

        self.process_code_path_to_exit(node);

        self.postprocess(node);

        self.current_node = None;
    }
}

pub struct OnLooped;

impl OnLooped {
    pub fn on_looped(
        &self,
        arena: &Arena<CodePathSegment>,
        from_segment: Id<CodePathSegment>,
        to_segment: Id<CodePathSegment>,
    ) {
        if arena[from_segment].reachable && arena[to_segment].reachable {
            debug::dump(&format!(
                "onCodePathSegmentLoop {} -> {}",
                arena[from_segment].id, arena[to_segment].id,
            ));
            // current_events.push(Event::OnCodePathSegmentLoop(
            //     from_segment,
            //     to_segment,
            //     current_node,
            // ));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, iter, path::PathBuf, sync::Arc};

    use rstest::rstest;
    use squalid::regex;
    use tree_sitter_lint::{
        instance_provider_factory, rule, ConfigBuilder, ErrorLevel, Rule, RuleConfiguration,
    };

    use super::{super::debug_helpers::make_dot_arrows, *};
    use crate::ProvidedTypes;

    fn get_expected_dot_arrows(source: &str) -> Vec<String> {
        regex!(r#"/\*expected\s+((?:.|[\r\n])+?)\s*\*/"#)
            .captures_iter(source)
            .map(|captures| {
                regex!(r#"\r?\n"#)
                    .replace_all(&captures[1], "\n")
                    .into_owned()
            })
            .collect()
    }

    #[rstest]
    fn test_completed_code_paths(#[files("tests/fixtures/code_path_analysis/*.js")] path: PathBuf) {
        let source = std::fs::read_to_string(path).unwrap();
        let mut expected = get_expected_dot_arrows(&source);

        assert!(!expected.is_empty(), "/*expected */ comments not found.");

        thread_local! {
            static ACTUAL: RefCell<Vec<String>> = Default::default();
        }

        let rule: Arc<dyn Rule> = rule! {
            name => "testing-code-path-analyzer-paths",
            languages => [Javascript],
            listeners => [
                r#"
                  (program) @c
                "# => |node, context| {
                    let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();
                    let dot_arrows = code_path_analyzer
                        .code_paths
                        .iter()
                        .map(|&code_path| {
                            make_dot_arrows(
                                &code_path_analyzer.code_path_segment_arena,
                                &code_path_analyzer.code_path_arena[code_path],
                                None,
                            )
                        })
                        .collect_vec();
                    ACTUAL.with(|actual| {
                        *actual.borrow_mut() = dot_arrows;
                    });
                },
            ],
        };

        let (violations, _) = tree_sitter_lint::run_for_slice(
            source.as_bytes(),
            None,
            "tmp.js",
            ConfigBuilder::default()
                .rule(rule.meta().name.clone())
                .all_standalone_rules([rule.clone()])
                .rule_configurations([RuleConfiguration {
                    name: rule.meta().name.clone(),
                    level: ErrorLevel::Error,
                    options: None,
                }])
                .build()
                .unwrap(),
            tree_sitter_lint::tree_sitter_grep::SupportedLanguageLanguage::Javascript,
            &instance_provider_factory!(ProvidedTypes),
        );

        assert!(violations.is_empty(), "Unexpected linting error in code.");
        ACTUAL.with(|actual| {
            let mut actual = actual.borrow().clone();
            actual.sort();
            expected.sort();

            for (actual, expected) in iter::zip(&*actual, &expected) {
                assert_eq!(actual, expected);
            }
        });
    }
}
