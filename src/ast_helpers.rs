use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

#[macro_export]
macro_rules! assert_kind {
    ($node:expr, $kind:expr) => {
        assert!(
            $node.kind() == $kind,
            "Expected kind {:?}, got: {:?}",
            $node.kind(),
            $kind
        );
    };
}

pub fn is_for_of_await(node: Node, context: &QueryMatchContext) -> bool {
    // assert_kind!(node, ForInStatement);
    matches!(
        node.child_by_field_name("operator"),
        Some(child) if context.get_node_text(child) == "of"
    ) && matches!(
        node.child(1),
        Some(child) if context.get_node_text(child) == "await"
    )
}
