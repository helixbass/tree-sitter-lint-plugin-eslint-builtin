use std::{collections::HashSet, sync::Arc};

use itertools::Itertools;
use regex::{Captures, Regex};
use squalid::{regex, CowExt, CowStrExt, EverythingExt};
use tree_sitter_lint::{
    rule, tree_sitter::Node, violation, Fixer, NodeExt, QueryMatchContext, Rule, SourceTextProvider,
};

use crate::{
    ast_helpers::{get_template_string_chunks, NodeExtJs},
    kind,
    kind::{BinaryExpression, TemplateString},
    utils::ast_utils,
};

fn is_concatenation(node: Node) -> bool {
    node.kind() == BinaryExpression && node.field("operator").kind() == "+"
}

fn get_top_concat_binary_expression(node: Node) -> Node {
    let mut current_node = node;

    while is_concatenation(current_node.next_non_parentheses_ancestor()) {
        current_node = current_node.next_non_parentheses_ancestor();
    }
    current_node
}

fn has_octal_or_non_octal_decimal_escape_sequence(node: Node, context: &QueryMatchContext) -> bool {
    if is_concatenation(node) {
        return has_octal_or_non_octal_decimal_escape_sequence(
            node.field("left").skip_parentheses(),
            context,
        ) || has_octal_or_non_octal_decimal_escape_sequence(
            node.field("right").skip_parentheses(),
            context,
        );
    }

    if node.kind() == kind::String {
        return ast_utils::has_octal_or_non_octal_decimal_escape_sequence(&node.text(context));
    }

    false
}

fn has_string_literal(node: Node) -> bool {
    if is_concatenation(node) {
        return has_string_literal(node.field("right").skip_parentheses())
            || has_string_literal(node.field("left").skip_parentheses());
    }
    ast_utils::is_string_literal(node)
}

fn has_non_string_literal(node: Node) -> bool {
    if is_concatenation(node) {
        return has_non_string_literal(node.field("right").skip_parentheses())
            || has_non_string_literal(node.field("left").skip_parentheses());
    }
    !ast_utils::is_string_literal(node)
}

fn starts_with_template_curly<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
    if node.kind() == BinaryExpression {
        return starts_with_template_curly(node.field("left").skip_parentheses(), context);
    }
    if node.kind() == TemplateString {
        let mut chunks = get_template_string_chunks(node, context);
        let first_chunk = chunks.next().unwrap();
        if chunks.next().is_none() {
            return false;
        }
        return first_chunk.0.is_empty();
    }
    node.kind() != kind::String
}

fn ends_with_template_curly<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
    if node.kind() == BinaryExpression {
        return starts_with_template_curly(node.field("right").skip_parentheses(), context);
    }
    if node.kind() == TemplateString {
        let chunks = get_template_string_chunks(node, context).collect_vec();
        if chunks.len() == 1 {
            return false;
        }
        return chunks.last().unwrap().0.is_empty();
    }
    node.kind() != kind::String
}

fn get_text_between<'a>(
    node1: Node<'a>,
    node2: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> String {
    let mut all_tokens = vec![node1];
    all_tokens.extend(context.get_tokens_between(node1, node2, Option::<fn(Node) -> bool>::None));
    all_tokens.push(node2);

    all_tokens[0..all_tokens.len() - 1]
        .into_iter()
        .enumerate()
        .fold("".to_owned(), |mut accumulator, (index, &token)| {
            accumulator
                .push_str(&context.slice(token.end_byte()..all_tokens[index + 1].start_byte()));
            accumulator
        })
}

