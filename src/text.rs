use std::borrow::Cow;

use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

pub trait SourceTextProvider<'a> {
    fn get_node_text(&self, node: Node) -> Cow<'a, str>;
}

impl<'a> SourceTextProvider<'a> for &'a [u8] {
    fn get_node_text(&self, node: Node) -> Cow<'a, str> {
        node.utf8_text(self).unwrap().into()
    }
}

impl<'a> SourceTextProvider<'a> for QueryMatchContext<'a> {
    fn get_node_text(&self, node: Node) -> Cow<'a, str> {
        self.get_node_text(node)
    }
}