use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

pub trait SourceTextProvider<'a> {
    fn get_node_text(&self, node: Node) -> &'a str;
}

impl<'a> SourceTextProvider<'a> for &'a [u8] {
    fn get_node_text(&self, node: Node) -> &'a str {
        node.utf8_text(self).unwrap()
    }
}

impl<'a> SourceTextProvider<'a> for QueryMatchContext<'a> {
    fn get_node_text(&self, node: Node) -> &'a str {
        self.get_node_text(node)
    }
}
