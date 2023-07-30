use const_format::formatcp;
use once_cell::sync::Lazy;
use regex::Regex;
use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

use crate::{
    assert_kind,
    kind::{
        self, ArrowFunction, FieldDefinition, Function, FunctionDeclaration, Identifier,
        MemberExpression, MethodDefinition, Null, Number, Pair, SubscriptExpression,
        TemplateString,
    },
};

static any_function_pattern: Lazy<Regex> = Lazy::new(|| {
    Regex::new(formatcp!(
        r#"^(?:{FunctionDeclaration}|{Function}|{ArrowFunction})$"#
    ))
    .unwrap()
});

pub fn is_function(node: Node) -> bool {
    any_function_pattern.is_match(node.kind())
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

fn get_number_literal_string_value<'a>(node: Node, context: &'a QueryMatchContext) -> &'a str {
    assert_kind!(node, "number");

    let node_text = context.get_node_text(node);
    if is_bigint_literal(node_text) {
        &node_text[..node_text.len() - 1]
    } else if is_hex_literal(node_text) {
        unimplemented!()
    } else if is_binary_literal(node_text) {
        unimplemented!()
    } else if is_octal_literal(node_text) {
        unimplemented!()
    } else {
        node_text
    }
}

fn get_static_string_value<'a>(node: Node, context: &'a QueryMatchContext) -> Option<&'a str> {
    match node.kind() {
        Number => Some(get_number_literal_string_value(node, context)),
        kind::Regex => Some(context.get_node_text(node)),
        kind::String => {
            let node_text = context.get_node_text(node);
            // TODO: this doesn't handle things like hex/unicode escapes
            Some(&node_text[1..node_text.len() - 1])
        }
        Null => Some("null"),
        TemplateString => {
            (!context.has_named_child_of_kind(node, "template_substitution")).then(|| {
                let node_text = context.get_node_text(node);
                // TODO: this doesn't handle things like hex/unicode escapes
                &node_text[1..node_text.len() - 1]
            })
        }
        _ => None,
    }
}

pub fn get_static_property_name<'a>(node: Node, context: &'a QueryMatchContext) -> Option<&'a str> {
    let prop = match node.kind() {
        Pair => node.child_by_field_name("key"),
        FieldDefinition | MemberExpression => node.child_by_field_name("property"),
        MethodDefinition => node.child_by_field_name("name"),
        SubscriptExpression => node.child_by_field_name("index"),
        _ => None,
    }?;

    if prop.kind() == Identifier && node.kind() != SubscriptExpression {
        return Some(context.get_node_text(prop));
    }

    get_static_string_value(prop, context)
}
