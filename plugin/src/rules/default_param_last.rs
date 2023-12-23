use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn default_param_last_rule() -> Arc<dyn Rule> {
    rule! {
        name => "default-param-last",
        languages => [Javascript],
        messages => [
            should_be_last => "Default parameters should be last.",
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
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::AssignmentPattern;

    #[test]
    fn test_default_param_last_rule() {
        let canned_error = RuleTestExpectedErrorBuilder::default()
            .message_id("should_be_last")
            .type_(AssignmentPattern)
            .build()
            .unwrap();

        RuleTester::run(
            default_param_last_rule(),
            rule_tests! {
                valid => [
                    "function f() {}",
                    "function f(a) {}",
                    "function f(a = 5) {}",
                    "function f(a, b) {}",
                    "function f(a, b = 5) {}",
                    "function f(a, b = 5, c = 5) {}",
                    "function f(a, b = 5, ...c) {}",
                    "const f = () => {}",
                    "const f = (a) => {}",
                    "const f = (a = 5) => {}",
                    "const f = function f() {}",
                    "const f = function f(a) {}",
                    "const f = function f(a = 5) {}"
                ],
                invalid => [
                    {
                        code => "function f(a = 5, b) {}",
                        errors => [
                            {
                                message_id => "should_be_last",
                                column => 12,
                                end_column => 17
                            }
                        ]
                    },
                    {
                        code => "function f(a = 5, b = 6, c) {}",
                        errors => [
                            {
                                message_id => "should_be_last",
                                column => 12,
                                end_column => 17
                            },
                            {
                                message_id => "should_be_last",
                                column => 19,
                                end_column => 24
                            }
                        ]
                    },
                    {
                        code => "function f (a = 5, b, c = 6, d) {}",
                        errors => [canned_error, canned_error]
                    },
                    {
                        code => "function f(a = 5, b, c = 5) {}",
                        errors => [
                            {
                                message_id => "should_be_last",
                                column => 12,
                                end_column => 17
                            }
                        ]
                    },
                    {
                        code => "const f = (a = 5, b, ...c) => {}",
                        errors => [canned_error]
                    },
                    {
                        code => "const f = function f (a, b = 5, c) {}",
                        errors => [canned_error]
                    },
                    {
                        code => "const f = (a = 5, { b }) => {}",
                        errors => [canned_error]
                    },
                    {
                        code => "const f = ({ a } = {}, b) => {}",
                        errors => [canned_error]
                    },
                    {
                        code => "const f = ({ a, b } = { a: 1, b: 2 }, c) => {}",
                        errors => [canned_error]
                    },
                    {
                        code => "const f = ([a] = [], b) => {}",
                        errors => [canned_error]
                    },
                    {
                        code => "const f = ([a, b] = [1, 2], c) => {}",
                        errors => [canned_error]
                    }
                ]
            },
        )
    }
}
