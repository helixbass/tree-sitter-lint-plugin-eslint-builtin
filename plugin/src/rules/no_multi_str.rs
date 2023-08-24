use std::sync::Arc;

use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{kind, utils::ast_utils::LINE_BREAK_PATTERN};

pub fn no_multi_str_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-multi-str",
        languages => [Javascript],
        messages => [
            multiline_string => "Multiline support is limited to browsers supporting ES5 only.",
        ],
        listeners => [
            // format!(r#"(
            //   (string) @c (#match? @c "{LINE_BREAK_PATTERN_STR}")
            // )"#) => |node, context| {
            kind::String => |node, context| {
                if LINE_BREAK_PATTERN.is_match(&node.text(context)) {
                    context.report(violation! {
                        node => node,
                        message_id => "multiline_string",
                    });
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_multi_str_rule() {
        RuleTester::run(
            no_multi_str_rule(),
            rule_tests! {
                valid => [
                    "var a = 'Line 1 Line 2';",
                    { code => "var a = <div>\n<h1>Wat</h1>\n</div>;", /*parserOptions: { ecmaVersion: 6, ecmaFeatures: { jsx: true } }*/ }
                ],
                invalid => [
                    {
                        code => "var x = 'Line 1 \\\n Line 2'",
                        errors => [{
                            message_id => "multiline_string",
                            type => kind::String
                        }]
                    },
                    {
                        code => "test('Line 1 \\\n Line 2');",
                        errors => [{
                            message_id => "multiline_string",
                            type => kind::String
                        }]
                    },
                    {
                        code => "'foo\\\rbar';",
                        errors => [{
                            message_id => "multiline_string",
                            type => kind::String
                        }]
                    },
                    {
                        code => "'foo\\\u{2028}bar';",
                        errors => [{
                            message_id => "multiline_string",
                            type => kind::String
                        }]
                    },
                    {
                        code => "'foo\\\u{2029}ar';",
                        errors => [{
                            message_id => "multiline_string",
                            type => kind::String
                        }]
                    }
                ]
            },
        )
    }
}
