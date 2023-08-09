use std::{borrow::Cow, rc::Rc};

use id_arena::{Arena, Id};
use itertools::{EitherOrBoth, Itertools};
use squalid::OptionExt;
use tree_sitter_lint::{
    tree_sitter::Node, tree_sitter_grep::SupportedLanguage, EventEmitter, FileRunContext, NodeExt,
    SourceTextProvider,
};

use crate::{
    ast_helpers::{get_binary_expression_operator, NodeExtJs, Number},
    kind::{
        self, is_literal_kind, AssignmentPattern, AugmentedAssignmentExpression, BinaryExpression,
        CallExpression, DoStatement, FieldDefinition, ForInStatement, ForStatement, IfStatement,
        ObjectAssignmentPattern, SubscriptExpression, SwitchCase, SwitchDefault, TernaryExpression,
        TryStatement, WhileStatement,
    },
};

use super::{
    code_path::{CodePath, CodePathOrigin},
    code_path_segment::CodePathSegment,
    fork_context::ForkContext,
    id_generator::IdGenerator,
};

fn is_property_definition_value(node: Node) -> bool {
    let parent = node.parent();

    parent.matches(|parent| {
        parent.kind() == FieldDefinition && parent.child_by_field_name("value") == Some(node)
    })
}

fn is_handled_logical_operator(operator: &str) -> bool {
    matches!(operator, "&&" | "||" | "??")
}

fn is_logical_assignment_operator(operator: &str) -> bool {
    matches!(operator, "&&=" | "||=" | "??=")
}

fn get_boolean_value_if_simple_constant<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Option<bool> {
    is_literal_kind(node.kind()).then(|| match node.kind() {
        kind::String => !node.text(source_text_provider).is_empty(),
        kind::Number => Number::from(&*node.text(source_text_provider)).is_truthy(),
        Regex => true,
        Null => false,
    })
}

pub struct CodePathAnalyzer<'a, 'b> {
    code_path: Option<Id<CodePath>>,
    id_generator: Rc<IdGenerator>,
    current_node: Option<Node<'a>>,
    code_path_arena: Arena<CodePath>,
    fork_context_arena: Arena<ForkContext>,
    code_path_segment_arena: Arena<CodePathSegment>,
    file_run_context: FileRunContext<'a, 'b>,
    pending_events: Vec<Event<'a>>,
}

impl<'a, 'b> CodePathAnalyzer<'a, 'b> {
    pub fn new(file_run_context: FileRunContext<'a, 'b>) -> Self {
        Self {
            code_path: Default::default(),
            id_generator: Rc::new(IdGenerator::new("s")),
            current_node: Default::default(),
            code_path_arena: Default::default(),
            fork_context_arena: Default::default(),
            code_path_segment_arena: Default::default(),
            file_run_context,
            pending_events: Default::default(),
        }
    }

    fn forward_current_to_head(&mut self, node: Node<'a>) {
        let code_path = self.code_path.unwrap();
        let state = &mut self.code_path_arena[code_path].state;
        let current_segments = state.current_segments.clone();
        let head_segments = state.head_segments(&self.fork_context_arena);

        for either_or_both in current_segments.iter().zip_longest(head_segments) {
            match either_or_both {
                EitherOrBoth::Both(current_segment, head_segment)
                    if current_segment != head_segment =>
                {
                    // debug.dump(`onCodePathSegmentEnd ${currentSegment.id}`);

                    if self.code_path_segment_arena[*current_segment].reachable {
                        self.pending_events
                            .push(Event::OnCodePathSegmentEnd(*current_segment, node));
                    }
                }
                EitherOrBoth::Left(current_segment) => {
                    // debug.dump(`onCodePathSegmentEnd ${currentSegment.id}`);

                    if self.code_path_segment_arena[*current_segment].reachable {
                        self.pending_events
                            .push(Event::OnCodePathSegmentEnd(*current_segment, node));
                    }
                }
                _ => (),
            }
        }

        state.current_segments = head_segments.to_owned();

        for either_or_both in current_segments.iter().zip_longest(head_segments) {
            match either_or_both {
                EitherOrBoth::Both(current_segment, head_segment)
                    if current_segment != head_segment =>
                {
                    // debug.dump(`onCodePathSegmentStart ${headSegment.id}`);

                    CodePathSegment::mark_used(&mut self.code_path_segment_arena, *head_segment);
                    if self.code_path_segment_arena[*head_segment].reachable {
                        self.pending_events
                            .push(Event::OnCodePathSegmentStart(*head_segment, node));
                    }
                }
                EitherOrBoth::Right(head_segment) => {
                    // debug.dump(`onCodePathSegmentStart ${headSegment.id}`);

                    CodePathSegment::mark_used(&mut self.code_path_segment_arena, *head_segment);
                    if self.code_path_segment_arena[*head_segment].reachable {
                        self.pending_events
                            .push(Event::OnCodePathSegmentStart(*head_segment, node));
                    }
                }
                _ => (),
            }
        }
    }

