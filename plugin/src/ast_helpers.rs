use std::{borrow::Cow, iter};

use itertools::Either;
use squalid::{BoolExt, CowStrExt, OptionExt};
use tree_sitter_lint::{
    regex, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, NodeExt, NonCommentChildren,
    QueryMatchContext, SourceTextProvider,
};

use crate::{
    kind::{
        self, Arguments, BinaryExpression, CallExpression, Comment, ComputedPropertyName,
        ExpressionStatement, FieldDefinition, ForInStatement, ImportClause, Kind, MemberExpression,
        MethodDefinition, NewExpression, Object, Pair, ParenthesizedExpression, PropertyIdentifier,
        SequenceExpression, ShorthandPropertyIdentifier, SubscriptExpression, TemplateString,
        UpdateExpression, Identifier, ArrowFunction, EscapeSequence,
    },
    return_default_if_none,
};

#[macro_export]
macro_rules! assert_kind {
    ($node:expr, $kind:pat) => {
        assert!(
            matches!($node.kind(), $kind),
            "Expected kind {:?}, got: {:?}",
            stringify!($kind),
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
    node.kind() == ForInStatement
        && matches!(
            node.child_by_field_name("operator"),
            Some(child) if source_text_provider.node_text(child) == "of"
        )
}

pub fn is_for_of_await(node: Node, context: &QueryMatchContext) -> bool {
    assert_kind!(node, ForInStatement);
    is_for_of(node, context)
        && matches!(
            // TODO: I can't do stuff like this because comments could be anywhere
            node.child(1),
            Some(child) if context.get_node_text(child) == "await"
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

pub fn maybe_get_prev_non_comment_sibling(mut node: Node) -> Option<Node> {
    node = node.prev_sibling()?;
    while node.kind() == Comment {
        node = node.prev_sibling()?;
    }
    Some(node)
}

pub fn get_prev_non_comment_sibling(node: Node) -> Node {
    maybe_get_prev_non_comment_sibling(node).unwrap()
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
    let is_object_method = node.parent().unwrap().kind() == Object;
    if name.kind() == PropertyIdentifier
        && !is_object_method
        && context.get_node_text(name) == "constructor"
        && !is_class_member_static(node, context)
    {
        return MethodDefinitionKind::Constructor;
    }
    if name.kind() == kind::String
        && !is_object_method
        && string_node_equals(name, "constructor", context)
        && !is_class_member_static(node, context)
    {
        return MethodDefinitionKind::Constructor;
    }
    match maybe_get_prev_non_comment_sibling(name)
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
    assert_kind!(node, MethodDefinition | FieldDefinition);

    let mut cursor = node.walk();
    return_default_if_false!(cursor.goto_first_child());
    while cursor.field_name() == Some("decorator") {
        return_default_if_false!(cursor.goto_next_sibling());
    }
    context.get_node_text(cursor.node()) == "static"
}

#[derive(Copy, Clone, Debug)]
pub enum Number {
    NaN,
    Integer(u64),
    Float(f64),
}

impl Number {
    pub fn is_truthy(&self) -> bool {
        match self {
            Number::NaN => false,
            Number::Integer(value) => *value != 0,
            Number::Float(value) => *value != 0.0,
        }
    }
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

pub fn get_number_literal_value(node: Node, context: &QueryMatchContext) -> Number {
    assert_kind!(node, kind::Number);

    Number::from(&*context.get_node_text(node))
}

pub fn get_number_literal_string_value(node: Node, context: &QueryMatchContext) -> String {
    match get_number_literal_value(node, context) {
        Number::NaN => unreachable!("I don't know if this should be possible?"),
        Number::Integer(number) => number.to_string(),
        Number::Float(number) => number.to_string(),
    }
}

pub fn is_logical_and(node: Node) -> bool {
    node.kind() == BinaryExpression && node.field("operator").kind() == "&&"
}

pub fn get_first_child_of_kind(node: Node, kind: Kind) -> Node {
    let mut cursor = node.walk();
    let ret = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == kind)
        .unwrap();
    ret
}

pub fn maybe_get_first_non_comment_child(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    let ret = node
        .children(&mut cursor)
        .find(|child| child.kind() != Comment);
    ret
}

pub fn get_first_non_comment_child(node: Node) -> Node {
    maybe_get_first_non_comment_child(node).unwrap()
}

pub trait NodeExtJs<'a> {
    fn maybe_next_non_parentheses_ancestor(&self) -> Option<Node<'a>>;
    fn next_non_parentheses_ancestor(&self) -> Node<'a>;
    fn skip_parentheses(&self) -> Node<'a>;
    fn is_first_call_expression_argument(&self, call_expression: Node) -> bool;
}

impl<'a> NodeExtJs<'a> for Node<'a> {
    fn maybe_next_non_parentheses_ancestor(&self) -> Option<Node<'a>> {
        let mut node = self.parent()?;
        while node.kind() == ParenthesizedExpression {
            node = node.parent()?;
        }
        Some(node)
    }

    fn next_non_parentheses_ancestor(&self) -> Node<'a> {
        self.maybe_next_non_parentheses_ancestor().unwrap()
    }

    fn skip_parentheses(&self) -> Node<'a> {
        skip_parenthesized_expressions(*self)
    }

    fn is_first_call_expression_argument(&self, call_expression: Node) -> bool {
        assert_kind!(call_expression, CallExpression);

        call_expression
            .field("arguments")
            .when_kind(Arguments)
            .matches(|arguments| {
                arguments
                    .non_comment_named_children(SupportedLanguage::Javascript)
                    .next()
                    .matches(|first| first == *self)
            })
    }
}

