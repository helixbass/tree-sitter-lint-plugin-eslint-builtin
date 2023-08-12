use std::{borrow::Cow, iter};

use itertools::Either;
use squalid::{CowStrExt, OptionExt};
use tree_sitter_lint::{
    regex,
    tree_sitter::{Node, Range, TreeCursor},
    NodeExt, QueryMatchContext, SkipOptions, SkipOptionsBuilder, SourceTextProvider,
};

use crate::{
    kind::{
        self, Arguments, BinaryExpression, CallExpression, Comment, ComputedPropertyName,
        FieldDefinition, ForInStatement, Kind, MemberExpression, MethodDefinition, NewExpression,
        Object, Pair, ParenthesizedExpression, PropertyIdentifier, SequenceExpression,
        ShorthandPropertyIdentifier, SubscriptExpression, TemplateString, UnaryExpression,
    },
    return_default_if_none,
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
    assert_one_of_kinds!(node, [MethodDefinition, FieldDefinition]);

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

pub fn get_number_literal_string_value(node: Node, context: &QueryMatchContext) -> String {
    assert_kind!(node, "number");

    match Number::from(&*context.get_node_text(node)) {
        Number::NaN => unreachable!("I don't know if this should be possible?"),
        Number::Integer(number) => number.to_string(),
        Number::Float(number) => number.to_string(),
    }
}

pub fn is_logical_and(node: Node, context: &QueryMatchContext) -> bool {
    is_binary_expression_with_operator(node, "&&", context)
}

pub fn is_binary_expression_with_operator(
    node: Node,
    operator: &str,
    context: &QueryMatchContext,
) -> bool {
    node.kind() == BinaryExpression && get_binary_expression_operator(node, context) == operator
}

pub fn is_binary_expression_with_one_of_operators(
    node: Node,
    operators: &[impl AsRef<str>],
    context: &QueryMatchContext,
) -> bool {
    if node.kind() != BinaryExpression {
        return false;
    }
    let operator_text = get_binary_expression_operator(node, context);
    operators
        .iter()
        .any(|operator| operator_text == operator.as_ref())
}

pub fn get_binary_expression_operator<'a>(
    node: Node,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Cow<'a, str> {
    assert_kind!(node, BinaryExpression);
    node.child_by_field_name("operator")
        .unwrap()
        .text(source_text_provider)
}

pub fn get_unary_expression_operator<'a>(
    node: Node,
    context: &QueryMatchContext<'a, '_>,
) -> Cow<'a, str> {
    assert_kind!(node, UnaryExpression);
    context.get_node_text(node.child_by_field_name("operator").unwrap())
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
    fn non_comment_children(&self) -> NonCommentChildren<'a>;
    fn non_comment_children_and_field_names(&self) -> NonCommentChildrenAndFieldNames<'a>;
    fn text<'b>(&self, source_text_provider: &impl SourceTextProvider<'b>) -> Cow<'b, str>;
    fn non_comment_named_children(&self) -> NonCommentNamedChildren<'a>;
    fn next_non_parentheses_ancestor(&self) -> Node<'a>;
    fn skip_parentheses(&self) -> Node<'a>;
    fn is_only_non_comment_named_sibling(&self) -> bool;
    fn has_trailing_comments(&self, context: &QueryMatchContext<'a, '_>) -> bool;
    fn maybe_first_non_comment_named_child(&self) -> Option<Node<'a>>;
    fn first_non_comment_named_child(&self) -> Node<'a>;
    fn skip_nodes_of_types(&self, kinds: &[Kind]) -> Node<'a>;
    fn skip_nodes_of_type(&self, kind: Kind) -> Node<'a>;
    fn next_ancestor_not_of_types(&self, kinds: &[Kind]) -> Node<'a>;
    fn next_ancestor_not_of_type(&self, kind: Kind) -> Node<'a>;
    fn has_child_of_kind(&self, kind: Kind) -> bool;
    fn maybe_first_child_of_kind(&self, kind: Kind) -> Option<Node<'a>>;
    fn has_non_comment_named_children(&self) -> bool;
    fn is_first_call_expression_argument(&self, call_expression: Node) -> bool;
    fn when_kind(&self, kind: Kind) -> Option<Node<'a>>;
    fn is_first_non_comment_named_child(&self) -> bool;
}

