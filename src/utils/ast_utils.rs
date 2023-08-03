use std::borrow::Cow;

use const_format::formatcp;
use once_cell::sync::Lazy;
use regex::Regex;
use squalid::{CowStrExt, EverythingExt, OptionExt};
use tree_sitter_lint::{
    tree_sitter::{Node, Range},
    NodeExt, QueryMatchContext,
};

use crate::{
    assert_kind,
    ast_helpers::{
        get_binary_expression_operator, get_first_non_comment_child, get_method_definition_kind,
        get_number_literal_string_value, get_prev_non_comment_sibling, is_chain_expression,
        skip_nodes_of_type, MethodDefinitionKind, NodeExtJs,
    },
    kind::{
        self, ArrowFunction, AssignmentExpression, AugmentedAssignmentExpression, AwaitExpression,
        BinaryExpression, CallExpression, ComputedPropertyName, Decorator, FieldDefinition,
        Function, FunctionDeclaration, GeneratorFunction, GeneratorFunctionDeclaration, Identifier,
        Kind, MemberExpression, MethodDefinition, NewExpression, Null, Number, Pair, PairPattern,
        ParenthesizedExpression, PrivatePropertyIdentifier, PropertyIdentifier, SequenceExpression,
        ShorthandPropertyIdentifierPattern, SubscriptExpression, TemplateString, TernaryExpression,
        UnaryExpression, UpdateExpression, YieldExpression,
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

static any_loop_pattern: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(?:do|for|for_in|while)_statement$"#).unwrap());

pub fn is_loop(node: Node) -> bool {
    any_loop_pattern.is_match(node.kind())
}

pub fn get_static_string_value<'a>(
    node: Node,
    context: &QueryMatchContext<'a>,
) -> Option<Cow<'a, str>> {
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
    context: &QueryMatchContext<'a>,
) -> Option<Cow<'a, str>> {
    let prop = match node.kind() {
        Pair | PairPattern => node.child_by_field_name("key"),
        FieldDefinition | MemberExpression => node.child_by_field_name("property"),
        MethodDefinition => node.child_by_field_name("name"),
        SubscriptExpression => node.child_by_field_name("index"),
        ShorthandPropertyIdentifierPattern => Some(node),
        _ => None,
    }?;

    if matches!(
        prop.kind(),
        Identifier | PropertyIdentifier | ShorthandPropertyIdentifierPattern
    ) && node.kind() != SubscriptExpression
    {
        return Some(context.get_node_text(prop));
    }

    get_static_string_value(skip_nodes_of_type(prop, ComputedPropertyName), context)
}

pub enum StrOrRegex<'a> {
    Str(&'a str),
    Regex(&'a Regex),
}

impl<'a> From<&'a str> for StrOrRegex<'a> {
    fn from(value: &'a str) -> Self {
        Self::Str(value)
    }
}

impl<'a> From<&'a Regex> for StrOrRegex<'a> {
    fn from(value: &'a Regex) -> Self {
        Self::Regex(value)
    }
}

fn check_text<'a>(actual: &str, expected: impl Into<StrOrRegex<'a>>) -> bool {
    let expected = expected.into();
    match expected {
        StrOrRegex::Str(expected) => expected == actual,
        StrOrRegex::Regex(expected) => expected.is_match(actual),
    }
}

fn is_specific_id<'a>(
    node: Node,
    name: impl Into<StrOrRegex<'a>>,
    context: &QueryMatchContext,
) -> bool {
    node.kind() == Identifier && check_text(&node.text(context), name)
}

pub fn is_specific_member_access<'a>(
    node: Node,
    object_name: Option<impl Into<StrOrRegex<'a>>>,
    property_name: Option<impl Into<StrOrRegex<'a>>>,
    context: &QueryMatchContext,
) -> bool {
    let check_node = node;

    if !matches!(check_node.kind(), MemberExpression | SubscriptExpression) {
        return false;
    }

    if object_name
        .matches(|object_name| !is_specific_id(check_node.field("object"), object_name, context))
    {
        return false;
    }

    if let Some(property_name) = property_name {
        let actual_property_name = get_static_property_name(check_node, context);

        if actual_property_name.is_none_or_matches(|actual_property_name| {
            !check_text(&actual_property_name, property_name)
        }) {
            return false;
        }
    }

    true
}

pub fn is_parenthesised(node: Node) -> bool {
    node.kind() == ParenthesizedExpression
        || node
            .parent()
            .matches(|parent| parent.kind() == ParenthesizedExpression)
}

pub fn is_comma_token(node: Node, context: &QueryMatchContext) -> bool {
    context.get_node_text(node) == ","
}

pub fn is_closing_paren_token(node: Node, context: &QueryMatchContext) -> bool {
    context.get_node_text(node) == ")"
}

fn get_opening_paren_of_params(node: Node) -> Node {
    if node.kind() == ArrowFunction {
        if let Some(parameter) = node.child_by_field_name("parameter") {
            return parameter;
        }
    }

    get_first_non_comment_child(node.child_by_field_name("parameters").unwrap())
}

