use std::{borrow::Cow, rc::Rc};

use id_arena::{Arena, Id};
use tree_sitter_lint::{
    tree_sitter::Node, tree_sitter_grep::SupportedLanguage, Event, EventEmitter, FileRunContext,
    NodeExt, SourceTextProvider,
};

use crate::{
    ast_helpers::{get_binary_expression_operator, NodeExtJs, Number},
    kind::{
        self, is_literal_kind, AugmentedAssignmentExpression, BinaryExpression, CallExpression,
        DoStatement, ForInStatement, ForStatement, IfStatement, SubscriptExpression, SwitchCase,
        SwitchDefault, TernaryExpression, TryStatement, WhileStatement,
    },
};

use super::{
    code_path::CodePath, code_path_segment::CodePathSegment, fork_context::ForkContext,
    id_generator::IdGenerator,
};

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
                    );
                }
            }
            ForInStatement => {}
            _ => (),
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

    fn enter_node(&mut self, node: Node<'a>) -> Option<Vec<Event>> {
        self.current_node = Some(node);

        if node.parent().is_some() {
            self.preprocess(node);
        }

        unimplemented!()
    }

    fn exit_node(&mut self, node: Node<'a>) -> Option<Vec<Event>> {
        todo!()
    }
}

impl<'a, 'b> SourceTextProvider<'a> for CodePathAnalyzer<'a, 'b> {
    fn node_text(&self, node: Node) -> Cow<'a, str> {
        self.file_run_context.node_text(node)
    }
}