impl<'a> NodeExtJs<'a> for Node<'a> {
    fn non_comment_children(&self) -> NonCommentChildren<'a> {
        NonCommentChildren::new(*self)
    }

    fn non_comment_children_and_field_names(&self) -> NonCommentChildrenAndFieldNames<'a> {
        NonCommentChildrenAndFieldNames::new(*self)
    }

    fn text<'b>(&self, source_text_provider: &impl SourceTextProvider<'b>) -> Cow<'b, str> {
        source_text_provider.node_text(*self)
    }

    fn non_comment_named_children(&self) -> NonCommentNamedChildren<'a> {
        NonCommentNamedChildren::new(*self)
    }

    fn next_non_parentheses_ancestor(&self) -> Node<'a> {
        let mut node = self.parent().unwrap();
        while node.kind() == ParenthesizedExpression {
            node = node.parent().unwrap();
        }
        node
    }

    fn skip_parentheses(&self) -> Node<'a> {
        skip_parenthesized_expressions(*self)
    }

    fn is_only_non_comment_named_sibling(&self) -> bool {
        assert!(self.is_named());
        let parent = return_default_if_none!(self.parent());
        parent.non_comment_named_children().count() == 1
    }

    fn has_trailing_comments(&self, context: &QueryMatchContext<'a, '_>) -> bool {
        context
            .get_last_token(
                *self,
                Option::<SkipOptions<fn(Node) -> bool>>::Some(
                    SkipOptionsBuilder::default()
                        .include_comments(true)
                        .build()
                        .unwrap(),
                ),
            )
            .kind()
            == Comment
    }

    fn maybe_first_non_comment_named_child(&self) -> Option<Node<'a>> {
        self.non_comment_named_children().next()
    }

    fn first_non_comment_named_child(&self) -> Node<'a> {
        self.non_comment_named_children().next().unwrap()
    }

    fn skip_nodes_of_types(&self, kinds: &[Kind]) -> Node<'a> {
        skip_nodes_of_types(*self, kinds)
    }

    fn skip_nodes_of_type(&self, kind: Kind) -> Node<'a> {
        skip_nodes_of_type(*self, kind)
    }

    fn next_ancestor_not_of_types(&self, kinds: &[Kind]) -> Node<'a> {
        let mut node = self.parent().unwrap();
        while kinds.contains(&node.kind()) {
            node = node.parent().unwrap();
        }
        node
    }

    fn next_ancestor_not_of_type(&self, kind: Kind) -> Node<'a> {
        let mut node = self.parent().unwrap();
        while node.kind() == kind {
            node = node.parent().unwrap();
        }
        node
    }

    fn has_child_of_kind(&self, kind: Kind) -> bool {
        self.maybe_first_child_of_kind(kind).is_some()
    }

    fn maybe_first_child_of_kind(&self, kind: Kind) -> Option<Node<'a>> {
        let mut cursor = self.walk();
        let ret = self
            .children(&mut cursor)
            .find(|child| child.kind() == kind);
        ret
    }

    fn has_non_comment_named_children(&self) -> bool {
        self.non_comment_named_children().count() > 0
    }

    fn is_first_call_expression_argument(&self, call_expression: Node) -> bool {
        assert_kind!(call_expression, CallExpression);

        call_expression
            .field("arguments")
            .when_kind(Arguments)
            .matches(|arguments| {
                arguments
                    .non_comment_named_children()
                    .next()
                    .matches(|first| first == *self)
            })
    }

    fn when_kind(&self, kind: Kind) -> Option<Node<'a>> {
        (self.kind() == kind).then_some(*self)
    }

    fn is_first_non_comment_named_child(&self) -> bool {
        self.parent()
            .matches(|parent| parent.maybe_first_non_comment_named_child() == Some(*self))
    }
}