pub fn equal_tokens(left: Node, right: Node, context: &QueryMatchContext) -> bool {
    let mut tokens_l = context.get_tokens(left, Option::<fn(Node) -> bool>::None);
    let mut tokens_r = context.get_tokens(right, Option::<fn(Node) -> bool>::None);

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

pub fn is_coalesce_expression(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() == BinaryExpression && get_binary_expression_operator(node, context) == "??"
}

pub fn is_not_closing_paren_token(node: Node, context: &QueryMatchContext) -> bool {
    !is_closing_paren_token(node, context)
}

static BREAKABLE_TYPE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(?:do|while|for(?:_in)?|switch)_statement$"#).unwrap());

pub fn is_breakable_statement(node: Node) -> bool {
    BREAKABLE_TYPE_PATTERN.is_match(node.kind())
}

pub fn get_precedence(node: Node, context: &QueryMatchContext) -> u32 {
    _get_precedence(
        node.kind(),
        (node.kind() == BinaryExpression).then(|| get_binary_expression_operator(node, context)),
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

pub fn get_function_name_with_kind(node: Node, context: &QueryMatchContext) -> String {
    if node.kind() == MethodDefinition
        && get_method_definition_kind(node, context) == MethodDefinitionKind::Constructor
    {
        return "constructor".into();
    }

    enum FunctionType {
        Method,
        GeneratorFunction,
        Function,
        ArrowFunction,
        GeneratorMethod,
        Getter,
        Setter,
    }
    let mut function_type = match node.kind() {
        MethodDefinition => FunctionType::Method,
        GeneratorFunction | GeneratorFunctionDeclaration => FunctionType::GeneratorFunction,
        Function | FunctionDeclaration => FunctionType::Function,
        ArrowFunction => FunctionType::ArrowFunction,
        _ => unreachable!(),
    };
    let mut is_async = false;
    let mut is_static = false;
    let mut is_private = false;
    let function_name = if let Some(field_definition) = node
        .parent()
        .filter(|parent| parent.kind() == FieldDefinition)
    {
        function_type = FunctionType::Method;
        let mut children = field_definition
            .non_comment_children_and_field_names()
            .skip_while(|(child, _)| child.kind() == Decorator);
        let (child, field_name) = children.next().unwrap();
        let property_name = if field_name == Some("property") {
            child
        } else {
            is_static = true;
            children.next().unwrap().0
        };
        match property_name.kind() {
            PrivatePropertyIdentifier => {
                is_private = true;
                Some(context.get_node_text(property_name))
            }
            _ => get_static_property_name(field_definition, context)
                .map(|name| format!("'{}'", name).into()),
        }
    } else if node.kind() == MethodDefinition {
        let mut children = node
            .non_comment_children_and_field_names()
            .skip_while(|(child, _)| child.kind() == Decorator);
        let (mut child, mut field_name) = children.next().unwrap();
        while !field_name.matches(|field_name| field_name == "name") {
            match &*child.text(context) {
                "static" => is_static = true,
                "async" => is_async = true,
                "get" => function_type = FunctionType::Getter,
                "set" => function_type = FunctionType::Setter,
                "*" => function_type = FunctionType::GeneratorMethod,
                _ => unreachable!(),
            }
            (child, field_name) = children.next().unwrap();
        }
        match child.kind() {
            PrivatePropertyIdentifier => {
                is_private = true;
                Some(child.text(context))
            }
            _ => get_static_property_name(node, context).map(|name| format!("'{}'", name).into()),
        }
    } else if matches!(
        node.kind(),
        Function | FunctionDeclaration | GeneratorFunction | GeneratorFunctionDeclaration
    ) {
        if get_first_non_comment_child(node).text(context) == "async" {
            is_async = true;
        }
        node.child_by_field_name("name")
            .map(|name| format!("'{}'", name.text(context)).into())
    } else {
        assert_kind!(node, ArrowFunction);
        None
    };
    let mut tokens: Vec<Cow<'_, str>> = Default::default();
    if is_static {
        tokens.push("static".into());
    }
    if is_private {
        tokens.push("private".into());
    }
    if is_async {
        tokens.push("async".into());
    }
    tokens.push(
        match function_type {
            FunctionType::Method => "method",
            FunctionType::GeneratorFunction => "generator function",
            FunctionType::Function => "function",
            FunctionType::ArrowFunction => "arrow function",
            FunctionType::GeneratorMethod => "generator method",
            FunctionType::Getter => "getter",
            FunctionType::Setter => "setter",
        }
        .into(),
    );
    if let Some(function_name) = function_name {
        tokens.push(function_name);
    }
    tokens.join(" ")
}

pub fn get_function_head_range(node: Node) -> Range {
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum StartOrEnd {
        Start,
        End,
    }
    use StartOrEnd::*;

    let parent = node.parent().unwrap();

    if parent.kind() == FieldDefinition {
        ((parent, Start), (get_opening_paren_of_params(node), Start))
    } else if node.kind() == ArrowFunction {
        let arrow_token = get_prev_non_comment_sibling(node.child_by_field_name("body").unwrap());
        ((arrow_token, Start), (arrow_token, End))
    } else {
        ((node, Start), (get_opening_paren_of_params(node), Start))
    }
    .thrush(
        |((start_node, start_node_start_or_end), (end_node, end_node_start_or_end))| Range {
            start_byte: match start_node_start_or_end {
                Start => start_node.range().start_byte,
                End => start_node.range().end_byte,
            },
            end_byte: match end_node_start_or_end {
                Start => end_node.range().start_byte,
                End => end_node.range().end_byte,
            },
            start_point: match start_node_start_or_end {
                Start => start_node.range().start_point,
                End => start_node.range().end_point,
            },
            end_point: match end_node_start_or_end {
                Start => end_node.range().start_point,
                End => end_node.range().end_point,
            },
        },
    )
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