pub fn get_num_call_expression_arguments(node: Node) -> Option<usize> {
    get_call_expression_arguments(node).map(|arguments| arguments.count())
}

pub fn get_call_expression_arguments(node: Node) -> Option<impl Iterator<Item = Node>> {
    assert_kind!(node, CallExpression | NewExpression);

    let arguments = match node.child_by_field_name("arguments") {
        Some(arguments) => arguments,
        None => return Some(Either::Left(iter::empty())),
    };
    match arguments.kind() {
        TemplateString => None,
        Arguments => Some(Either::Right(
            arguments.non_comment_named_children(SupportedLanguage::Javascript),
        )),
        _ => unreachable!(),
    }
}

pub fn call_expression_has_single_matching_argument(
    node: Node,
    predicate: impl FnOnce(Node) -> bool,
) -> bool {
    let mut arguments = return_default_if_none!(get_call_expression_arguments(node));
    let first_arg = return_default_if_none!(arguments.next());
    if !predicate(first_arg) {
        return false;
    }
    if arguments.next().is_some() {
        return false;
    }
    true
}

pub fn get_last_expression_of_sequence_expression(mut node: Node) -> Node {
    assert_kind!(node, SequenceExpression);

    while node.kind() == SequenceExpression {
        node = node.field("right");
    }
    node
}

pub fn is_logical_expression(node: Node) -> bool {
    if node.kind() != BinaryExpression {
        return false;
    }

    matches!(node.field("operator").kind(), "&&" | "||" | "??")
}

pub fn get_object_property_computed_property_name(node: Node) -> Option<Node> {
    match node.kind() {
        Pair => Some(node.field("key")),
        MethodDefinition => Some(node.field("name")),
        _ => None,
    }
    .filter(|name| name.kind() == ComputedPropertyName)
}

pub fn get_object_property_key(node: Node) -> Node {
    match node.kind() {
        Pair => node.field("key"),
        MethodDefinition => node.field("name"),
        ShorthandPropertyIdentifier => node,
        _ => unreachable!(),
    }
}

pub fn get_comment_contents<'a>(
    comment: Node,
    context: &QueryMatchContext<'a, '_>,
) -> Cow<'a, str> {
    assert_kind!(comment, Comment);
    let text = comment.text(context);
    if text.starts_with("//") {
        text.sliced(2..)
    } else {
        assert!(text.starts_with("/*"));
        text.sliced(2..text.len() - 2)
    }
}

