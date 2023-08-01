use std::borrow::Cow;

use const_format::formatcp;
use once_cell::sync::Lazy;
use regex::Regex;
use squalid::{CowStrExt, OptionExt};
use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

use crate::{
    ast_helpers::{
        get_binary_expression_operator, get_number_literal_string_value, is_chain_expression,
        skip_nodes_of_type,
    },
    kind::{
        self, ArrowFunction, AssignmentExpression, AugmentedAssignmentExpression, AwaitExpression,
        BinaryExpression, CallExpression, ComputedPropertyName, FieldDefinition, Function,
        FunctionDeclaration, Identifier, Kind, MemberExpression, MethodDefinition, NewExpression,
        Null, Number, Pair, ParenthesizedExpression, PropertyIdentifier, SequenceExpression,
        SubscriptExpression, TemplateString, TernaryExpression, UnaryExpression, UpdateExpression,
        YieldExpression,
    },
    text::SourceTextProvider,
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
        kind::Regex => Some(context.get_node_text(node)),
        kind::String => {
            let node_text = context.get_node_text(node);
            // TODO: this doesn't handle things like hex/unicode escapes
            Some(node_text.sliced(|node_text| &node_text[1..node_text.len() - 1]))
        }
        Null => Some("null".into()),
        TemplateString => {
            (!context.has_named_child_of_kind(node, "template_substitution")).then(|| {
                let node_text = context.get_node_text(node);
                // TODO: this doesn't handle things like hex/unicode escapes
                node_text.sliced(|node_text| &node_text[1..node_text.len() - 1])
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
        return Some(context.get_node_text(prop));
    }

    get_static_string_value(skip_nodes_of_type(prop, ComputedPropertyName), context)
}

pub fn is_parenthesised(node: Node) -> bool {
    node.kind() == ParenthesizedExpression
        || node
            .parent()
            .matches(|parent| parent.kind() == ParenthesizedExpression)
}

pub fn equal_tokens(left: Node, right: Node, context: &QueryMatchContext) -> bool {
    let mut tokens_l = context.get_tokens(left);
    let mut tokens_r = context.get_tokens(right);

    loop {
        match (tokens_l.next(), tokens_r.next()) {
            (Some(token_l), Some(token_r)) => {
                if token_l.kind_id() != token_r.kind_id()
                    || context.get_node_text(token_l) != context.get_node_text(token_r)
                {
                    return false;
                }
            }
            (None, None) => return true,
            _ => return false,
        }
    }
}

pub fn is_coalesce_expression<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> bool {
    node.kind() == BinaryExpression
        && get_binary_expression_operator(node, source_text_provider) == "??"
}

pub fn get_precedence<'a>(node: Node, source_text_provider: &impl SourceTextProvider<'a>) -> u32 {
    _get_precedence(
        node.kind(),
        (node.kind() == BinaryExpression)
            .then(|| get_binary_expression_operator(node, source_text_provider)),
        (node.kind() == MemberExpression).then(|| is_chain_expression(node)),
    )
}

fn _get_precedence(
    kind: Kind,
    binary_expression_operator: Option<Cow<'_, str>>,
    member_expression_is_chain_expression: Option<bool>,
) -> u32 {
    match kind {
        SequenceExpression => 0,
        AssignmentExpression | AugmentedAssignmentExpression | ArrowFunction | YieldExpression => 1,
        TernaryExpression => 3,
        BinaryExpression => match &*binary_expression_operator.unwrap() {
            "||" | "??" => 4,
            "&&" => 5,
            "|" => 6,
            "^" => 7,
            "&" => 8,
            "==" | "!=" | "===" | "!==" => 9,
            "<" | "<=" | ">" | ">=" | "in" | "instanceof" => 10,
            "<<" | ">>" | ">>>" => 11,
            "+" | "-" => 12,
            "*" | "/" | "%" => 13,
            "**" => 15,
            _ => unreachable!("maybe?"),
        },
        UnaryExpression | AwaitExpression => 16,
        UpdateExpression => 17,
        CallExpression => 18,
        MemberExpression if member_expression_is_chain_expression.unwrap_or_default() => 18,
        NewExpression => 19,
        _ => 20,
    }
}

pub fn get_kind_precedence(kind: Kind) -> u32 {
    assert!(
        kind != BinaryExpression,
        "Use get_binary_expression_operator_precedence()"
    );
    _get_precedence(kind, None, None)
}

pub fn get_binary_expression_operator_precedence<'a>(operator: impl Into<Cow<'a, str>>) -> u32 {
    let operator = operator.into();
    _get_precedence(BinaryExpression, Some(operator), None)
}

pub enum NodeOrKind<'a> {
    Node(Node<'a>),
    Kind(Kind),
}

impl<'a> From<Node<'a>> for NodeOrKind<'a> {
    fn from(value: Node<'a>) -> Self {
        Self::Node(value)
    }
}

impl<'a> From<Kind> for NodeOrKind<'a> {
    fn from(value: Kind) -> Self {
        Self::Kind(value)
    }
}

pub fn get_parenthesised_text<'a>(context: &'a QueryMatchContext, mut node: Node) -> Cow<'a, str> {
    loop {
        let parent = node.parent();
        if let Some(parent) = parent.filter(|parent| parent.kind() == ParenthesizedExpression) {
            node = parent;
        } else {
            break;
        }
    }
    context.get_node_text(node)
}
