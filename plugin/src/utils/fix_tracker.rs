use std::cmp;

use tree_sitter_lint::{
    tree_sitter::{Node, Range},
    Fixer, QueryMatchContext,
};

use super::ast_utils;

pub struct FixTracker<'a, 'b, 'c, 'd> {
    fixer: &'a mut Fixer,
    context: &'b QueryMatchContext<'c, 'd>,
    retained_range: Option<Range>,
}

impl<'a, 'b, 'c, 'd> FixTracker<'a, 'b, 'c, 'd> {
    pub fn new(fixer: &'a mut Fixer, context: &'b QueryMatchContext<'c, 'd>) -> Self {
        Self {
            fixer,
            context,
            retained_range: Default::default(),
        }
    }

    pub fn retain_range(&mut self, range: Range) -> &mut Self {
        self.retained_range = Some(range);
        self
    }

    pub fn retain_enclosing_function(&mut self, node: Node) -> &mut Self {
        let function_node = ast_utils::get_upper_function(node);

        self.retain_range(function_node.map_or_else(
            || self.context.file_run_context.tree.root_node().range(),
            |function_node| function_node.range(),
        ))
    }

    pub fn replace_text_range(&mut self, range: Range, text: &str) {
        let actual_range = self.retained_range.map_or(range, |retained_range| Range {
            start_byte: cmp::min(retained_range.start_byte, range.start_byte),
            end_byte: cmp::max(retained_range.end_byte, range.end_byte),
            start_point: if retained_range.start_byte < range.start_byte {
                retained_range.start_point
            } else {
                range.start_point
            },
            end_point: if retained_range.start_byte < range.start_byte {
                range.end_point
            } else {
                retained_range.end_point
            },
        });
    }

    pub fn remove(&mut self, node_or_token: Node) {
        self.replace_text_range(node_or_token.range(), "");
    }
}
