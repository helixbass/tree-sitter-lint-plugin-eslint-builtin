use std::{borrow::Cow, cell::RefCell, collections::HashSet, sync::Arc};

use id_arena::Id;
use once_cell::sync::Lazy;
use regex::Captures;
use regexpp_js::{
    visit_reg_exp_ast, visitor, AllArenas, NodeInterface, RegExpParser, ValidatePatternFlags, Wtf16,
};
use squalid::{regex, OptionExt};
use tree_sitter_lint::{
    rule,
    tree_sitter::{Node, Point, Range},
    violation, NodeExt, QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::{get_template_string_chunks, is_tagged_template_expression},
    utils::ast_utils,
};

static VALID_STRING_ESCAPES: Lazy<HashSet<char>> = Lazy::new(|| {
    ['\\', 'n', 'r', 'v', 't', 'b', 'f', 'u', 'x']
        .into_iter()
        .chain(ast_utils::LINE_BREAK_SINGLE_CHARS.iter().copied())
        .collect()
});

static REGEX_GENERAL_ESCAPES: Lazy<HashSet<char>> = Lazy::new(|| {
    [
        '\\', 'b', 'c', 'd', 'D', 'f', 'n', 'p', 'P', 'r', 's', 'S', 't', 'v', 'w', 'W', 'x', 'u',
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    ]
    .into()
});

static REGEX_NON_CHARCLASS_ESCAPES: Lazy<HashSet<char>> = Lazy::new(|| {
    REGEX_GENERAL_ESCAPES
        .iter()
        .copied()
        .chain([
            '^', '/', '.', '$', '*', '+', '?', '[', '{', '}', '|', '(', ')', 'B', 'k',
        ])
        .collect()
});

static REGEX_CLASSSET_CHARACTER_ESCAPES: Lazy<HashSet<char>> = Lazy::new(|| {
    REGEX_GENERAL_ESCAPES
        .iter()
        .copied()
        .chain(['q', '/', '[', '{', '}', '|', '(', ')', '-'])
        .collect()
});

static REGEX_CLASS_SET_RESERVED_DOUBLE_PUNCTUATOR: Lazy<HashSet<char>> = Lazy::new(|| {
    [
        '!', '#', '$', '%', '&', '*', '+', ',', '.', ':', ';', '<', '=', '>', '?', '@', '^', '`',
        '~',
    ]
    .into()
});

fn report<'a>(
    node: Node<'a>,
    start_byte: usize,
    character: String,
    context: &QueryMatchContext<'a, '_>,
) {
    let range_start = start_byte;
    let start_offset = range_start - node.start_byte();
    let range = [range_start, range_start + 1];

    context.report(violation! {
        node => node,
        range => Range {
            start_byte: range[0],
            end_byte: range[1],
            // TODO: this may not be a valid assumption?
            start_point: Point {
                row: node.start_position().row,
                column: node.start_position().column + start_offset,
            },
            end_point: Point {
                row: node.start_position().row,
                column: node.start_position().column + start_offset + 1,
            },
        },
        message_id => "unnecessary_escape",
        data => {
            character => character,
        },
        // TODO: suggestions?
    });
}

fn validate_string<'a>(
    node: Node<'a>,
    captures: Captures<'_>,
    template_chunk_and_start: Option<&(Cow<'a, str>, usize)>,
    context: &QueryMatchContext<'a, '_>,
) {
    let is_template_element = template_chunk_and_start.is_some();
    let escaped_char = captures[0].chars().nth(1).unwrap();
    let mut is_unnecessary_escape = !VALID_STRING_ESCAPES.contains(&escaped_char);
    let is_quote_escape;

    if is_template_element {
        is_quote_escape = escaped_char == '`';

        let template_chunk_and_start = template_chunk_and_start.unwrap();
        match escaped_char {
            '$' => {
                let next_char_after_escaped_start = captures.get(0).unwrap().start() + 2;
                is_unnecessary_escape =
                    if next_char_after_escaped_start >= template_chunk_and_start.0.len() {
                        true
                    } else {
                        &template_chunk_and_start.0
                            [next_char_after_escaped_start..next_char_after_escaped_start + 1]
                            != "{"
                    };
            }
            '{' => {
                let match_start = captures.get(0).unwrap().start();
                is_unnecessary_escape = if match_start == 0 {
                    true
                } else {
                    &template_chunk_and_start.0[match_start - 1..match_start] != "$"
                };
            }
            _ => (),
        }
    } else {
        is_quote_escape = escaped_char == node.text(context).chars().next().unwrap();
    }

    if is_unnecessary_escape && !is_quote_escape {
        report(
            node,
            template_chunk_and_start
                .map(|template_chunk_and_start| template_chunk_and_start.1)
                .unwrap_or_else(|| node.start_byte())
                + captures.get(0).unwrap().start(),
            captures[0][1..].to_owned(),
            context,
        );
    }
}