    fn preprocess(&mut self, node: Node<'a>) {
        let code_path = self.code_path.unwrap();
        let state = &mut self.code_path_arena[code_path].state;
        let parent = node.parent().unwrap();

        match parent.kind() {
            CallExpression => {
                if parent.child_by_field_name("optional_chain").is_some()
                    && node.is_first_call_expression_argument(parent)
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
                if parent.field("right") == node
                    && is_handled_logical_operator(&get_binary_expression_operator(
                        parent,
                        &self.file_run_context,
                    ))
                {
                    state.make_logical_right(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                    );
                }
            }
            AugmentedAssignmentExpression => {
                if parent.field("right") == node
                    && is_logical_assignment_operator(
                        &parent.field("operator").text(&self.file_run_context),
                    )
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
                if parent.first_non_comment_named_child() == node {
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
                        get_boolean_value_if_simple_constant(node, &self.file_run_context),
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
                    assert!(parent.field("condition") == node);
                    state.make_do_while_test(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        get_boolean_value_if_simple_constant(node, &self.file_run_context),
                    );
                }
            }
            ForStatement => {
                if parent.field("condition") == node {
                    state.make_for_test(
                        &mut self.fork_context_arena,
                        &mut self.code_path_segment_arena,
                        get_boolean_value_if_simple_constant(node, &self.file_run_context),
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
                        &mut self.pending_events,
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
                        &mut self.pending_events,
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
        let state = self
            .code_path
            .map(|code_path| &mut self.code_path_arena[code_path].state);
        let parent = node.parent();

        if is_property_definition_value(node) {
            self.start_code_path(node, CodePathOrigin::ClassFieldInitializer);
        }
    }

    fn start_code_path(&mut self, node: Node<'a>, origin: CodePathOrigin) {
        if let Some(code_path) = self.code_path {
            self.forward_current_to_head(node);
            // debug.dumpState(node, state, false);
        }

        self.code_path = Some(CodePath::new(
            &mut self.code_path_arena,
            &mut self.fork_context_arena,
            &mut self.code_path_segment_arena,
            self.id_generator.next(),
            origin,
            self.code_path,
            OnLooped,
        ));
    }
}

pub struct OnLooped;

impl OnLooped {
    pub fn on_looped<'a>(
        &self,
        arena: &Arena<CodePathSegment>,
        current_node: Node<'a>,
        pending_events: &mut Vec<Event<'a>>,
        from_segment: Id<CodePathSegment>,
        to_segment: Id<CodePathSegment>,
    ) {
        if arena[from_segment].reachable && arena[to_segment].reachable {
            // debug.dump(`onCodePathSegmentLoop ${fromSegment.id} -> ${toSegment.id}`);
            pending_events.push(Event::OnCodePathSegmentLoop(
                from_segment,
                to_segment,
                current_node,
            ));
        }
    }
}

impl<'a, 'b> EventEmitter<'a> for CodePathAnalyzer<'a, 'b> {
    fn name(&self) -> String {
        "code-path-analyzer".to_owned()
    }

    fn languages(&self) -> Vec<SupportedLanguage> {
        vec![SupportedLanguage::Javascript]
    }

    fn enter_node(&mut self, node: Node<'a>) -> Option<Vec<tree_sitter_lint::Event>> {
        self.current_node = Some(node);

        if node.parent().is_some() {
            self.preprocess(node);
        }

        self.process_code_path_to_enter(node);

        unimplemented!()
    }

    fn exit_node(&mut self, node: Node<'a>) -> Option<Vec<tree_sitter_lint::Event>> {
        todo!()
    }
}

impl<'a, 'b> SourceTextProvider<'a> for CodePathAnalyzer<'a, 'b> {
    fn node_text(&self, node: Node) -> Cow<'a, str> {
        self.file_run_context.node_text(node)
    }
}

pub enum Event<'a> {
    OnCodePathSegmentStart(Id<CodePathSegment>, Node<'a>),
    OnCodePathSegmentEnd(Id<CodePathSegment>, Node<'a>),
    OnCodePathSegmentLoop(Id<CodePathSegment>, Id<CodePathSegment>, Node<'a>),
}