pub struct NonCommentChildren<'a> {
    cursor: TreeCursor<'a>,
    is_done: bool,
}

impl<'a> NonCommentChildren<'a> {
    pub fn new(node: Node<'a>) -> Self {
        let mut cursor = node.walk();
        let is_done = !cursor.goto_first_child();
        Self { cursor, is_done }
    }
}

impl<'a> Iterator for NonCommentChildren<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.is_done {
            let node = self.cursor.node();
            self.is_done = !self.cursor.goto_next_sibling();
            if node.kind() != Comment {
                return Some(node);
            }
        }
        None
    }
}

pub struct NonCommentChildrenAndFieldNames<'a> {
    cursor: TreeCursor<'a>,
    is_done: bool,
}

impl<'a> NonCommentChildrenAndFieldNames<'a> {
    pub fn new(node: Node<'a>) -> Self {
        let mut cursor = node.walk();
        let is_done = !cursor.goto_first_child();
        Self { cursor, is_done }
    }
}

impl<'a> Iterator for NonCommentChildrenAndFieldNames<'a> {
    type Item = (Node<'a>, Option<&'static str>);

    fn next(&mut self) -> Option<Self::Item> {
        while !self.is_done {
            let node = self.cursor.node();
            let field_name = self.cursor.field_name();
            self.is_done = !self.cursor.goto_next_sibling();
            if node.kind() != Comment {
                return Some((node, field_name));
            }
        }
        None
    }
}

pub struct NonCommentNamedChildren<'a> {
    cursor: TreeCursor<'a>,
    is_done: bool,
}

impl<'a> NonCommentNamedChildren<'a> {
    pub fn new(node: Node<'a>) -> Self {
        let mut cursor = node.walk();
        let is_done = !cursor.goto_first_child();
        Self { cursor, is_done }
    }
}

impl<'a> Iterator for NonCommentNamedChildren<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.is_done {
            let node = self.cursor.node();
            self.is_done = !self.cursor.goto_next_sibling();
            if node.is_named() && node.kind() != Comment {
                return Some(node);
            }
        }
        None
    }
}

pub fn get_num_call_expression_arguments(node: Node) -> Option<usize> {
    get_call_expression_arguments(node).map(|arguments| arguments.count())
}

pub fn get_call_expression_arguments(node: Node) -> Option<impl Iterator<Item = Node>> {
    assert_one_of_kinds!(node, [CallExpression, NewExpression]);

    let arguments = match node.child_by_field_name("arguments") {
        Some(arguments) => arguments,
        None => return Some(Either::Left(iter::empty())),
    };
    match arguments.kind() {
        TemplateString => None,
        Arguments => Some(Either::Right(arguments.non_comment_named_children())),
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

pub fn is_logical_expression(node: Node, context: &QueryMatchContext) -> bool {
    if node.kind() != BinaryExpression {
        return false;
    }

    matches!(&*node.field("operator").text(context), "&&" | "||" | "??")
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

pub fn range_between_starts(a: Range, b: Range) -> Range {
    Range {
        start_byte: a.start_byte,
        end_byte: b.start_byte,
        start_point: a.start_point,
        end_point: b.start_point,
    }
}

pub fn range_between_start_and_end(a: Range, b: Range) -> Range {
    Range {
        start_byte: a.start_byte,
        end_byte: b.end_byte,
        start_point: a.start_point,
        end_point: b.end_point,
    }
}

pub fn range_between_ends(a: Range, b: Range) -> Range {
    Range {
        start_byte: a.end_byte,
        end_byte: b.end_byte,
        start_point: a.end_point,
        end_point: b.end_point,
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
