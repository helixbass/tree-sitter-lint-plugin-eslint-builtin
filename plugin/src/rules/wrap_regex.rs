use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule, NodeExt};

pub fn wrap_regex_rule() -> Arc<dyn Rule> {
    rule! {
        name => "wrap-regex",
        languages => [Javascript],
        messages => [
            require_parens => "Wrap the regexp literal in parens to disambiguate the slash.",
        ],
        fixable => true,
        listeners => [
            r#"
              (member_expression
                object: (regex) @c
              )
              (subscript_expression
                object: (regex) @c
              )
            "# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "require_parens",
                    fix => |fixer| {
                        fixer.replace_text(
                            node,
                            format!("({})", node.text(context))
                        );
                    }
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind;

    #[test]
    fn test_wrap_regex_rule() {
        RuleTester::run(
            wrap_regex_rule(),
            rule_tests! {
                valid => [
                    "(/foo/).test(bar);",
                    "(/foo/ig).test(bar);",
                    "/foo/;",
                    "var f = 0;",
                    "a[/b/];"
                ],
                invalid => [
                    {
                        code => "/foo/.test(bar);",
                        output => "(/foo/).test(bar);",
                        errors => [{ message_id => "require_parens", type => kind::Regex }]
                    },
                    {
                        code => "/foo/ig.test(bar);",
                        output => "(/foo/ig).test(bar);",
                        errors => [{ message_id => "require_parens", type => kind::Regex }]
                    },

                    // https://github.com/eslint/eslint/issues/10573
                    {
                        code => "if(/foo/ig.test(bar));",
                        output => "if((/foo/ig).test(bar));",
                        errors => [{ message_id => "require_parens", type => kind::Regex }]
                    }
                ]
            },
        )
    }
}