fn get_template_literal<'a>(
    current_node: Node<'a>,
    text_before_node: Option<String>,
    text_after_node: Option<String>,
    context: &QueryMatchContext<'a, '_>,
) -> String {
    if current_node.kind() == kind::String {
        let current_node_text = current_node.text(context);
        return format!(
            "`{}`",
            current_node_text
                .sliced(|len| 1..len - 1)
                .map_cow(|text| {
                    regex!(r#"\\*(\$\{|`)"#).replace_all(text, |captures: &Captures| {
                        let matched = &captures[0];
                        if match matched.rfind('\\') {
                            None => true,
                            Some(pos) => pos % 2 == 1,
                        } {
                            return format!("\\{matched}");
                        }
                        matched.to_owned()
                    })
                })
                .map_cow(|text| {
                    Regex::new(&format!(r#"\\{}"#, &current_node_text[0..1]))
                        .unwrap()
                        .replace_all(text, &current_node_text[0..1])
                })
        );
    }

    if current_node.kind() == TemplateString {
        return current_node.text(context).into_owned();
    }

    if is_concatenation(current_node) && has_string_literal(current_node) {
        let plus_sign = context
            .get_first_token_between(
                current_node.field("left").skip_parentheses(),
                current_node.field("right").skip_parentheses(),
                Some(|token: Node| token.kind() == "+"),
            )
            .unwrap();
        let text_before_plus = get_text_between(
            current_node.field("left").skip_parentheses(),
            plus_sign,
            context,
        );
        let text_after_plus = get_text_between(
            plus_sign,
            current_node.field("right").skip_parentheses(),
            context,
        );
        let left_ends_with_curly =
            ends_with_template_curly(current_node.field("left").skip_parentheses(), context);
        let right_starts_with_curly =
            starts_with_template_curly(current_node.field("right").skip_parentheses(), context);

        if left_ends_with_curly {
            return format!(
                "{}{}",
                get_template_literal(
                    current_node.field("left").skip_parentheses(),
                    text_before_node,
                    Some(format!("{text_before_plus}{text_after_plus}")),
                    context
                )
                .thrush(|template_literal| template_literal
                    [0..template_literal.len() - 1]
                    .to_owned()),
                &get_template_literal(
                    current_node.field("right").skip_parentheses(),
                    None,
                    text_after_node,
                    context
                )[1..]
            );
        }
        if right_starts_with_curly {
            return format!(
                "{}{}",
                get_template_literal(
                    current_node.field("left").skip_parentheses(),
                    text_before_node,
                    None,
                    context
                )
                .thrush(|template_literal| template_literal
                    [0..template_literal.len() - 1]
                    .to_owned()),
                &get_template_literal(
                    current_node.field("right").skip_parentheses(),
                    Some(format!("{text_before_plus}{text_after_plus}")),
                    text_after_node,
                    context
                )[1..]
            );
        }

        return format!(
            "{}{}+{}{}",
            get_template_literal(
                current_node.field("left").skip_parentheses(),
                text_before_node,
                None,
                context
            ),
            text_before_plus,
            text_after_plus,
            get_template_literal(
                current_node.field("right").skip_parentheses(),
                text_after_node,
                None,
                context
            ),
        );
    }

    format!(
        "`${{{}{}{}}}`",
        text_before_node.unwrap_or_default(),
        current_node.text(context),
        text_after_node.unwrap_or_default(),
    )
}

fn fix_non_string_binary_expression<'a>(
    fixer: &mut Fixer,
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) {
    let top_binary_expr = get_top_concat_binary_expression(node.next_non_parentheses_ancestor());

    if has_octal_or_non_octal_decimal_escape_sequence(top_binary_expr, context) {
        return;
    }

    fixer.replace_text(
        top_binary_expr,
        get_template_literal(top_binary_expr, None, None, context),
    );
}

