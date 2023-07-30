use std::borrow::Cow;

use const_format::formatcp;
use once_cell::sync::Lazy;
use regex::Regex;
use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

use crate::{
    ast_helpers::{get_number_literal_string_value, skip_nodes_of_type},
    kind::{
        self, ArrowFunction, ComputedPropertyName, FieldDefinition, Function, FunctionDeclaration,
        Identifier, MemberExpression, MethodDefinition, Null, Number, Pair, PropertyIdentifier,
        SubscriptExpression, TemplateString,
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

fn get_static_string_value<'a>(node: Node, context: &'a QueryMatchContext) -> Option<Cow<'a, str>> {
    match node.kind() {
        Number => Some(get_number_literal_string_value(node, context).into()),
        kind::Regex => Some(context.get_node_text(node).into()),
        kind::String => {
            let node_text = context.get_node_text(node);
            // TODO: this doesn't handle things like hex/unicode escapes
            Some((&node_text[1..node_text.len() - 1]).into())
        }
        Null => Some("null".into()),
        TemplateString => {
            (!context.has_named_child_of_kind(node, "template_substitution")).then(|| {
                let node_text = context.get_node_text(node);
                // TODO: this doesn't handle things like hex/unicode escapes
                (&node_text[1..node_text.len() - 1]).into()
            })
        }
        _ => None,
    }
}

pub fn get_static_property_name<'a>(
    node: Node,
    context: &'a QueryMatchContext,
) -> Option<Cow<'a, str>> {
    let prop = match node.kind() {
        Pair => node.child_by_field_name("key"),
        FieldDefinition | MemberExpression => node.child_by_field_name("property"),
        MethodDefinition => node.child_by_field_name("name"),
        SubscriptExpression => node.child_by_field_name("index"),
        _ => None,
    }?;

    if matches!(prop.kind(), Identifier | PropertyIdentifier) && node.kind() != SubscriptExpression
    {
        return Some(context.get_node_text(prop).into());
    }

    get_static_string_value(skip_nodes_of_type(prop, ComputedPropertyName), context)
}
