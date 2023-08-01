use std::borrow::Cow;

use tree_sitter_lint::{regex, tree_sitter::Node, QueryMatchContext};

use crate::{
    kind::{
        self, BinaryExpression, Comment, FieldDefinition, ForInStatement, Kind, MemberExpression,
        MethodDefinition, Pair, ParenthesizedExpression, PropertyIdentifier,
        ShorthandPropertyIdentifier, UnaryExpression,
    },
    return_default_if_none,
    text::SourceTextProvider,
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

pub fn is_for_of<'a>(node: Node, source_text_provider: &impl SourceTextProvider<'a>) -> bool {
    assert_kind!(node, ForInStatement);
    matches!(
        node.child_by_field_name("operator"),
        Some(child) if source_text_provider.get_node_text(child) == "of"
    )
}

pub fn is_for_of_await<'a>(node: Node, source_text_provider: &impl SourceTextProvider<'a>) -> bool {
    assert_kind!(node, ForInStatement);
    is_for_of(node, source_text_provider)
        && matches!(
            // TODO: I can't do stuff like this because comments could be anywhere
            node.child(1),
            Some(child) if source_text_provider.get_node_text(child) == "await"
        )
}

#[allow(dead_code)]
pub fn skip_parenthesized_expressions(node: Node) -> Node {
    skip_nodes_of_type(node, ParenthesizedExpression)
}

pub fn skip_nodes_of_type(mut node: Node, kind: Kind) -> Node {
    while node.kind() == kind {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return node;
        }
        while cursor.node().kind() == Comment || !cursor.node().is_named() {
            if !cursor.goto_next_sibling() {
                return node;
            }
        }
        node = cursor.node();
    }
    node
}

pub fn skip_nodes_of_types<'a>(mut node: Node<'a>, kinds: &[Kind]) -> Node<'a> {
    while kinds.contains(&node.kind()) {
        let mut cursor = node.walk();
        if !cursor.goto_first_child() {
            return node;
        }
        while cursor.node().kind() == Comment || !cursor.node().is_named() {
            if !cursor.goto_next_sibling() {
                return node;
            }
        }
        node = cursor.node();
    }
    node
}

fn get_previous_non_comment_sibling(mut node: Node) -> Option<Node> {
    node = node.prev_sibling()?;
    while node.kind() == Comment {
        node = node.prev_sibling()?;
    }
    Some(node)
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
    match get_previous_non_comment_sibling(name)
        .map(|prev_sibling| context.get_node_text(prev_sibling))
        .as_deref()
    {
        Some("get") => MethodDefinitionKind::Get,
        Some("set") => MethodDefinitionKind::Set,
        _ => MethodDefinitionKind::Method,
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ObjectPropertyKind {
    Init,
    Get,
    Set,
}

pub fn get_object_property_kind(node: Node, context: &QueryMatchContext) -> ObjectPropertyKind {
    match node.kind() {
        Pair | ShorthandPropertyIdentifier => ObjectPropertyKind::Init,
        MethodDefinition => {
            let mut cursor = node.walk();
            assert!(cursor.goto_first_child());
            loop {
                if cursor.field_name() == Some("name") {
                    return ObjectPropertyKind::Init;
                }
                match &*context.get_node_text(cursor.node()) {
                    "get" => return ObjectPropertyKind::Get,
                    "set" => return ObjectPropertyKind::Set,
                    _ => (),
                }
                assert!(cursor.goto_next_sibling());
            }
        }
        _ => unreachable!(),
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
        let value = regex!(r#"_"#).replace_all(value, "");
        if is_hex_literal(&value) {
            u64::from_str_radix(&value[2..], 16).map_or(Self::NaN, Self::Integer)
        } else if is_octal_literal(&value) {
            u64::from_str_radix(&value[2..], 8).map_or(Self::NaN, Self::Integer)
        } else if is_binary_literal(&value) {
            u64::from_str_radix(&value[2..], 2).map_or(Self::NaN, Self::Integer)
        } else if is_bigint_literal(&value) {
            value[..value.len() - 1]
                .parse::<u64>()
                .map_or(Self::NaN, Self::Integer)
        } else if let Some(value) = value.strip_prefix('0') {
            u64::from_str_radix(value, 8).map_or(Self::NaN, Self::Integer)
        } else {
            value
                .parse::<u64>()
                .map(Self::Integer)
                .unwrap_or_else(|_| value.parse::<f64>().map_or(Self::NaN, Self::Float))
        }
    }
}

fn is_bigint_literal(number_node_text: &str) -> bool {
    number_node_text.ends_with('n')
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

    match Number::from(&*context.get_node_text(node)) {
        Number::NaN => unreachable!("I don't know if this should be possible?"),
        Number::Integer(number) => number.to_string(),
        Number::Float(number) => number.to_string(),
    }
}

pub fn is_logical_and<'a>(node: Node, source_text_provider: &impl SourceTextProvider<'a>) -> bool {
    is_binary_expression_with_operator(node, "&&", source_text_provider)
}

pub fn is_binary_expression_with_operator<'a>(
    node: Node,
    operator: &str,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> bool {
    node.kind() == BinaryExpression
        && get_binary_expression_operator(node, source_text_provider) == operator
}

pub fn is_binary_expression_with_one_of_operators<'a>(
    node: Node,
    operators: &[impl AsRef<str>],
    source_text_provider: &impl SourceTextProvider<'a>,
) -> bool {
    if node.kind() != BinaryExpression {
        return false;
    }
    let operator_text = get_binary_expression_operator(node, source_text_provider);
    operators
        .iter()
        .any(|operator| operator_text == operator.as_ref())
}

pub fn is_chain_expression(mut node: Node) -> bool {
    loop {
        if node.kind() != MemberExpression {
            return false;
        }
        if node.child_by_field_name("optional_chain").is_some() {
            return true;
        }
        node = return_default_if_none!(node.parent());
    }
}

pub fn get_binary_expression_operator<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Cow<'a, str> {
    assert_kind!(node, BinaryExpression);
    source_text_provider.get_node_text(node.child_by_field_name("operator").unwrap())
}

pub fn get_unary_expression_operator<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Cow<'a, str> {
    assert_kind!(node, UnaryExpression);
    source_text_provider.get_node_text(node.child_by_field_name("operator").unwrap())
}

pub fn get_first_child_of_kind(node: Node, kind: Kind) -> Node {
    let mut cursor = node.walk();
    let ret = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind)
        .unwrap();
    ret
}
