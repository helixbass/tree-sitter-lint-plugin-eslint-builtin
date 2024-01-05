use std::{borrow::Cow, collections::HashSet};

use const_format::formatcp;
use once_cell::sync::Lazy;
use regex::Regex;
use squalid::{return_default_if_none, CowExt, CowStrExt, EverythingExt, OptionExt};
use tree_sitter_lint::{
    tree_sitter::{Node, Point, Range, Tree},
    tree_sitter_grep::SupportedLanguage,
    NodeExt, QueryMatchContext,
};

use crate::{
    assert_kind,
    ast_helpers::{
        get_call_expression_arguments, get_cooked_value, get_first_non_comment_child,
        get_last_expression_of_sequence_expression, get_method_definition_kind,
        get_number_literal_string_value, get_number_literal_value, get_prev_non_comment_sibling,
        is_block_comment, is_chain_expression, is_logical_expression, is_punctuation_kind, parse,
        skip_nodes_of_type, template_string_has_any_cooked_literal_characters,
        MethodDefinitionKind, NodeExtJs, Number, NumberOrBigInt,
    },
    kind::{
        self, is_literal_kind, Array, ArrowFunction, AssignmentExpression,
        AugmentedAssignmentExpression, AwaitExpression, BinaryExpression, CallExpression, Class,
        ClassStaticBlock, Comment, ComputedPropertyName, Decorator, False, FieldDefinition,
        Function, FunctionDeclaration, GeneratorFunction, GeneratorFunctionDeclaration, Identifier,
        Kind, MemberExpression, MethodDefinition, NewExpression, Null, Object, Pair, PairPattern,
        ParenthesizedExpression, PrivatePropertyIdentifier, Program, PropertyIdentifier,
        SequenceExpression, ShorthandPropertyIdentifier, ShorthandPropertyIdentifierPattern,
        SpreadElement, StatementBlock, SubscriptExpression, Super, SwitchCase, SwitchDefault,
        TemplateString, TemplateSubstitution, TernaryExpression, This, True, UnaryExpression,
        Undefined, UpdateExpression, YieldExpression,
    },
    scope::{Reference, Scope, ScopeType, Variable},
};

static ARRAY_OR_TYPED_ARRAY_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r#"Array$"#).unwrap());

pub const LINE_BREAK_PATTERN_STR: &str = r#"\r\n|[\r\n\u2028\u2029]"#;

pub static LINE_BREAK_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(LINE_BREAK_PATTERN_STR).unwrap());

pub static COMMENTS_IGNORE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*(?:eslint|jshint\s+|jslint\s+|istanbul\s+|globals?\s+|exported\s+|jscs)"#)
        .unwrap()
});

#[allow(dead_code)]
pub static LINE_BREAKS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["\r\n", "\r", "\n", "\u{2028}", "\u{2029}"].into());

pub static LINE_BREAK_SINGLE_CHARS: Lazy<HashSet<char>> =
    Lazy::new(|| ['\r', '\n', '\u{2028}', '\u{2029}'].into());

pub static STATEMENT_LIST_PARENTS: Lazy<HashSet<Kind>> = Lazy::new(|| {
    [
        Program,
        StatementBlock,
        ClassStaticBlock,
        SwitchCase,
        SwitchDefault,
    ]
    .into()
});

pub static OCTAL_OR_NON_OCTAL_DECIMAL_ESCAPE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)^(?:[^\\]|\\.)*\\(?:[1-9]|0[0-9])"#).unwrap());

fn is_modifying_reference(reference: &Reference, index: usize, references: &[Reference]) -> bool {
    let identifier = reference.identifier();

    let modifying_different_identifier = match index {
        0 => true,
        index => references[index - 1].identifier() != identifier,
    };

    // identifier &&
    reference.init() == Some(false) && reference.is_write() && modifying_different_identifier
}