pub fn prefer_template_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-template",
        languages => [Javascript],
        messages => [
            unexpected_string_concatenation => "Unexpected string concatenation.",
        ],
        fixable => true,
        state => {
            [per-file-run]
            done: HashSet<usize>,
        },
        listeners => [
            r#"
              (string) @c
              (template_string) @c
            "# => |node, context| {
                let parent = node.next_non_parentheses_ancestor();
                if !is_concatenation(parent) {
                    return;
                }

                let top_binary_expr = get_top_concat_binary_expression(parent);

                if self.done.contains(&top_binary_expr.start_byte()) {
                    return;
                }
                self.done.insert(top_binary_expr.start_byte());

                if has_non_string_literal(top_binary_expr) {
                    context.report(violation! {
                        node => top_binary_expr,
                        message_id => "unexpected_string_concatenation",
                        fix => |fixer| {
                            fix_non_string_binary_expression(fixer, node, context);
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::BinaryExpression;

    #[test]
    fn test_prefer_template_rule() {
        let errors = vec![RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected_string_concatenation")
            .type_(BinaryExpression)
            .build()
            .unwrap()];

        RuleTester::run(
            prefer_template_rule(),
            rule_tests! {
                valid => [
                    "'use strict';",
                    "var foo = 'foo' + '\\0';",
                    "var foo = 'bar';",
                    "var foo = 'bar' + 'baz';",
                    "var foo = foo + +'100';",
                    "var foo = `bar`;",
                    "var foo = `hello, ${name}!`;",

                    // https://github.com/eslint/eslint/issues/3507
                    "var foo = `foo` + `bar` + \"hoge\";",
                    "var foo = `foo` +\n    `bar` +\n    \"hoge\";"
                ],
                invalid => [
                    {
                        code => "var foo = 'hello, ' + name + '!';",
                        output => "var foo = `hello, ${  name  }!`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'baz';",
                        output => "var foo = `${bar  }baz`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + `baz`;",
                        output => "var foo = `${bar  }baz`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = +100 + 'yen';",
                        output => "var foo = `${+100  }yen`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = 'bar' + baz;",
                        output => "var foo = `bar${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '￥' + (n * 1000) + '-'",
                        output => "var foo = `￥${  n * 1000  }-`",
                        errors => errors,
                    },
                    {
                        code => "var foo = 'aaa' + aaa; var bar = 'bbb' + bbb;",
                        output => "var foo = `aaa${  aaa}`; var bar = `bbb${  bbb}`;",
                        errors => [errors[0], errors[0]]
                    },
                    {
                        code => "var string = (number + 1) + 'px';",
                        output => "var string = `${number + 1  }px`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = 'bar' + baz + 'qux';",
                        output => "var foo = `bar${  baz  }qux`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '0 backslashes: ${bar}' + baz;",
                        output => "var foo = `0 backslashes: \\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '1 backslash: \\${bar}' + baz;",
                        output => "var foo = `1 backslash: \\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '2 backslashes: \\\\${bar}' + baz;",
                        output => "var foo = `2 backslashes: \\\\\\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '3 backslashes: \\\\\\${bar}' + baz;",
                        output => "var foo = `3 backslashes: \\\\\\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'this is a backtick: `' + baz;",
                        output => "var foo = `${bar  }this is a backtick: \\`${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'this is a backtick preceded by a backslash: \\`' + baz;",
                        output => "var foo = `${bar  }this is a backtick preceded by a backslash: \\`${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'this is a backtick preceded by two backslashes: \\\\`' + baz;",
                        output => "var foo = `${bar  }this is a backtick preceded by two backslashes: \\\\\\`${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + `${baz}foo`;",
                        output => "var foo = `${bar  }${baz}foo`;",
                        errors => errors,
                    },
                    {
                        code =>
                        "var foo = 'favorites: ' + favorites.map(f => {
    return f.name;
}) + ';';",
                        output =>
                        "var foo = `favorites: ${  favorites.map(f => {
    return f.name;
})  };`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + baz + 'qux';",
                        output => "var foo = `${bar + baz  }qux`;",
                        errors => errors,
                    },
                    {
                        code =>
                        "var foo = 'favorites: ' +
    favorites.map(f => {
        return f.name;
    }) +
';';",
                        output =>
                        "var foo = `favorites: ${ \n    favorites.map(f => {
        return f.name;
    }) 
};`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = /* a */ 'bar' /* b */ + /* c */ baz /* d */ + 'qux' /* e */ ;",
                        output => "var foo = /* a */ `bar${ /* b */  /* c */ baz /* d */  }qux` /* e */ ;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + ('baz') + 'qux' + (boop);",
                        output => "var foo = `${bar  }baz` + `qux${  boop}`;",
                        errors => errors,
                    },
                    {
                        code => "foo + 'unescapes an escaped single quote in a single-quoted string: \\''",
                        output => "`${foo  }unescapes an escaped single quote in a single-quoted string: '`",
                        errors => errors,
                    },
                    {
                        code => "foo + \"unescapes an escaped double quote in a double-quoted string: \\\"\"",
                        output => "`${foo  }unescapes an escaped double quote in a double-quoted string: \"`",
                        errors => errors,
                    },
                    {
                        code => "foo + 'does not unescape an escaped double quote in a single-quoted string: \\\"'",
                        output => "`${foo  }does not unescape an escaped double quote in a single-quoted string: \\\"`",
                        errors => errors,
                    },
                    {
                        code => "foo + \"does not unescape an escaped single quote in a double-quoted string: \\'\"",
                        output => "`${foo  }does not unescape an escaped single quote in a double-quoted string: \\'`",
                        errors => errors,
                    },
                    {
                        code => "foo + 'handles unicode escapes correctly: \\x27'", // "\x27" === "'"
                        output => "`${foo  }handles unicode escapes correctly: \\x27`",
                        errors => errors,
                    },
                    {
                        code => "foo + 'does not autofix octal escape sequence' + '\\033'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + 'does not autofix non-octal decimal escape sequence' + '\\8'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\n other text \\033'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\0\\1'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\08'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\\\033'",
                        output => "`${foo  }\\\\033`",
                        errors => errors,
                    },
                    {
                        code => "foo + '\\0'",
                        output => "`${foo  }\\0`",
                        errors => errors,
                    },

                    // https://github.com/eslint/eslint/issues/15083
                    {
                        code => r#""default-src 'self' https://*.google.com;"
                        + "frame-ancestors 'none';"
                        + "report-to " + foo + ";""#,
                        output => r#"`default-src 'self' https://*.google.com;`
                        + `frame-ancestors 'none';`
                        + `report-to ${  foo  };`"#,
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo",
                        output => "`a` + `b${  foo}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + 'c' + 'd'",
                        output => "`a` + `b${  foo  }c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b + c' + foo + 'd' + 'e'",
                        output => "`a` + `b + c${  foo  }d` + `e`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + 'd')",
                        output => "`a` + `b${  foo  }c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('a' + 'b')",
                        output => "`a` + `b${  foo  }a` + `b`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + 'd') + ('e' + 'f')",
                        output => "`a` + `b${  foo  }c` + `d` + `e` + `f`",
                        errors => errors,
                    },
                    {
                        code => "foo + ('a' + 'b') + ('c' + 'd')",
                        output => "`${foo  }a` + `b` + `c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + foo + ('b' + 'c') + ('d' + bar + 'e')",
                        output => "`a${  foo  }b` + `c` + `d${  bar  }e`",
                        errors => errors,
                    },
                    {
                        code => "foo + ('b' + 'c') + ('d' + bar + 'e')",
                        output => "`${foo  }b` + `c` + `d${  bar  }e`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + 'd' + 'e')",
                        output => "`a` + `b${  foo  }c` + `d` + `e`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + bar + 'd')",
                        output => "`a` + `b${  foo  }c${  bar  }d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + bar + ('d' + 'e') + 'f')",
                        output => "`a` + `b${  foo  }c${  bar  }d` + `e` + `f`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + bar + 'e') + 'f' + test",
                        output => "`a` + `b${  foo  }c${  bar  }e` + `f${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + foo + ('b' + bar + 'c') + ('d' + test)",
                        output => "`a${  foo  }b${  bar  }c` + `d${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + foo + ('b' + 'c') + ('d' + bar)",
                        output => "`a${  foo  }b` + `c` + `d${  bar}`",
                        errors => errors,
                    },
                    {
                        code => "foo + ('a' + bar + 'b') + 'c' + test",
                        output => "`${foo  }a${  bar  }b` + `c${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + '`b`' + c",
                        output => "`a` + `\\`b\\`${  c}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + '`b` + `c`' + d",
                        output => "`a` + `\\`b\\` + \\`c\\`${  d}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + b + ('`c`' + '`d`')",
                        output => "`a${  b  }\\`c\\`` + `\\`d\\``",
                        errors => errors,
                    },
                    {
                        code => "'`a`' + b + ('`c`' + '`d`')",
                        output => "`\\`a\\`${  b  }\\`c\\`` + `\\`d\\``",
                        errors => errors,
                    },
                    {
                        code => "foo + ('`a`' + bar + '`b`') + '`c`' + test",
                        output => "`${foo  }\\`a\\`${  bar  }\\`b\\`` + `\\`c\\`${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('b' + 'c') + d",
                        output => "`a` + `b` + `c${  d}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('`b`' + '`c`') + d",
                        output => "`a` + `\\`b\\`` + `\\`c\\`${  d}`",
                        errors => errors,
                    },
                    {
                        code => "a + ('b' + 'c') + d",
                        output => "`${a  }b` + `c${  d}`",
                        errors => errors,
                    },
                    {
                        code => "a + ('b' + 'c') + (d + 'e')",
                        output => "`${a  }b` + `c${  d  }e`",
                        errors => errors,
                    },
                    {
                        code => "a + ('`b`' + '`c`') + d",
                        output => "`${a  }\\`b\\`` + `\\`c\\`${  d}`",
                        errors => errors,
                    },
                    {
                        code => "a + ('`b` + `c`' + '`d`') + e",
                        output => "`${a  }\\`b\\` + \\`c\\`` + `\\`d\\`${  e}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('b' + 'c' + 'd') + e",
                        output => "`a` + `b` + `c` + `d${  e}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('b' + 'c' + 'd' + (e + 'f') + 'g' +'h' + 'i') + j",
                        output => "`a` + `b` + `c` + `d${  e  }fg` +`h` + `i${  j}`",
                        errors => errors,
                    },
                    {
                        code => "a + (('b' + 'c') + 'd')",
                        output => "`${a  }b` + `c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "(a + 'b') + ('c' + 'd') + e",
                        output => "`${a  }b` + `c` + `d${  e}`",
                        errors => errors,
                    },
                    {
                        code => "var foo = \"Hello \" + \"world \" + \"another \" + test",
                        output => "var foo = `Hello ` + `world ` + `another ${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'Hello ' + '\"world\" ' + test",
                        output => "`Hello ` + `\"world\" ${  test}`",
                        errors => errors,
                    },
                    {
                        code => "\"Hello \" + \"'world' \" + test",
                        output => "`Hello ` + `'world' ${  test}`",
                        errors => errors,
                    }
                ]
            },
        )
    }
}
