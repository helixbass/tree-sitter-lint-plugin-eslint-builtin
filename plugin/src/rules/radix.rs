use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    #[default]
    Always,
    AsNeeded,
}

pub fn radix_rule() -> Arc<dyn Rule> {
    rule! {
        name => "radix",
        languages => [Javascript],
        messages => [
            missing_parameters => "Missing parameters.",
            redundant_radix => "Redundant radix parameter.",
            missing_radix => "Missing radix parameter.",
            invalid_radix => "Invalid radix parameter, must be an integer between 2 and 36.",
            add_radix_parameter_10 => "Add radix parameter `10` for parsing decimal numbers.",
        ],
        options_type => Mode,
        state => {
            [per-config]
            mode: Mode = options,
        },
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

    use super::*;
    use crate::kind::CallExpression;

    #[test]
    fn test_radix_rule() {
        RuleTester::run(
            radix_rule(),
            rule_tests! {
                valid => [
                    "parseInt(\"10\", 10);",
                    "parseInt(\"10\", 2);",
                    "parseInt(\"10\", 36);",
                    "parseInt(\"10\", 0x10);",
                    "parseInt(\"10\", 1.6e1);",
                    "parseInt(\"10\", 10.0);",
                    "parseInt(\"10\", foo);",
                    "Number.parseInt(\"10\", foo);",
                    {
                        code => "parseInt(\"10\", 10);",
                        options => "always"
                    },
                    {
                        code => "parseInt(\"10\");",
                        options => "as-needed"
                    },
                    {
                        code => "parseInt(\"10\", 8);",
                        options => "as-needed"
                    },
                    {
                        code => "parseInt(\"10\", foo);",
                        options => "as-needed"
                    },
                    "parseInt",
                    "Number.foo();",
                    "Number[parseInt]();",
                    { code => "class C { #parseInt; foo() { Number.#parseInt(); } }", environment => { ecma_version => 2022 } },
                    { code => "class C { #parseInt; foo() { Number.#parseInt(foo); } }", environment => { ecma_version => 2022 } },
                    { code => "class C { #parseInt; foo() { Number.#parseInt(foo, 'bar'); } }", environment => { ecma_version => 2022 } },
                    { code => "class C { #parseInt; foo() { Number.#parseInt(foo, 10); } }", options => "as-needed", environment => { ecma_version => 2022 } },

                    // Ignores if it's shadowed or disabled.
                    "var parseInt; parseInt();",
                    { code => "var parseInt; parseInt(foo);", options => "always" },
                    { code => "var parseInt; parseInt(foo, 10);", options => "as-needed" },
                    "var Number; Number.parseInt();",
                    { code => "var Number; Number.parseInt(foo);", options => "always" },
                    { code => "var Number; Number.parseInt(foo, 10);", options => "as-needed" },
                    { code => "/* globals parseInt:off */ parseInt(foo);", options => "always" },
                    { code => "Number.parseInt(foo, 10);", options => "as-needed", environment => { globals => { Number => "off" } } }
                ],

                invalid => [
                    {
                        code => "parseInt();",
                        options => "as-needed",
                        errors => [{
                            message_id => "missingParameters",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt();",
                        errors => [{
                            message_id => "missingParameters",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\");",
                        errors => [{
                            message_id => "missing_radix",
                            type => CallExpression,
                            // suggestions: [{ message_id => "addRadixParameter10", output => "parseInt(\"10\", 10);" }]
                        }]
                    },
                    {
                        code => "parseInt(\"10\",);", // Function parameter with trailing comma
                        environment => { ecma_version => 2017 },
                        errors => [{
                            message_id => "missing_radix",
                            type => CallExpression,
                            // suggestions: [{ message_id => "addRadixParameter10", output => "parseInt(\"10\", 10,);" }]
                        }]
                    },
                    {
                        code => "parseInt((0, \"10\"));", // Sequence expression (no trailing comma).
                        errors => [{
                            message_id => "missing_radix",
                            type => CallExpression,
                            // suggestions: [{ message_id => "addRadixParameter10", output => "parseInt((0, \"10\"), 10);" }]
                        }]
                    },
                    {
                        code => "parseInt((0, \"10\"),);", // Sequence expression (with trailing comma).
                        environment => { ecma_version => 2017 },
                        errors => [{
                            message_id => "missing_radix",
                            type => CallExpression,
                            // suggestions: [{ message_id => "addRadixParameter10", output => "parseInt((0, \"10\"), 10,);" }]
                        }]
                    },
                    {
                        code => "parseInt(\"10\", null);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", undefined);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", true);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", \"foo\");",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", \"123\");",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 1);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 37);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 10.5);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt();",
                        errors => [{
                            message_id => "missingParameters",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt();",
                        options => "as-needed",
                        errors => [{
                            message_id => "missingParameters",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt(\"10\");",
                        errors => [{
                            message_id => "missing_radix",
                            type => CallExpression,
                            // suggestions: [{ message_id => "addRadixParameter10", output => "Number.parseInt(\"10\", 10);" }]
                        }]
                    },
                    {
                        code => "Number.parseInt(\"10\", 1);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt(\"10\", 37);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt(\"10\", 10.5);",
                        errors => [{
                            message_id => "invalidRadix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 10);",
                        options => "as-needed",
                        errors => [{
                            message_id => "redundantRadix",
                            type => CallExpression
                        }]
                    },

                    // Optional chaining
                    {
                        code => "parseInt?.(\"10\");",
                        environment => { ecma_version => 2020 },
                        errors => [
                            {
                                message_id => "missing_radix",
                                type => CallExpression,
                                // suggestions: [{ message_id => "addRadixParameter10", output => "parseInt?.(\"10\", 10);" }]
                            }
                        ]
                    },
                    {
                        code => "Number.parseInt?.(\"10\");",
                        environment => { ecma_version => 2020 },
                        errors => [
                            {
                                message_id => "missing_radix",
                                type => CallExpression,
                                // suggestions: [{ message_id => "addRadixParameter10", output => "Number.parseInt?.(\"10\", 10);" }]
                            }
                        ]
                    },
                    {
                        code => "Number?.parseInt(\"10\");",
                        environment => { ecma_version => 2020 },
                        errors => [
                            {
                                message_id => "missing_radix",
                                type => CallExpression,
                                // suggestions: [{ message_id => "addRadixParameter10", output => "Number?.parseInt(\"10\", 10);" }]
                            }
                        ]
                    },
                    {
                        code => "(Number?.parseInt)(\"10\");",
                        environment => { ecma_version => 2020 },
                        errors => [
                            {
                                message_id => "missing_radix",
                                type => CallExpression,
                                // suggestions: [{ message_id => "addRadixParameter10", output => "(Number?.parseInt)(\"10\", 10);" }]
                            }
                        ]
                    }
                ]
            },
        )
    }
}