fn starts_with_upper_case(str_: &str) -> bool {
    str_.chars().next().matches(|ch| ch.is_uppercase())
}

pub fn is_es5_constructor(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() != MethodDefinition
        && node
            .child_by_field_name("name")
            .matches(|name| starts_with_upper_case(&name.text(context)))
}

pub fn get_upper_function(node: Node) -> Option<Node> {
    let mut current_node = node;
    loop {
        if any_function_pattern.is_match(current_node.kind()) {
            return Some(current_node);
        }
        current_node = current_node.parent()?;
    }
}

static any_function_pattern: Lazy<Regex> = Lazy::new(|| {
    Regex::new(formatcp!(
        r#"^(?:{FunctionDeclaration}|{GeneratorFunctionDeclaration}|{Function}|{GeneratorFunction}|{ArrowFunction}|{MethodDefinition})$"#
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

pub fn is_in_loop(node: Node) -> bool {
    let mut current_node = node;
    while !is_function(current_node) {
        if is_loop(current_node) {
            return true;
        }
        current_node = return_default_if_none!(current_node.parent());
    }
    false
}

pub fn is_null_literal(node: Node) -> bool {
    node.kind() == Null
}

pub fn is_null_or_undefined(node: Node) -> bool {
    is_null_literal(node)
        || node.kind() == Undefined
        || node.kind() == UnaryExpression && node.field("operator").kind() == "void"
}

pub fn is_callee(node: Node) -> bool {
    node.maybe_next_non_parentheses_ancestor()
        .matches(|parent| {
            parent.kind() == CallExpression && parent.field("function").skip_parentheses() == node
        })
}

pub fn get_static_string_value<'a>(
    node: Node,
    context: &QueryMatchContext<'a, '_>,
) -> Option<Cow<'a, str>> {
    match node.kind() {
        kind::Number => Some(get_number_literal_string_value(node, context).into()),
        kind::Regex => Some(context.get_node_text(node)),
        kind::String => Some(
            node.text(context)
                .sliced(|len| 1..len - 1)
                .map_cow(get_cooked_value /* , false */),
        ),
        Null => Some("null".into()),
        TemplateString => {
            (!context.has_named_child_of_kind(node, "template_substitution")).then(|| {
                node.text(context)
                    .sliced(|len| 1..len - 1)
                    .map_cow(get_cooked_value /* , true */)
            })
        }
        _ => None,
    }
}

pub fn get_static_property_name<'a>(
    node: Node,
    context: &QueryMatchContext<'a, '_>,
) -> Option<Cow<'a, str>> {
    let prop = match node.kind() {
        Pair | PairPattern => node.child_by_field_name("key"),
        FieldDefinition | MemberExpression => node.child_by_field_name("property"),
        MethodDefinition | "public_field_definition" => node.child_by_field_name("name"),
        SubscriptExpression => node.child_by_field_name("index"),
        ShorthandPropertyIdentifierPattern | ShorthandPropertyIdentifier => Some(node),
        _ => None,
    }?;

    if matches!(
        prop.kind(),
        Identifier
            | PropertyIdentifier
            | ShorthandPropertyIdentifierPattern
            | ShorthandPropertyIdentifier
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

pub fn is_specific_id<'a>(
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

    if object_name.matches(|object_name| {
        !is_specific_id(
            check_node.field("object").skip_parentheses(),
            object_name,
            context,
        )
    }) {
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

fn equal_literal_value(left: Node, right: Node, context: &QueryMatchContext) -> bool {
    match (left.kind(), right.kind()) {
        // TODO: these presumably need much refinement?
        (kind::String, kind::String) => left.text(context) == right.text(context),
        (kind::Number, kind::Number) => left.text(context) == right.text(context),
        (kind::Regex, kind::Regex) => left.text(context) == right.text(context),
        (Null, Null) => true,
        (True, True) => true,
        (False, False) => true,
        _ => false,
    }
}

pub fn is_same_reference(
    left: Node,
    right: Node,
    disable_static_computed_key: Option<bool>,
    context: &QueryMatchContext,
) -> bool {
    let left = left.skip_parentheses();
    let right = right.skip_parentheses();
    let disable_static_computed_key = disable_static_computed_key.unwrap_or_default();
    if left.kind() != right.kind()
        && !([MemberExpression, SubscriptExpression].contains(&left.kind())
            && [MemberExpression, SubscriptExpression].contains(&right.kind()))
    {
        return false;
    }

    match left.kind() {
        Super | This => true,
        Identifier | PrivatePropertyIdentifier => left.text(context) == right.text(context),
        kind if is_literal_kind(kind) => equal_literal_value(left, right, context),
        MemberExpression | SubscriptExpression => {
            if !disable_static_computed_key {
                let name_a = get_static_property_name(left, context);

                if let Some(name_a) = name_a {
                    return is_same_reference(
                        left.field("object"),
                        right.field("object"),
                        Some(disable_static_computed_key),
                        context,
                    ) && Some(name_a) == get_static_property_name(right, context);
                }
            }

            left.kind() == right.kind()
                && is_same_reference(
                    left.field("object"),
                    right.field("object"),
                    Some(disable_static_computed_key),
                    context,
                )
                && match left.kind() {
                    MemberExpression => is_same_reference(
                        left.field("property"),
                        right.field("property"),
                        Some(disable_static_computed_key),
                        context,
                    ),
                    SubscriptExpression => is_same_reference(
                        left.field("index"),
                        right.field("index"),
                        Some(disable_static_computed_key),
                        context,
                    ),
                    _ => unreachable!(),
                }
        }
        _ => false,
    }
}

pub fn is_array_from_method(node: Node, context: &QueryMatchContext) -> bool {
    is_specific_member_access(
        node,
        Some(&*ARRAY_OR_TYPED_ARRAY_PATTERN),
        Some("from"),
        context,
    )
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

pub fn is_comment_token(node: Node) -> bool {
    node.kind() == Comment
}

fn get_opening_paren_of_params(node: Node) -> Node {
    if node.kind() == ArrowFunction {
        if let Some(parameter) = node.child_by_field_name("parameter") {
            return parameter;
        }
    }

    get_first_non_comment_child(node.child_by_field_name("parameters").unwrap())
}

pub fn equal_tokens<'a>(
    left: Node<'a>,
    right: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> bool {
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

pub fn is_coalesce_expression(node: Node) -> bool {
    node.kind() == BinaryExpression && node.field("operator").kind() == "??"
}

static LOGICAL_ASSIGNMENT_OPERATORS: Lazy<HashSet<&'static str>> =
    Lazy::new(|| ["&&=", "||=", "??="].into_iter().collect());

pub fn is_logical_assignment_operator(operator: &str) -> bool {
    LOGICAL_ASSIGNMENT_OPERATORS.contains(operator)
}

fn get_boolean_value(node: Node, context: &QueryMatchContext) -> bool {
    match node.kind() {
        kind::String => node.range().end_byte - node.range().start_byte > 2,
        kind::Number => match get_number_literal_value(node, context) {
            NumberOrBigInt::Number(Number::NaN) => false,
            NumberOrBigInt::Number(Number::Integer(value)) => value != 0,
            NumberOrBigInt::Number(Number::Float(value)) => value != 0.0,
            NumberOrBigInt::BigInt(value) => value != 0,
        },
        kind::Regex => true,
        Null => false,
        True => true,
        False => false,
        _ => unreachable!(),
    }
}

fn is_logical_identity(node: Node, operator: &str, context: &QueryMatchContext) -> bool {
    let node = node.skip_parentheses();
    match node.kind() {
        #[allow(clippy::bool_comparison)]
        kind if is_literal_kind(kind) => {
            operator == "||" && get_boolean_value(node, context) == true
                || operator == "&&" && get_boolean_value(node, context) == false
        }
        UnaryExpression => operator == "&&" && node.field("operator").kind() == "void",
        BinaryExpression => {
            operator == node.field("operator").kind()
                && (is_logical_identity(node.field("left"), operator, context)
                    || is_logical_identity(node.field("right"), operator, context))
        }
        AugmentedAssignmentExpression => {
            let node_operator = node.field("operator").kind();
            ["||=", "&&="].contains(&node_operator)
                && operator == &node_operator[0..2]
                && is_logical_identity(node.field("right"), operator, context)
        }
        _ => false,
    }
}

pub fn is_reference_to_global_variable(scope: &Scope, node: Node) -> bool {
    scope
        .references()
        .find(|ref_| ref_.identifier() == node)
        .and_then(|reference| reference.resolved())
        .matches(|resolved| {
            resolved.scope().type_() == ScopeType::Global && resolved.defs().next().is_none()
        })
}

pub fn is_constant(
    scope: &Scope,
    node: Node,
    in_boolean_position: bool,
    context: &QueryMatchContext,
) -> bool {
    // if (!node) {
    //     return true;
    // }
    match node.kind() {
        kind if is_literal_kind(kind) => true,
        ArrowFunction | Function | Class | Object => true,
        TemplateString => {
            in_boolean_position && template_string_has_any_cooked_literal_characters(node, context)
                || node.children_of_kind(TemplateSubstitution).all(|exp| {
                    is_constant(
                        scope,
                        exp.first_non_comment_named_child(SupportedLanguage::Javascript),
                        false,
                        context,
                    )
                })
        }
        Array => {
            if !in_boolean_position {
                return node
                    .non_comment_named_children(SupportedLanguage::Javascript)
                    .all(|element| is_constant(scope, element, false, context));
            }
            true
        }
        UnaryExpression => {
            let operator = node.field("operator").kind();
            if operator == "void" || operator == "typeof" && in_boolean_position {
                return true;
            }

            if operator == "!" {
                return is_constant(scope, node.field("argument"), true, context);
            }

            is_constant(scope, node.field("argument"), false, context)
        }
        BinaryExpression => {
            if is_logical_expression(node) {
                let left = node.field("left");
                let right = node.field("right");
                let operator = node.field("operator").kind();
                let is_left_constant = is_constant(scope, left, in_boolean_position, context);
                let is_right_constant = is_constant(scope, right, in_boolean_position, context);
                let is_left_short_circuit =
                    is_left_constant && is_logical_identity(left, operator, context);
                let is_right_short_circuit = in_boolean_position
                    && is_right_constant
                    && is_logical_identity(right, operator, context);

                is_left_constant && is_right_constant
                    || is_left_short_circuit
                    || is_right_short_circuit
            } else {
                is_constant(scope, node.field("left"), false, context)
                    && is_constant(scope, node.field("right"), false, context)
                    && node.field("operator").kind() != "in"
            }
        }
        NewExpression => in_boolean_position,
        AssignmentExpression => {
            is_constant(scope, node.field("right"), in_boolean_position, context)
        }
        AugmentedAssignmentExpression => {
            let operator = node.field("operator").kind();
            if ["||=", "&&="].contains(&operator) && in_boolean_position {
                return is_logical_identity(node.field("right"), &operator[0..2], context);
            }

            false
        }
        SequenceExpression => is_constant(
            scope,
            get_last_expression_of_sequence_expression(node),
            in_boolean_position,
            context,
        ),
        SpreadElement => is_constant(
            scope,
            node.first_non_comment_named_child(SupportedLanguage::Javascript),
            in_boolean_position,
            context,
        ),
        CallExpression => {
            let callee = node.field("function");
            #[allow(clippy::collapsible_if)]
            if callee.kind() == Identifier && callee.text(context) == "Boolean" {
                if get_call_expression_arguments(node).matches(|mut arguments| {
                    match arguments.next() {
                        None => true,
                        Some(first_argument) => is_constant(scope, first_argument, true, context),
                    }
                }) {
                    return is_reference_to_global_variable(scope, callee);
                }
            }
            false
        }
        Undefined => is_reference_to_global_variable(scope, node),
        ParenthesizedExpression => is_constant(
            scope,
            node.first_non_comment_named_child(SupportedLanguage::Javascript),
            in_boolean_position,
            context,
        ),
        _ => false,
    }
}

pub fn is_token_on_same_line(left: Node, right: Node) -> bool {
    left.range().end_point.row == right.range().start_point.row
}

pub fn is_not_closing_paren_token(node: Node, context: &QueryMatchContext) -> bool {
    !is_closing_paren_token(node, context)
}

pub static BREAKABLE_TYPE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(?:do|while|for(?:_in)?|switch)_statement$"#).unwrap());

pub fn is_string_literal(node: Node) -> bool {
    matches!(node.kind(), kind::String | TemplateString)
}

pub fn is_breakable_statement(node: Node) -> bool {
    BREAKABLE_TYPE_PATTERN.is_match(node.kind())
}

pub fn get_modifying_references<'a, 'b>(
    references: &[Reference<'a, 'b>],
) -> Vec<Reference<'a, 'b>> {
    references
        .into_iter()
        .enumerate()
        .filter(|(index, reference)| is_modifying_reference(reference, *index, references))
        .map(|(_, reference)| reference.clone())
        .collect()
}

pub fn get_variable_by_name<'a, 'b>(
    init_scope: Scope<'a, 'b>,
    name: &str,
) -> Option<Variable<'a, 'b>> {
    let mut scope = init_scope;

    loop {
        if let Some(variable) = scope.set().get(name).cloned() {
            return Some(variable);
        }

        scope = scope.maybe_upper()?;
    }
}

pub fn get_precedence(node: Node) -> u32 {
    _get_precedence(
        node.kind(),
        (node.kind() == BinaryExpression).then(|| node.field("operator").kind()),
        (node.kind() == MemberExpression).then(|| is_chain_expression(node)),
    )
}

fn _get_precedence(
    kind: Kind,
    binary_expression_operator: Option<&str>,
    member_expression_is_chain_expression: Option<bool>,
) -> u32 {
    match kind {
        SequenceExpression => 0,
        AssignmentExpression | AugmentedAssignmentExpression | ArrowFunction | YieldExpression => 1,
        TernaryExpression => 3,
        BinaryExpression => match binary_expression_operator.unwrap() {
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

pub fn get_binary_expression_operator_precedence(operator: &str) -> u32 {
    _get_precedence(BinaryExpression, Some(operator), None)
}

pub fn is_empty_block(node: Node) -> bool {
    node.kind() == StatementBlock
        && node
            .non_comment_named_children(SupportedLanguage::Javascript)
            .next()
            .is_none()
}

pub fn is_empty_function(node: Node) -> bool {
    is_function(node) && is_empty_block(node.field("body"))
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

static DECIMAL_INTEGER_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(?:0|0[0-7]*[89]\d*|[1-9](?:_?\d)*)$"#).unwrap());

pub fn is_decimal_integer_numeric_token(token: Node, context: &QueryMatchContext) -> bool {
    token.kind() == kind::Number && DECIMAL_INTEGER_PATTERN.is_match(&token.text(context))
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
    let is_object_literal_method =
        matches!(node.kind(), Function | ArrowFunction) && node.parent().unwrap().kind() == Pair;
    let mut function_type = match node.kind() {
        MethodDefinition => FunctionType::Method,
        GeneratorFunction | GeneratorFunctionDeclaration => FunctionType::GeneratorFunction,
        Function | FunctionDeclaration => {
            if is_object_literal_method {
                FunctionType::Method
            } else {
                FunctionType::Function
            }
        }
        ArrowFunction => {
            if is_object_literal_method {
                FunctionType::Method
            } else {
                FunctionType::ArrowFunction
            }
        }
        _ => unreachable!(),
    };
    let mut is_async = false;
    let mut is_static = false;
    let mut is_private = false;
    let function_name = if let Some(field_definition) = node
        .parent()
        .filter(|parent| matches!(parent.kind(), FieldDefinition | "public_field_definition"))
    {
        function_type = FunctionType::Method;
        let mut children = field_definition
            .non_comment_children_and_field_names(context)
            .skip_while(|(child, _)| child.kind() == Decorator);
        let (child, field_name) = children.next().unwrap();
        let property_name = if field_name == Some("property") || field_name == Some("name") {
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
            .non_comment_children_and_field_names(context)
            .skip_while(|(child, _)| child.kind() == Decorator);
        let (mut child, mut field_name) = children.next().unwrap();
        while !field_name.matches(|field_name| field_name == "name") {
            match child.kind() {
                "static" => is_static = true,
                "async" => is_async = true,
                "get" => function_type = FunctionType::Getter,
                "set" => function_type = FunctionType::Setter,
                "*" => function_type = FunctionType::GeneratorMethod,
                "static get" => {
                    is_static = true;
                    function_type = FunctionType::Getter
                }
                "readonly" | "accessibility_modifier" | "override_modifier" => (),
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
    } else {
        assert_kind!(
            node,
            Function
                | FunctionDeclaration
                | GeneratorFunction
                | GeneratorFunctionDeclaration
                | ArrowFunction
        );
        if get_first_non_comment_child(node).text(context) == "async" {
            is_async = true;
        }
        if is_object_literal_method {
            let pair = node.parent().unwrap();
            match pair.field("key").kind() {
                PrivatePropertyIdentifier => {
                    is_private = true;
                    Some(context.get_node_text(pair.field("key")))
                }
                _ => {
                    get_static_property_name(pair, context).map(|name| format!("'{}'", name).into())
                }
            }
        } else {
            node.child_by_field_name("name")
                .map(|name| format!("'{}'", name.text(context)).into())
        }
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

    if matches!(
        parent.kind(),
        FieldDefinition | "public_field_definition" | Pair
    ) {
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

pub fn could_be_error(node: Node, context: &QueryMatchContext) -> bool {
    match node.kind() {
        Identifier | CallExpression | NewExpression | MemberExpression | SubscriptExpression
        | YieldExpression | AwaitExpression | Undefined => true,
        AssignmentExpression => could_be_error(node.field("right"), context),
        AugmentedAssignmentExpression => match &*node.field("operator").text(context) {
            "&&=" => could_be_error(node.field("right"), context),
            "||=" | "??=" => {
                could_be_error(node.field("left"), context)
                    || could_be_error(node.field("right"), context)
            }
            _ => false,
        },
        SequenceExpression => {
            could_be_error(get_last_expression_of_sequence_expression(node), context)
        }
        BinaryExpression => {
            if !is_logical_expression(node) {
                return false;
            }

            if node.field("operator").text(context) == "&&" {
                return could_be_error(node.field("right"), context);
            }

            could_be_error(node.field("left"), context)
                || could_be_error(node.field("right"), context)
        }
        TernaryExpression => {
            could_be_error(node.field("consequence"), context)
                || could_be_error(node.field("alternative"), context)
        }
        _ => false,
    }
}

pub fn can_tokens_be_adjacent<'a>(
    left_value: impl Into<NodeOrStr<'a>>,
    right_value: impl Into<NodeOrStr<'a>>,
    context: &QueryMatchContext,
) -> bool {
    let left_value = left_value.into();

    let left_value_tree: Option<Tree>;
    let left_value = match left_value {
        NodeOrStr::Node(left_value) => left_value,
        NodeOrStr::Str(left_value) => {
            left_value_tree = Some(parse(left_value));
            left_value_tree
                .as_ref()
                .unwrap()
                .root_node()
                .tokens()
                .last()
                .unwrap()
        }
    };
    let right_value = right_value.into();

    let right_value_tree: Option<Tree>;
    let right_value = match right_value {
        NodeOrStr::Node(right_value) => right_value,
        NodeOrStr::Str(right_value) => {
            right_value_tree = Some(parse(right_value));
            right_value_tree
                .as_ref()
                .unwrap()
                .root_node()
                .tokens()
                .next()
                .unwrap()
        }
    };

    match (left_value.kind(), right_value.kind()) {
        (left_kind, right_kind)
            if is_punctuation_kind(left_kind) && is_punctuation_kind(right_kind) =>
        {
            static PLUS_TOKENS: [&str; 2] = ["+", "++"];
            static MINUS_TOKENS: [&str; 2] = ["-", "--"];

            !(PLUS_TOKENS.contains(&left_kind) && PLUS_TOKENS.contains(&right_kind)
                || MINUS_TOKENS.contains(&left_kind) && MINUS_TOKENS.contains(&right_kind))
        }
        ("/", right_kind) => ![Comment, kind::Regex].contains(&right_kind),
        (left_kind, right_kind)
            if is_punctuation_kind(left_kind) || is_punctuation_kind(right_kind) =>
        {
            true
        }
        (kind::String | TemplateString, _) | (_, kind::String | TemplateString) => true,
        (left_kind, kind::Number)
            if left_kind != kind::Number && right_value.text(context).starts_with('.') =>
        {
            true
        }
        (Comment, _) if is_block_comment(left_value, context) => true,
        (_, Comment | PrivatePropertyIdentifier) => true,
        _ => false,
    }
}

pub fn get_name_location_in_global_directive_comment(
    context: &QueryMatchContext,
    comment: Node,
    name: &str,
) -> Range {
    let name_pattern =
        Regex::new(&format!(r#"[\s,]{}(?:$|[\s,:*])"#, regex::escape(name))).unwrap();

    let comment_text = comment.text(context);
    let start_index = comment_text.find("global").unwrap() + 6;

    let match_ = name_pattern.find(&comment_text[start_index..]);

    let start_offset = match_.map_or_default(|match_| match_.start() + start_index + 1);
    let start_byte = comment.start_byte() + start_offset;
    // TODO: this doesn't handle multi-line comments,
    // I was avoiding the fact that currently don't appear
    // to have the equivalent of .lines/.lineStartIndices
    // in order to eg implement the equivalent of getLocFromIndex()
    // on FileRunContext
    let start_point = Point {
        row: comment.start_position().row,
        column: comment.start_position().column + start_offset,
    };
    let end_offset = start_offset
        + match match_ {
            Some(_) => name.len(),
            None => 1,
        };
    let end_byte = comment.start_byte() + end_offset;
    let end_point = Point {
        row: comment.start_position().row,
        column: comment.start_position().column + end_offset,
    };

    Range {
        start_byte,
        start_point,
        end_byte,
        end_point,
    }
}

pub enum NodeOrStr<'a> {
    Node(Node<'a>),
    Str(&'a str),
}

impl<'a> From<Node<'a>> for NodeOrStr<'a> {
    fn from(value: Node<'a>) -> Self {
        Self::Node(value)
    }
}

impl<'a> From<&'a str> for NodeOrStr<'a> {
    fn from(value: &'a str) -> Self {
        Self::Str(value)
    }
}

pub fn is_static_template_literal(node: Node) -> bool {
    node.kind() == TemplateString && !node.has_child_of_kind(TemplateSubstitution)
}

pub fn has_octal_or_non_octal_decimal_escape_sequence(raw_string: &str) -> bool {
    OCTAL_OR_NON_OCTAL_DECIMAL_ESCAPE_PATTERN.is_match(raw_string)
}