pub fn is_chain_expression(node: Node) -> bool {
    match node.kind() {
        CallExpression => {
            node.child_by_field_name("optional_chain").is_some()
                || is_chain_expression(node.field("function"))
        }
        MemberExpression | SubscriptExpression => {
            node.child_by_field_name("optional_chain").is_some()
                || is_chain_expression(node.field("object"))
        }
        _ => false,
    }
}

pub fn is_outermost_chain_expression(node: Node) -> bool {
    is_chain_expression(node) && !is_chain_expression(node.parent().unwrap())
}

pub fn is_generator_method_definition(node: Node, context: &QueryMatchContext) -> bool {
    assert_kind!(node, MethodDefinition);
    let mut cursor = node.walk();
    assert!(cursor.goto_first_child());
    while cursor.field_name() != Some("name") {
        if cursor.node().text(context) == "*" {
            return true;
        }
        assert!(cursor.goto_next_sibling());
    }
    false
}

pub fn get_comma_separated_optional_non_comment_named_children(
    node: Node,
) -> impl Iterator<Item = Option<Node>> {
    CommaSeparated::new(node.non_comment_children(SupportedLanguage::Javascript))
}

struct CommaSeparated<'a> {
    non_comment_children: NonCommentChildren<'a>,
    just_saw_item: bool,
}

impl<'a> CommaSeparated<'a> {
    fn new(non_comment_children: NonCommentChildren<'a>) -> Self {
        Self {
            non_comment_children,
            just_saw_item: Default::default(),
        }
    }
}

impl<'a> Iterator for CommaSeparated<'a> {
    type Item = Option<Node<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.non_comment_children.next()?;
            match next.kind() {
                "," => match self.just_saw_item {
                    true => {
                        self.just_saw_item = false;
                    }
                    false => {
                        return Some(None);
                    }
                },
                _ => {
                    if !next.is_named() {
                        continue;
                    }
                    self.just_saw_item = true;
                    return Some(Some(next));
                }
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CommentType {
    Block,
    Line,
}

pub fn get_comment_type(comment: Node, context: &QueryMatchContext) -> CommentType {
    assert_kind!(comment, Comment);
    let text = comment.text(context);
    if text.starts_with("//") {
        CommentType::Line
    } else {
        CommentType::Block
    }
}

pub fn is_punctuation_kind(kind: Kind) -> bool {
    !regex!(r#"^[a-zA-Z]"#).is_match(kind)
}

#[allow(dead_code)]
pub fn is_punctuation(node: Node) -> bool {
    is_punctuation_kind(node.kind())
}

#[allow(dead_code)]
pub fn is_line_comment(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() == Comment && get_comment_type(node, context) == CommentType::Line
}

pub fn is_block_comment(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() == Comment && get_comment_type(node, context) == CommentType::Block
}

pub fn is_postfix_update_expression(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() == UpdateExpression
        && node.first_non_comment_child(context) == node.field("argument")
}

pub fn maybe_get_directive<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Option<Cow<'a, str>> {
    (node.kind() == ExpressionStatement).then_and(|| {
        node.first_non_comment_named_child(SupportedLanguage::Javascript)
            .when_kind(kind::String)
            .map(|child| child.text(source_text_provider))
    })
}

#[allow(dead_code)]
pub fn is_default_import(node: Node) -> bool {
    node.kind() == Identifier && node.parent().unwrap().kind() == ImportClause
}

pub fn get_function_params(node: Node) -> impl Iterator<Item = Node> {
    if node.kind() == ArrowFunction {
        if let Some(parameter) = node.child_by_field_name("parameter") {
            return Either::Left(iter::once(parameter));
        }
    }
    Either::Right(node.field("parameters").non_comment_named_children(SupportedLanguage::Javascript))
}

pub fn template_string_has_any_literal_characters(node: Node) -> bool {
    assert_kind!(node, TemplateString);

    let mut last_end: Option<usize> = Default::default();
    node.non_comment_children(SupportedLanguage::Javascript).any(|child| {
        if child.kind() == EscapeSequence {
            return true;
        }
        let ret = last_end.map_or_default(|last_end| {
            last_end < child.range().start_byte
        });
        last_end = Some(child.range().end_byte);
        ret
    })
}
