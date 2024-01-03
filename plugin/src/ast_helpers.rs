use std::{
    borrow::Cow,
    iter::{self, Peekable},
    str::CharIndices,
};

use itertools::Either;
use regexpp_js::CodePoint;
use squalid::{BoolExt, CowStrExt, OptionExt};
use tree_sitter_lint::{
    regex,
    tree_sitter::{Node, Parser},
    tree_sitter_grep::SupportedLanguage,
    NodeExt, NonCommentChildren, QueryMatchContext, SourceTextProvider,
};

use crate::{
    kind::{
        self, Arguments, ArrowFunction, BinaryExpression, CallExpression, Comment,
        ComputedPropertyName, EscapeSequence, ExpressionStatement, FieldDefinition, ForInStatement,
        Identifier, ImportClause, Kind, MemberExpression, MethodDefinition, NewExpression, Object,
        Pair, ParenthesizedExpression, PropertyIdentifier, SequenceExpression,
        ShorthandPropertyIdentifier, SubscriptExpression, TemplateString, UpdateExpression,
    },
    return_default_if_none,
};

mod number;

pub use number::{get_number_literal_string_value, get_number_literal_value, Number};
use squalid::EverythingExt;
use tree_sitter_lint::tree_sitter::{Tree, TreeCursor};

use crate::kind::{
    ExportStatement, ImportStatement, JsxOpeningElement, JsxSelfClosingElement, NamedImports,
    NamespaceImport, TemplateSubstitution,
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
        Some("static get") => MethodDefinitionKind::Get,
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
    assert_kind!(
        node,
        MethodDefinition | FieldDefinition | "public_field_definition" // I guess Typescript uses this instead of FieldDefinition?
    );

    let mut cursor = node.walk();
    return_default_if_false!(cursor.goto_first_child());
    while cursor.field_name() == Some("decorator") {
        return_default_if_false!(cursor.goto_next_sibling());
    }
    matches!(
        &*context.get_node_text(cursor.node()),
        "static" | "static get"
    )
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
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Cow<'a, str> {
    assert_kind!(comment, Comment);
    let text = comment.text(source_text_provider);
    if text.starts_with("//") {
        text.sliced(|_| 2..)
    } else {
        assert!(text.starts_with("/*"));
        text.sliced(|len| 2..len - 2)
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

pub fn is_default_import_declaration(node: Node) -> bool {
    node.kind() == ImportStatement
        && node
            .first_non_comment_named_child(SupportedLanguage::Javascript)
            .thrush(|child| {
                child.kind() == ImportClause
                    && child
                        .first_non_comment_named_child(SupportedLanguage::Javascript)
                        .kind()
                        == Identifier
            })
}

pub fn get_function_params(node: Node) -> impl Iterator<Item = Node> {
    if node.kind() == ArrowFunction {
        if let Some(parameter) = node.child_by_field_name("parameter") {
            return Either::Left(iter::once(parameter));
        }
    }
    Either::Right(
        node.field("parameters")
            .non_comment_named_children(SupportedLanguage::Javascript),
    )
}

pub fn template_string_has_any_cooked_literal_characters(
    node: Node,
    context: &QueryMatchContext,
) -> bool {
    assert_kind!(node, TemplateString);

    let mut last_end: Option<usize> = Default::default();
    node.non_comment_children(SupportedLanguage::Javascript)
        .any(|child| {
            if child.kind() == EscapeSequence
                && !get_cooked_value(&child.text(context) /* , true */).is_empty()
            {
                return true;
            }
            let quasi = last_end.map(|last_end| context.slice(last_end..child.range().start_byte));
            last_end = Some(child.range().end_byte);
            let Some(quasi) = quasi else {
                return false;
            };
            if quasi.is_empty() {
                return false;
            }
            !get_cooked_value(&quasi /* , true */).is_empty()
        })
}

// from acorn pp.readString()
pub fn get_cooked_value(input: &str) -> Cow<'_, str> {
    // TODO: should handle lone surrogates? Maybe rather than
    // always returning Wtf16 return an enum of Cow<'_, str> or
    // Wtf16 and only return Wtf16 if it contains lone surrogates
    // (ie can't be encoded as UTF-8 in a String)?
    // Or maybe always/sometimes return Wtf8?
    let mut out: Option<String> = Default::default();
    let mut chunk_start = 0;
    let mut char_indices = input.char_indices().peekable();
    let mut should_reset_chunk_start = false;
    loop {
        let Some((index, ch)) = char_indices.next() else {
            break;
        };
        if should_reset_chunk_start {
            chunk_start = index;
        }
        should_reset_chunk_start = false;
        if matches!(ch, '\\') {
            let out = out.get_or_insert_with(Default::default);
            out.push_str(&input[chunk_start..index]);
            out.push_str(&read_escaped_char(&mut char_indices));
            should_reset_chunk_start = true;
        }
    }
    if should_reset_chunk_start {
        chunk_start = input.len();
    }
    if chunk_start < input.len() {
        if let Some(out) = out.as_mut() {
            out.push_str(&input[chunk_start..]);
        }
    }
    match out {
        Some(out) => Cow::Owned(out),
        None => Cow::Borrowed(input),
    }
}

fn read_escaped_char(
    char_indices: &mut Peekable<CharIndices>, /* in_template: bool */
) -> Cow<'static, str> {
    let (_, ch) = char_indices.next().unwrap();
    match ch {
        'n' => "\n".into(),
        'r' => "\r".into(),
        'x' => String::from(
            char::try_from(read_hex_char(&[
                char_indices.next().unwrap().1,
                char_indices.next().unwrap().1,
            ]))
            .unwrap(),
        )
        .into(),
        'u' => String::from(char::try_from(read_code_point(char_indices)).unwrap()).into(),
        't' => "\t".into(),
        'b' => "\u{0008}".into(),
        'v' => "\u{000b}".into(),
        'f' => "\u{000c}".into(),
        '\r' => {
            if matches!(char_indices.peek(), Some((_, '\n'))) {
                char_indices.next();
            }
            "".into()
        }
        // TODO: this also appears to be in the realm of
        // "invalid input" which I don't kknow if we're trying
        // to handle?
        // '8' | '9' => {}
        ch if ('0'..='7').contains(&ch) => {
            unimplemented!()
        }
        ch if is_new_line(ch) => "".into(),
        ch => String::from(ch).into(),
    }
}

