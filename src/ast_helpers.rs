use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

use crate::kind::{
    FieldDefinition, Kind, MethodDefinition, ParenthesizedExpression, PropertyIdentifier,
};

#[macro_export]
macro_rules! assert_kind {
    ($node:expr, $kind:expr) => {
        assert!(
            $node.kind() == $kind,
            "Expected kind {:?}, got: {:?}",
            $kind,
            $node.kind()
        );
    };
}

#[macro_export]
macro_rules! assert_one_of_kinds {
    ($node:expr, $kinds:expr) => {
        assert!(
            $kinds.iter().any(|kind| $node.kind() == *kind),
            "Expected kind {:?}, got: {:?}",
            $kinds,
            $node.kind()
        );
    };
}

#[macro_export]
macro_rules! return_default_if_false {
    ($expr:expr) => {
        if !$expr {
            return Default::default();
        }
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

#[allow(dead_code)]
pub fn skip_parenthesized_expressions(mut node: Node) -> Node {
    while node.kind() == ParenthesizedExpression {
        node = node.named_child(0).unwrap();
    }
    node
}

pub fn skip_nodes_of_types<'a>(mut node: Node<'a>, kinds: &[Kind]) -> Node<'a> {
    while kinds.contains(&node.kind()) {
        node = node.named_child(0).unwrap();
    }
    node
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MethodDefinitionKind {
    Method,
    Constructor,
    Get,
    Set,
}

pub fn get_method_definition_kind(node: Node, context: &QueryMatchContext) -> MethodDefinitionKind {
    assert_kind!(node, MethodDefinition);
    let name = node.child_by_field_name("name").unwrap();
    if name.kind() == PropertyIdentifier && context.get_node_text(name) == "constructor" {
        return MethodDefinitionKind::Constructor;
    }
    match name
        .prev_sibling()
        .map(|prev_sibling| context.get_node_text(prev_sibling))
    {
        Some("get") => MethodDefinitionKind::Get,
        Some("set") => MethodDefinitionKind::Set,
        _ => MethodDefinitionKind::Method,
    }
}

pub fn is_class_member_static(node: Node, context: &QueryMatchContext) -> bool {
    assert_one_of_kinds!(node, [MethodDefinition, FieldDefinition]);

    let mut cursor = node.walk();
    return_default_if_false!(cursor.goto_first_child());
    while cursor.field_name() == Some("decorator") {
        return_default_if_false!(cursor.goto_next_sibling());
    }
    context.get_node_text(cursor.node()) == "static"
}
