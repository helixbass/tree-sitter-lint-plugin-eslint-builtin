use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

use crate::kind::{
    self, FieldDefinition, Kind, MethodDefinition, ParenthesizedExpression, PropertyIdentifier,
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

pub fn skip_nodes_of_type<'a>(mut node: Node<'a>, kind: Kind) -> Node<'a> {
    while node.kind() == kind {
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
    if name.kind() == PropertyIdentifier
        && context.get_node_text(name) == "constructor"
        && !is_class_member_static(node, context)
    {
        return MethodDefinitionKind::Constructor;
    }
    if name.kind() == kind::String
        && string_node_equals(name, "constructor", context)
        && !is_class_member_static(node, context)
    {
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

fn string_node_equals(node: Node, value: &str, context: &QueryMatchContext) -> bool {
    assert_kind!(node, kind::String);
    let node_text = context.get_node_text(node);
    &node_text[1..node_text.len() - 1] == value
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

pub enum Number {
    NaN,
    Integer(u64),
    Float(f64),
}

impl From<&str> for Number {
    fn from(value: &str) -> Self {
        if is_hex_literal(value) {
            u64::from_str_radix(&value[2..], 16).map_or(Self::NaN, |parsed| Self::Integer(parsed))
        } else if is_octal_literal(value) {
            u64::from_str_radix(&value[2..], 8).map_or(Self::NaN, |parsed| Self::Integer(parsed))
        } else if is_binary_literal(value) {
            u64::from_str_radix(&value[2..], 2).map_or(Self::NaN, |parsed| Self::Integer(parsed))
        } else if is_bigint_literal(value) {
            value[..value.len() - 1]
                .parse::<u64>()
                .map_or(Self::NaN, Self::Integer)
        } else {
            value
                .parse::<u64>()
                .map(Self::Integer)
                .unwrap_or_else(|_| value.parse::<f64>().map_or(Self::NaN, Self::Float))
        }
    }
}

fn is_bigint_literal(number_node_text: &str) -> bool {
    number_node_text.ends_with("n")
}

fn is_hex_literal(number_node_text: &str) -> bool {
    number_node_text.starts_with("0x") || number_node_text.starts_with("0X")
}

fn is_binary_literal(number_node_text: &str) -> bool {
    number_node_text.starts_with("0b") || number_node_text.starts_with("0B")
}

fn is_octal_literal(number_node_text: &str) -> bool {
    number_node_text.starts_with("0o") || number_node_text.starts_with("0O")
}

pub fn get_number_literal_string_value(node: Node, context: &QueryMatchContext) -> String {
    assert_kind!(node, "number");

    match Number::from(context.get_node_text(node)) {
        Number::NaN => unreachable!("I don't know if this should be possible?"),
        Number::Integer(number) => number.to_string(),
        Number::Float(number) => number.to_string(),
    }
}