fn read_code_point(char_indices: &mut Peekable<CharIndices>) -> CodePoint {
    match char_indices.peek().unwrap().1 {
        '{' => {
            char_indices.next().unwrap();
            let mut hex_chars: Vec<char> = Default::default();
            while char_indices.peek().unwrap().1 != '}' {
                hex_chars.push(char_indices.next().unwrap().1);
            }
            char_indices.next().unwrap();
            read_hex_char(&hex_chars)
        }
        _ => read_hex_char(&[
            char_indices.next().unwrap().1,
            char_indices.next().unwrap().1,
            char_indices.next().unwrap().1,
            char_indices.next().unwrap().1,
        ]),
    }
}

fn read_hex_char(chars: &[char]) -> CodePoint {
    CodePoint::from_str_radix(&chars.iter().collect::<String>(), 16).unwrap()
}

fn is_new_line(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

pub fn parse(source_text: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(SupportedLanguage::Javascript.language(None))
        .unwrap();
    parser.parse(source_text, None).unwrap()
}

pub fn is_jsx_tag_name(node: Node) -> bool {
    node.parent().matches(|parent| {
        matches!(parent.kind(), JsxOpeningElement | JsxSelfClosingElement)
            && parent.field("name") == node
    })
}

pub fn is_tagged_template_expression(node: Node) -> bool {
    node.kind() == CallExpression && node.field("arguments").kind() == TemplateString
}

pub fn get_template_string_chunks<'a>(
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> TemplateStringChunks<'a> {
    assert_kind!(node, TemplateString);
    TemplateStringChunks::new(node, context)
}

pub struct TemplateStringChunks<'a> {
    node: Node<'a>,
    node_text: Cow<'a, str>,
    cursor: TreeCursor<'a>,
    next_byte_index: usize,
    has_seen_start_backtick: bool,
    has_seen_end_backtick: bool,
}