fn check<'a>(
    node: Node<'a>,
    template_chunk_and_start: Option<(Cow<'a, str>, usize)>,
    context: &QueryMatchContext<'a, '_>,
) {
    let is_template_element = template_chunk_and_start.is_some();

    if is_template_element
        && node.parent().matches(|parent| {
            parent.parent().matches(|parent_parent| {
                is_tagged_template_expression(parent_parent)
                    && parent_parent.field("function") == parent
            })
        })
    {
        return;
    }

    // if matches!(
    //     node.parent().unwrap().kind(),
    //     JSX
    // )

    let value = template_chunk_and_start
        .as_ref()
        .map(|template_chunk_and_start| template_chunk_and_start.0.clone())
        .unwrap_or_else(|| node.text(context));

    for captures in regex!(r#"\\[^\d]"#).captures_iter(&value) {
        validate_string(node, captures, template_chunk_and_start.as_ref(), context);
    }
}

pub fn no_useless_escape_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-useless-escape",
        languages => [Javascript],
        messages => [
            unnecessary_escape => "Unnecessary escape character: \\{{character}}.",
            remove_escape => "Remove the `\\`. This maintains the current functionality.",
            remove_escape_do_not_keep_semantics => "Remove the `\\` if it was inserted by mistake.",
            escape_backslash => "Replace the `\\` with `\\\\` to include the actual backslash character.",
        ],
        listeners => [
            r#"
              (template_string) @c
            "# => |node, context| {
                for (chunk, chunk_start) in get_template_string_chunks(node, context) {
                    // println!("no_useless_escape_rule() 1 chunk: {chunk:#?}, chunk_start: {chunk_start:#?}");
                    check(node, Some((chunk, chunk_start)), context);
                }
            },
            r#"
              (string) @c
            "# => |node, context| {
                check(node, None, context);
            },
            r#"
              (regex) @c
            "# => |node, context| {
                let pattern = node.field("pattern").text(context);
                let flags = node.child_by_field_name("flags").map(|flags| flags.text(context));
                let unicode = flags.as_ref().matches(|flags| flags.contains('u'));
                let unicode_sets = flags.as_ref().matches(|flags| flags.contains('v'));

                let arena = AllArenas::default();
                let mut parser = RegExpParser::new(&arena, None);
                let pattern_as_wtf16: Wtf16 = (&*pattern).into();
                let Ok(pattern_node) = parser.parse_pattern(
                    &pattern_as_wtf16,
                    Some(0),
                    Some(pattern_as_wtf16.len()),
                    Some(ValidatePatternFlags {
                        unicode: Some(unicode),
                        unicode_sets: Some(unicode_sets),
                    }),
                ) else {
                    return;
                };

                struct Handlers<'a, 'b, 'c> {
                    arena: &'b AllArenas,
                    pattern_as_wtf16: &'b Wtf16,
                    unicode_sets: bool,
                    character_class_stack: RefCell<Vec<Id<regexpp_js::Node>>>,
                    node: Node<'a>,
                    context: &'b QueryMatchContext<'a, 'c>,
                }

                impl<'a, 'b, 'c> Handlers<'a, 'b, 'c> {
                    pub fn new(
                        arena: &'b AllArenas,
                        pattern_as_wtf16: &'b Wtf16,
                        unicode_sets: bool,
                        node: Node<'a>,
                        context: &'b QueryMatchContext<'a, 'c>,
                    ) -> Self {
                        Self {
                            arena,
                            pattern_as_wtf16,
                            unicode_sets,
                            character_class_stack: Default::default(),
                            node,
                            context,
                        }
                    }
                }

                impl<'a, 'b, 'c> visitor::Handlers for Handlers<'a, 'b, 'c> {
                    fn on_character_class_enter(&self, character_class_node: Id<regexpp_js::Node /*CharacterClass*/>) {
                        self.character_class_stack.borrow_mut().insert(0, character_class_node);
                    }

                    fn on_character_class_leave(&self, _node: Id<regexpp_js::Node /*CharacterClass*/>) {
                        self.character_class_stack.borrow_mut().remove(0);
                    }

                    fn on_expression_character_class_enter(&self, character_class_node: Id<regexpp_js::Node /*ExpressionCharacterClass*/>) {
                        self.character_class_stack.borrow_mut().insert(0, character_class_node);
                    }

                    fn on_expression_character_class_leave(&self, _node: Id<regexpp_js::Node /*ExpressionCharacterClass*/>) {
                        self.character_class_stack.borrow_mut().remove(0);
                    }

                    fn on_character_enter(&self, character_node: Id<regexpp_js::Node /*Character*/>) {
                        let character_node_ref = self.arena.node(character_node);
                        let character_node_raw = character_node_ref.raw();
                        if character_node_raw[0] != u16::try_from('\\').unwrap() {
                            return;
                        }

                        let escaped_char = &character_node_raw[1..];
                        if escaped_char != *Wtf16::from(character_node_ref.as_character().value) {
                            return;
                        }

                        let character_class_stack = self.character_class_stack.borrow();
                        let allowed_escapes = if !character_class_stack.is_empty() {
                            if self.unicode_sets {
                                &REGEX_CLASSSET_CHARACTER_ESCAPES
                            } else {
                                &REGEX_GENERAL_ESCAPES
                            }
                        } else {
                            &REGEX_NON_CHARCLASS_ESCAPES
                        };
                        let escaped_char_as_wtf_16 = escaped_char;
                        let escaped_char = char::try_from(&Wtf16::from(escaped_char_as_wtf_16)).unwrap();
                        if allowed_escapes.contains(&escaped_char) {
                            return;
                        }

                        let reported_index = character_node_ref.start() + 1;
                        // let mut disable_escape_backslash_suggest = false;

                        if !character_class_stack.is_empty() {
                            let character_class_node = character_class_stack[0];
                            let character_class_node_ref = self.arena.node(character_class_node);

                            #[allow(clippy::collapsible_if)]
                            if escaped_char == '^' {
                                if character_class_node_ref.start() + 1 == character_node_ref.start() {
                                    return;
                                }
                            }
                            #[allow(clippy::collapsible_else_if)]
                            if !self.unicode_sets {
                                #[allow(clippy::collapsible_if)]
                                if escaped_char == '-' {
                                    if character_class_node_ref.start() + 1 != character_node_ref.start() &&
                                        character_node_ref.end() != character_class_node_ref.end() - 1 {
                                        return;
                                    }
                                }
                            } else {
                                if REGEX_CLASS_SET_RESERVED_DOUBLE_PUNCTUATOR.contains(&escaped_char) {
                                    if self.pattern_as_wtf16[character_node_ref.end()] == escaped_char_as_wtf_16[0] {
                                        return;
                                    }
                                    if self.pattern_as_wtf16[character_node_ref.start() - 1] == escaped_char_as_wtf_16[0] {
                                        if escaped_char != '^' {
                                            return;
                                        }

                                        if !character_class_node_ref.as_character_class().negate {
                                            return;
                                        }
                                        let negate_caret_index = character_class_node_ref.start() + 1;

                                        if negate_caret_index < character_node_ref.start() - 1 {
                                            return;
                                        }
                                    }
                                }

                                // if matches!(
                                //     &*self.arena.node(character_node_ref.parent()),
                                //     regexpp_js::Node::ClassIntersection(_) |
                                //     regexpp_js::Node::ClassSubtraction(_)
                                // ) {
                                //     disable_escape_backslash_suggest = true;
                                // }
                            }
                        }

                        report(
                            self.node,
                            reported_index,
                            escaped_char.into(),
                            self.context,
                        );
                    }
                }

                let handlers = Handlers::new(&arena, &pattern_as_wtf16, unicode_sets, node, context);

                visit_reg_exp_ast(
                    pattern_node,
                    &handlers,
                    &arena,
                );
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{kind, kind::TemplateString};

    #[test]
    fn test_no_useless_escape_rule() {
        RuleTester::run(
            no_useless_escape_rule(),
            rule_tests! {
                valid => [
                    "var foo = /\\./",
                    "var foo = /\\//g",
                    "var foo = /\"\"/",
                    "var foo = /''/",
                    "var foo = /([A-Z])\\t+/g",
                    "var foo = /([A-Z])\\n+/g",
                    "var foo = /([A-Z])\\v+/g",
                    "var foo = /\\D/",
                    "var foo = /\\W/",
                    "var foo = /\\w/",
                    "var foo = /\\B/",
                    "var foo = /\\\\/g",
                    "var foo = /\\w\\$\\*\\./",
                    "var foo = /\\^\\+\\./",
                    "var foo = /\\|\\}\\{\\./",
                    "var foo = /]\\[\\(\\)\\//",
                    "var foo = \"\\x123\"",
                    "var foo = \"\\u00a9\"",
                    "var foo = \"\\377\"",
                    "var foo = \"\\\"\"",
                    "var foo = \"xs\\u2111\"",
                    "var foo = \"foo \\\\ bar\";",
                    "var foo = \"\\t\";",
                    "var foo = \"foo \\b bar\";",
                    "var foo = '\\n';",
                    "var foo = 'foo \\r bar';",
                    "var foo = '\\v';",
                    "var foo = '\\f';",
                    "var foo = '\\\n';",
                    "var foo = '\\\r\n';",
                    { code => "<foo attr=\"\\d\"/>", /*environment => { parserOptions: { ecmaFeatures: { jsx: true } } }*/ },
                    { code => "<div> Testing: \\ </div>", /*environment => { parserOptions: { ecmaFeatures: { jsx: true } } }*/ },
                    { code => "<div> Testing: &#x5C </div>", /*environment => { parserOptions: { ecmaFeatures: { jsx: true } } }*/ },
                    { code => "<foo attr='\\d'></foo>", /*environment => { parserOptions: { ecmaFeatures: { jsx: true } } }*/ },
                    { code => "<> Testing: \\ </>", /*environment => { parserOptions: { ecmaFeatures: { jsx: true } } }*/ },
                    { code => "<> Testing: &#x5C </>", /*environment => { parserOptions: { ecmaFeatures: { jsx: true } } }*/ },
                    { code => "var foo = `\\x123`", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\u00a9`", environment => { ecma_version => 6 } },
                    { code => "var foo = `xs\\u2111`", environment => { ecma_version => 6 } },
                    { code => "var foo = `foo \\\\ bar`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\t`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `foo \\b bar`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\n`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `foo \\r bar`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\v`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\f`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\\n`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\\r\n`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo} \\x123`", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo} \\u00a9`", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo} xs\\u2111`", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo} \\\\ ${bar}`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo} \\b ${bar}`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo}\\t`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo}\\n`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo}\\r`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo}\\v`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo}\\f`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo}\\\n`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `${foo}\\\r\n`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\``", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\`${foo}\\``", environment => { ecma_version => 6 } },
                    { code => "var foo = `\\${{${foo}`;", environment => { ecma_version => 6 } },
                    { code => "var foo = `$\\{{${foo}`;", environment => { ecma_version => 6 } },
                    { code => "var foo = String.raw`\\.`", environment => { ecma_version => 6 } },
                    { code => "var foo = myFunc`\\.`", environment => { ecma_version => 6 } },

                    r#"var foo = /[\d]/"#,
                    r#"var foo = /[a\-b]/"#,
                    r#"var foo = /foo\?/"#,
                    r#"var foo = /example\.com/"#,
                    r#"var foo = /foo\|bar/"#,
                    r#"var foo = /\^bar/"#,
                    r#"var foo = /[\^bar]/"#,
                    r#"var foo = /\(bar\)/"#,
                    r#"var foo = /[[\]]/"#, // A character class containing '[' and ']'
                    r#"var foo = /[[]\./"#, // A character class containing '[', followed by a '.' character
                    r#"var foo = /[\]\]]/"#, // A (redundant) character class containing ']'
                    r#"var foo = /\[abc]/"#, // Matches the literal string '[abc]'
                    r#"var foo = /\[foo\.bar]/"#, // Matches the literal string '[foo.bar]'
                    r#"var foo = /vi/m"#,
                    r#"var foo = /\B/"#,

                    // https://github.com/eslint/eslint/issues/7472
                    r#"var foo = /\0/"#, // null character
                    "var foo = /\\1/", // \x01 character (octal literal)
                    "var foo = /(a)\\1/", // backreference
                    "var foo = /(a)\\12/", // backreference
                    "var foo = /[\\0]/", // null character in character class

                    "var foo = 'foo \\\u{2028} bar'",
                    "var foo = 'foo \\\u{2029} bar'",

                    // https://github.com/eslint/eslint/issues/7789
                    r#"/]/"#,
                    r#"/\]/"#,
                    { code => r#"/\]/u"#, environment => { ecma_version => 6 } },
                    r#"var foo = /foo\]/"#,
                    r#"var foo = /[[]\]/"#, // A character class containing '[', followed by a ']' character
                    r#"var foo = /\[foo\.bar\]/"#,

                    // ES2018
                    { code => r#"var foo = /(?<a>)\k<a>/"#, environment => { ecma_version => 2018 } },
                    { code => r#"var foo = /(\\?<a>)/"#, environment => { ecma_version => 2018 } },
                    { code => r#"var foo = /\p{ASCII}/u"#, environment => { ecma_version => 2018 } },
                    { code => r#"var foo = /\P{ASCII}/u"#, environment => { ecma_version => 2018 } },
                    { code => r#"var foo = /[\p{ASCII}]/u"#, environment => { ecma_version => 2018 } },
                    { code => r#"var foo = /[\P{ASCII}]/u"#, environment => { ecma_version => 2018 } },

                    // Carets
                    r#"/[^^]/"#,
                    { code => r#"/[^^]/u"#, environment => { ecma_version => 2015 } },

                    // ES2024
                    { code => r#"/[\q{abc}]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\(]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\)]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\{]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\]]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\}]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\/]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\-]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\|]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\$$]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\&&]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\!!]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\##]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\%%]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\**]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\++]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\,,]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\..]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\::]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\;;]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\<<]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\==]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\>>]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\??]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\@@]/v"#, environment => { ecma_version => 2024 } },
                    { code => "/[\\``]/v", environment => { ecma_version => 2024 } },
                    { code => r#"/[\~~]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[^\^^]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[_\^^]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[$\$]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[&\&]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[!\!]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[#\#]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[%\%]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[*\*]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[+\+]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[,\,]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[.\.]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[:\:]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[;\;]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[<\<]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[=\=]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[>\>]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[?\?]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[@\@]/v"#, environment => { ecma_version => 2024 } },
                    { code => "/[`\\`]/v", environment => { ecma_version => 2024 } },
                    { code => r#"/[~\~]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[^^\^]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[_^\^]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\&&&\&]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[[\-]\-]/v"#, environment => { ecma_version => 2024 } },
                    { code => r#"/[\^]/v"#, environment => { ecma_version => 2024 } }
                ],
                invalid => [
                    {
                        code => "var foo = /\\#/;",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\#.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = /#/;"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = /\\\\#/;"
                            // }]
                        }],
                    },
                    {
                        code => "var foo = /\\;/;",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\;.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = /;/;"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = /\\\\;/;"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = \"\\'\";",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\'.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = \"'\";"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = \"\\\\'\";"
                            // }]
                        }],
                    },
                    {
                        code => "var foo = \"\\#/\";",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\#.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = \"#/\";"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = \"\\\\#/\";"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = \"\\a\"",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\a.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = \"a\""
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = \"\\\\a\""
                            // }]
                        }],
                    },
                    {
                        code => "var foo = \"\\B\";",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\B.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = \"B\";"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = \"\\\\B\";"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = \"\\@\";",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\@.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = \"@\";"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = \"\\\\@\";"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = \"foo \\a bar\";",
                        errors => [{
                            line => 1,
                            column => 16,
                            end_column => 17,
                            message => "Unnecessary escape character: \\a.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = \"foo a bar\";"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = \"foo \\\\a bar\";"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = '\\\"';",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\\".",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = '\"';"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = '\\\\\"';"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = '\\#';",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\#.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = '#';"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = '\\\\#';"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = '\\$';",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\$.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = '$';"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = '\\\\$';"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = '\\p';",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\p.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = 'p';"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = '\\\\p';"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = '\\p\\a\\@';",
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                end_column => 13,
                                message => "Unnecessary escape character: \\p.",
                                type => kind::String,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = 'p\\a\\@';"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = '\\\\p\\a\\@';"
                                // }]
                            },
                            {
                                line => 1,
                                column => 14,
                                end_column => 15,
                                message => "Unnecessary escape character: \\a.",
                                type => kind::String,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = '\\pa\\@';"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = '\\p\\\\a\\@';"
                                // }]
                            },
                            {
                                line => 1,
                                column => 16,
                                end_column => 17,
                                message => "Unnecessary escape character: \\@.",
                                type => kind::String,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = '\\p\\a@';"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = '\\p\\a\\\\@';"
                                // }]
                            }
                        ]
                    },
                    {
                        code => "<foo attr={\"\\d\"}/>",
                        // environment => { parserOptions: { ecmaFeatures: { jsx: true } } },
                        errors => [{
                            line => 1,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\d.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "<foo attr={\"d\"}/>"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "<foo attr={\"\\\\d\"}/>"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = '\\`';",
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\`.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = '`';"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = '\\\\`';"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = `\\\"`;",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\\".",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = `\"`;"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = `\\\\\"`;"
                            // }]
                        }],
                    },
                    {
                        code => "var foo = `\\'`;",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\'.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = `'`;"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = `\\\\'`;"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = `\\#`;",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\#.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = `#`;"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = `\\\\#`;"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = '\\`foo\\`';",
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                end_column => 13,
                                message => "Unnecessary escape character: \\`.",
                                type => kind::String,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = '`foo\\`';"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = '\\\\`foo\\`';"
                                // }]
                            },
                            {
                                line => 1,
                                column => 17,
                                end_column => 18,
                                message => "Unnecessary escape character: \\`.",
                                type => kind::String,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = '\\`foo`';"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = '\\`foo\\\\`';"
                                // }]
                            }
                        ]
                    },
                    {
                        code => "var foo = `\\\"${foo}\\\"`;",
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                end_column => 13,
                                message => "Unnecessary escape character: \\\".",
                                type => TemplateString,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = `\"${foo}\\\"`;"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = `\\\\\"${foo}\\\"`;"
                                // }]
                            },
                            {
                                line => 1,
                                column => 20,
                                end_column => 21,
                                message => "Unnecessary escape character: \\\".",
                                type => TemplateString,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = `\\\"${foo}\"`;"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = `\\\"${foo}\\\\\"`;"
                                // }]
                            }
                        ]
                    },
                    {
                        code => "var foo = `\\'${foo}\\'`;",
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                end_column => 13,
                                message => "Unnecessary escape character: \\'.",
                                type => TemplateString,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = `'${foo}\\'`;"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = `\\\\'${foo}\\'`;"
                                // }]
                            },
                            {
                                line => 1,
                                column => 20,
                                end_column => 21,
                                message => "Unnecessary escape character: \\'.",
                                type => TemplateString,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = `\\'${foo}'`;"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = `\\'${foo}\\\\'`;"
                                // }]
                            }
                        ]
                    },
                    {
                        code => "var foo = `\\#${foo}`;",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\#.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "var foo = `#${foo}`;"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "var foo = `\\\\#${foo}`;"
                            // }]
                        }]
                    },
                    {
                        code => "let foo = '\\ ';",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\ .",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "let foo = ' ';"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "let foo = '\\\\ ';"
                            // }]
                        }]
                    },
                    {
                        code => "let foo = /\\ /;",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\ .",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "let foo = / /;"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "let foo = /\\\\ /;"
                            // }]
                        }]
                    },
                    {
                        code => "var foo = `\\$\\{{${foo}`;",
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                end_column => 13,
                                message => "Unnecessary escape character: \\$.",
                                type => TemplateString,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = `$\\{{${foo}`;"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = `\\\\$\\{{${foo}`;"
                                // }]
                            }
                        ]
                    },
                    {
                        code => "var foo = `\\$a${foo}`;",
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                end_column => 13,
                                message => "Unnecessary escape character: \\$.",
                                type => TemplateString,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = `$a${foo}`;"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = `\\\\$a${foo}`;"
                                // }]
                            }
                        ]
                    },
                    {
                        code => "var foo = `a\\{{${foo}`;",
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                line => 1,
                                column => 13,
                                end_column => 14,
                                message => "Unnecessary escape character: \\{.",
                                type => TemplateString,
                                // suggestions: [{
                                //     messageId: "removeEscape",
                                //     output: "var foo = `a{{${foo}`;"
                                // }, {
                                //     messageId: "escapeBackslash",
                                //     output: "var foo = `a\\\\{{${foo}`;"
                                // }]
                            }
                        ]
                    },
                    {
                        code => r#"var foo = /[ab\-]/"#,
                        errors => [{
                            line => 1,
                            column => 15,
                            end_column => 16,
                            message => "Unnecessary escape character: \\-.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[ab-]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[ab\\-]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[\-ab]/"#,
                        errors => [{
                            line => 1,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\-.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[-ab]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[\\-ab]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[ab\?]/"#,
                        errors => [{
                            line => 1,
                            column => 15,
                            end_column => 16,
                            message => "Unnecessary escape character: \\?.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[ab?]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[ab\\?]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[ab\.]/"#,
                        errors => [{
                            line => 1,
                            column => 15,
                            end_column => 16,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[ab.]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[ab\\.]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[a\|b]/"#,
                        errors => [{
                            line => 1,
                            column => 14,
                            end_column => 15,
                            message => "Unnecessary escape character: \\|.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[a|b]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[a\\|b]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /\-/"#,
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\-.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /-/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /\\-/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[\-]/"#,
                        errors => [{
                            line => 1,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\-.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[-]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[\\-]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[ab\$]/"#,
                        errors => [{
                            line => 1,
                            column => 15,
                            end_column => 16,
                            message => "Unnecessary escape character: \\$.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[ab$]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[ab\\$]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[\(paren]/"#,
                        errors => [{
                            line => 1,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\(.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[(paren]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[\\(paren]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[\[]/"#,
                        errors => [{
                            line => 1,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\[.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[[]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[\\[]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[\/]/"#, // A character class containing '/'
                        errors => [{
                            line => 1,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\/.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[/]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[\\/]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[\B]/"#,
                        errors => [{
                            line => 1,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\B.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[B]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[\\B]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[a][\-b]/"#,
                        errors => [{
                            line => 1,
                            column => 16,
                            end_column => 17,
                            message => "Unnecessary escape character: \\-.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[a][-b]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[a][\\-b]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /\-[]/"#,
                        errors => [{
                            line => 1,
                            column => 12,
                            end_column => 13,
                            message => "Unnecessary escape character: \\-.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /-[]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /\\-[]/"#
                            // }]
                        }]
                    },
                    {
                        code => r#"var foo = /[a\^]/"#,
                        errors => [{
                            line => 1,
                            column => 14,
                            end_column => 15,
                            message => "Unnecessary escape character: \\^.",
                            type => kind::Regex,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: r#"var foo = /[a^]/"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"var foo = /[a\\^]/"#
                            // }]
                        }]
                    },
                    {
                        code => "`multiline template\nliteral with useless \\escape`",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 2,
                            column => 22,
                            end_column => 23,
                            message => "Unnecessary escape character: \\e.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "`multiline template\nliteral with useless escape`"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "`multiline template\nliteral with useless \\\\escape`"
                            // }]
                        }]
                    },
                    {
                        code => "`multiline template\r\nliteral with useless \\escape`",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 2,
                            column => 22,
                            end_column => 23,
                            message => "Unnecessary escape character: \\e.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "`multiline template\r\nliteral with useless escape`"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "`multiline template\r\nliteral with useless \\\\escape`"
                            // }]
                        }]
                    },
                    {
                        code => "`template literal with line continuation \\\nand useless \\escape`",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 2,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\e.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "`template literal with line continuation \\\nand useless escape`"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "`template literal with line continuation \\\nand useless \\\\escape`"
                            // }]
                        }]
                    },
                    {
                        code => "`template literal with line continuation \\\r\nand useless \\escape`",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 2,
                            column => 13,
                            end_column => 14,
                            message => "Unnecessary escape character: \\e.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "`template literal with line continuation \\\r\nand useless escape`"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "`template literal with line continuation \\\r\nand useless \\\\escape`"
                            // }]
                        }]
                    },
                    {
                        code => "`template literal with mixed linebreaks \r\r\n\n\\and useless escape`",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 4,
                            column => 1,
                            end_column => 2,
                            message => "Unnecessary escape character: \\a.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "`template literal with mixed linebreaks \r\r\n\nand useless escape`"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "`template literal with mixed linebreaks \r\r\n\n\\\\and useless escape`"
                            // }]
                        }]
                    },
                    {
                        code => "`template literal with mixed linebreaks in line continuations \\\n\\\r\\\r\n\\and useless escape`",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 4,
                            column => 1,
                            end_column => 2,
                            message => "Unnecessary escape character: \\a.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "`template literal with mixed linebreaks in line continuations \\\n\\\r\\\r\nand useless escape`"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "`template literal with mixed linebreaks in line continuations \\\n\\\r\\\r\n\\\\and useless escape`"
                            // }]
                        }]
                    },
                    {
                        code => "`\\a```",
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 2,
                            end_column => 3,
                            message => "Unnecessary escape character: \\a.",
                            type => TemplateString,
                            // suggestions: [{
                            //     messageId: "removeEscape",
                            //     output: "`a```"
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: "`\\\\a```"
                            // }]
                        }]
                    },

                    // https://github.com/eslint/eslint/issues/16988
                    {
                        code => r#""use\ strict";"#,
                        errors => [{
                            line => 1,
                            column => 5,
                            end_column => 6,
                            message => "Unnecessary escape character: \\ .",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscapeDoNotKeepSemantics",
                            //     output: r#""use strict";"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#""use\\ strict";"#
                            // }]
                        }]
                    },
                    {
                        code => r#"({ foo() { "foo"; "bar"; "ba\z" } })"#,
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 29,
                            end_column => 30,
                            message => "Unnecessary escape character: \\z.",
                            type => kind::String,
                            // suggestions: [{
                            //     messageId: "removeEscapeDoNotKeepSemantics",
                            //     output: r#"({ foo() { "foo"; "bar"; "baz" } })"#
                            // }, {
                            //     messageId: "escapeBackslash",
                            //     output: r#"({ foo() { "foo"; "bar"; "ba\\z" } })"#
                            // }]
                        }]
                    },

                    // Carets
                    {
                        code => r#"/[^\^]/"#,
                        errors => [{
                            line => 1,
                            column => 4,
                            message => "Unnecessary escape character: \\^.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: "/[^^]/"
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[^\\^]/"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[^\^]/u"#,
                        environment => { ecma_version => 2015 },
                        errors => [{
                            line => 1,
                            column => 4,
                            message => "Unnecessary escape character: \\^.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: "/[^^]/u"
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[^\\^]/u"#
                            //     }
                            // ]
                        }]
                    },

                    // ES2024
                    {
                        code => r#"/[\$]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            end_column => 4,
                            message => "Unnecessary escape character: \\$.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: "/[$]/v"
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\$]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\&\&]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\&.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[&\&]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\&\&]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\!\!]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\!.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[!\!]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\!\!]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\#\#]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\#.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[#\#]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\#\#]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\%\%]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\%.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[%\%]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\%\%]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\*\*]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\*.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[*\*]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\*\*]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\+\+]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\+.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[+\+]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\+\+]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\,\,]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\,.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[,\,]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\,\,]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\.\.]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[.\.]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\.\.]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\:\:]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\:.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[:\:]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\:\:]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\;\;]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\;.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[;\;]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\;\;]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\<\<]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\<.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[<\<]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\<\<]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\=\=]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\=.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[=\=]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\=\=]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\>\>]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\>.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[>\>]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\>\>]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\?\?]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\?.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[?\?]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\?\?]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\@\@]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\@.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[@\@]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\@\@]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "/[\\`\\`]/v",
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\`.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: "/[`\\`]/v"
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: "/[\\\\`\\`]/v"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\~\~]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\~.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[~\~]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\~\~]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[^\^\^]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 4,
                            message => "Unnecessary escape character: \\^.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[^^\^]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[^\\^\^]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[_\^\^]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 4,
                            message => "Unnecessary escape character: \\^.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[_^\^]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[_\\^\^]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\&\&&\&]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\&.",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[&\&&\&]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[\\&\&&\&]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\p{ASCII}--\.]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 14,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[\p{ASCII}--.]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\p{ASCII}&&\.]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 14,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[\p{ASCII}&&.]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\.--[.&]]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[.--[.&]]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\.&&[.&]]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[.&&[.&]]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\.--\.--\.]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[.--\.--\.]/v"#
                            //     }
                            // ]
                        }, {
                            line => 1,
                            column => 7,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[\.--.--\.]/v"#
                            //     }
                            // ]
                        }, {
                            line => 1,
                            column => 11,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[\.--\.--.]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[\.&&\.&&\.]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 3,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[.&&\.&&\.]/v"#
                            //     }
                            // ]
                        }, {
                            line => 1,
                            column => 7,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[\.&&.&&\.]/v"#
                            //     }
                            // ]
                        }, {
                            line => 1,
                            column => 11,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[\.&&\.&&.]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[[\.&]--[\.&]]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 4,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[[.&]--[\.&]]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[[\\.&]--[\.&]]/v"#
                            //     }
                            // ]
                        }, {
                            line => 1,
                            column => 11,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[[\.&]--[.&]]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[[\.&]--[\\.&]]/v"#
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => r#"/[[\.&]&&[\.&]]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{
                            line => 1,
                            column => 4,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[[.&]&&[\.&]]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[[\\.&]&&[\.&]]/v"#
                            //     }
                            // ]
                        }, {
                            line => 1,
                            column => 11,
                            message => "Unnecessary escape character: \\..",
                            type => kind::Regex,
                            // suggestions: [
                            //     {
                            //         messageId: "removeEscape",
                            //         output: r#"/[[\.&]&&[.&]]/v"#
                            //     },
                            //     {
                            //         messageId: "escapeBackslash",
                            //         output: r#"/[[\.&]&&[\\.&]]/v"#
                            //     }
                            // ]
                        }]
                    }
                ]
            },
        )
    }
}
