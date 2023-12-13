use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_control_regex_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-control-regex",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected control character(s) in regular expression: {{control_chars}}.",
        ],
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use crate::kind;
    use super::*;

    #[test]
    fn test_no_control_regex_rule() {
        RuleTester::run(
            no_control_regex_rule(),
            rule_tests! {
                valid => [
                    "var regex = /x1f/",
                    r#"var regex = /\\x1f/"#,
                    "var regex = new RegExp('x1f')",
                    "var regex = RegExp('x1f')",
                    "new RegExp('[')",
                    "RegExp('[')",
                    "new (function foo(){})('\\x1f')",
                    { code => r#""/\u{20}/u"#, environment => { ecma_version => 2015 } },
                    r#"/\u{1F}/"#,
                    r#"/\u{1F}/g"#,
                    r#"new RegExp("\\u{20}", "u")"#,
                    r#"new RegExp("\\u{1F}")"#,
                    r#"new RegExp("\\u{1F}", "g")"#,
                    r#"new RegExp("\\u{1F}", flags)"#, // when flags are unknown, this rule assumes there's no `u` flag
                    r#"new RegExp("[\\q{\\u{20}}]", "v")"#,
                    { code => r#""/[\u{20}--B]/v"#, environment => { ecma_version => 2024 } }
                ],
                invalid => [
                    { code => r#"var regex = /\x1f/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => r#"var regex = /\\\x1f\\x1e/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => r#"var regex = /\\\x1fFOO\\x00/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => r#"var regex = /FOO\\\x1fFOO\\x1f/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => "var regex = new RegExp('\\x1f\\x1e')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f, \\x1e" }, type => "Literal" }] },
                    { code => "var regex = new RegExp('\\x1fFOO\\x00')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f, \\x00" }, type => "Literal" }] },
                    { code => "var regex = new RegExp('FOO\\x1fFOO\\x1f')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f, \\x1f" }, type => "Literal" }] },
                    { code => "var regex = RegExp('\\x1f')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => "Literal" }] },
                    {
                        code => "var regex = /(?<a>\\x1f)/",
                        environment => { ecma_version => 2018 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"var regex = /(?<\u{1d49c}>.)\x1f/"#,
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"new RegExp("\\u001F", flags)"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => "Literal" }]
                    },
                    {
                        code => r#"/\u{1111}*\x1F/u"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"new RegExp("\\u{1111}*\\x1F", "u")"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => "Literal" }]
                    },
                    {
                        code => r#"/\u{1F}/u"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"/\u{1F}/gui"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"new RegExp("\\u{1F}", "u")"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => "Literal" }]
                    },
                    {
                        code => r#"new RegExp("\\u{1F}", "gui")"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => "Literal" }]
                    },
                    {
                        code => r#"new RegExp("[\\q{\\u{1F}}]", "v")"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => "Literal" }]
                    },
                    {
                        code => r#"/[\u{1F}--B]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"/\x11/; RegExp("foo", "uv");"#,
                        environment => { ecma_version => 2024 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x11" }, type => kind::Regex, column => 1 }]
                    }
                ]
            },
        )
    }
}