impl<'a> TemplateStringChunks<'a> {
    pub fn new(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> Self {
        let mut cursor = node.walk();
        assert!(cursor.goto_first_child());
        Self {
            node,
            node_text: node.text(context),
            cursor,
            next_byte_index: node.start_byte(),
            has_seen_start_backtick: Default::default(),
            has_seen_end_backtick: Default::default(),
        }
    }

    fn get_current_chunk_and_advance(
        &mut self,
        chunk_end_byte: usize,
        next_next_byte_index: usize,
    ) -> (Cow<'a, str>, usize) {
        let chunk_start_byte = self.next_byte_index;
        let chunk = self.node_text.sliced(|_| {
            chunk_start_byte - self.node.start_byte()..chunk_end_byte - self.node.start_byte()
        });
        self.next_byte_index = next_next_byte_index;
        (chunk, chunk_start_byte)
    }
}

impl<'a> Iterator for TemplateStringChunks<'a> {
    type Item = (Cow<'a, str>, usize);

    fn next(&mut self) -> Option<Self::Item> {
        if self.has_seen_end_backtick {
            return None;
        }
        assert!(self.next_byte_index < self.node.end_byte());
        loop {
            match self.cursor.node().kind() {
                EscapeSequence => {
                    assert!(self.cursor.goto_next_sibling());
                    continue;
                }
                TemplateSubstitution => {
                    let ret = self.get_current_chunk_and_advance(
                        self.cursor.node().start_byte(),
                        self.cursor.node().end_byte(),
                    );
                    assert!(self.cursor.goto_next_sibling());
                    return Some(ret);
                }
                "`" => {
                    if self.has_seen_start_backtick {
                        self.has_seen_end_backtick = true;
                        return Some(self.get_current_chunk_and_advance(
                            self.cursor.node().start_byte(),
                            self.cursor.node().end_byte(),
                        ));
                    } else {
                        self.has_seen_start_backtick = true;
                        assert!(self.cursor.node().start_byte() == self.node.start_byte());
                        assert!(self.cursor.node().end_byte() == self.node.start_byte() + 1);
                        assert!(self.next_byte_index == self.node.start_byte());
                        self.next_byte_index += 1;
                    }
                }
                _ => unreachable!(),
            }
            assert!(self.cursor.goto_next_sibling());
        }
    }
}

pub fn is_simple_template_literal(node: Node) -> bool {
    node.kind() == TemplateString
        && !node
            .non_comment_named_children(SupportedLanguage::Javascript)
            .any(|child| child.kind() == TemplateSubstitution)
}

pub fn is_export_default(node: Node) -> bool {
    if node.kind() != ExportStatement {
        return false;
    }
    node.non_comment_children(SupportedLanguage::Javascript)
        .skip_while(|child| child.kind() != "export")
        .nth(1)
        .unwrap()
        .kind()
        == "default"
}

pub fn get_num_import_specifiers(node: Node) -> usize {
    assert_kind!(node, ImportClause);
    let mut named_children = node.non_comment_named_children(SupportedLanguage::Javascript);
    let first_child = named_children.next().unwrap();
    match first_child.kind() {
        NamespaceImport => {
            assert!(named_children.next().is_none());
            1
        }
        NamedImports => {
            assert!(named_children.next().is_none());
            first_child.num_non_comment_named_children(SupportedLanguage::Javascript)
        }
        Identifier => {
            1 + named_children.next().map_or_default(|next_child| {
                assert!(named_children.next().is_none());
                match next_child.kind() {
                    NamespaceImport => 1,
                    NamedImports => {
                        next_child.num_non_comment_named_children(SupportedLanguage::Javascript)
                    }
                    _ => unreachable!(),
                }
            })
        }
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn test_get_cooked_value() {
        for (input, expected) in [
            ("abc", Cow::Borrowed("abc")),
            ("", Cow::Borrowed("")),
            // from acorn/test/tests-harmony.js
            (
                "\\n\\r\\b\\v\\t\\f\\\n\\\r\n\\\u{2028}\\\u{2029}",
                Cow::Owned("\n\r\u{0008}\u{000b}\t\u{000c}".to_owned()),
            ),
            (
                "\\u{000042}\\u0042\\x42u0\\A",
                Cow::Owned("BBBu0A".to_owned()),
            ),
        ] {
            assert_that!(&get_cooked_value(input /* , false */)).is_equal_to(expected);
        }
    }
}
