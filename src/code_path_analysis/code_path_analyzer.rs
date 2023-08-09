use std::{borrow::Cow, rc::Rc};

use id_arena::{Arena, Id};
use tree_sitter_lint::{
    tree_sitter::Node, tree_sitter_grep::SupportedLanguage, Event, EventEmitter, FileRunContext,
    NodeExt, SourceTextProvider,
};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{BinaryExpression, CallExpression, SubscriptExpression},
};

use super::{
    code_path::CodePath, code_path_segment::CodePathSegment, fork_context::ForkContext,
    id_generator::IdGenerator,
};

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
            BinaryExpression => {}
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
