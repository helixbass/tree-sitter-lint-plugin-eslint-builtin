use std::{collections::HashSet, sync::Arc};

use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::Deserialize;
use squalid::EverythingExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{get_call_expression_arguments, get_number_literal_value, NodeExtJs, Number},
    kind,
    kind::{is_literal_kind, MemberExpression, PropertyIdentifier, Undefined},
    scope::{ScopeManager, Variable},
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Mode {
    #[default]
    Always,
    AsNeeded,
}

static VALID_RADIX_VALUES: Lazy<HashSet<Number>> =
    Lazy::new(|| (2..=36).step_by(2).map(Number::Integer).collect());

fn is_shadowed(variable: &Variable) -> bool {
    variable.defs().next().is_some()
}

fn is_parse_int_method(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() == MemberExpression
        && node.field("property").thrush(|property| {
            property.kind() == PropertyIdentifier && property.text(context) == "parseInt"
        })
}

fn is_valid_radix(radix: Node, context: &QueryMatchContext) -> bool {
    match radix.kind() {
        kind::Number => !VALID_RADIX_VALUES.contains(&get_number_literal_value(radix, context)),
        kind if is_literal_kind(kind) => false,
        Undefined => false,
        _ => true,
    }
}

fn is_default_radix(radix: Node, context: &QueryMatchContext) -> bool {
    if radix.kind() != kind::Number {
        return false;
    }
    get_number_literal_value(radix, context) == Number::Integer(10)
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
        methods => {
            fn check_arguments(&self, node: Node, context: &QueryMatchContext) {
                let Some(args) = get_call_expression_arguments(node) else {
                    return;
                };
                let args = args.collect_vec();
                match args.len() {
                    0 => {
                        context.report(violation! {
                            node => node,
                            message_id => "missing_parameters",
                        });
                    }
                    1 => {
                        if self.mode == Mode::Always {
                            context.report(violation! {
                                node => node,
                                message_id => "missing_radix",
                                // TODO: suggestions?
                            });
                        }
                    }
                    _ => {
                        if self.mode == Mode::AsNeeded &&
                            is_default_radix(args[1], context) {
                            context.report(violation! {
                                node => node,
                                message_id => "redundant_radix",
                            });
                        } else if !is_valid_radix(args[1], context) {
                            context.report(violation! {
                                node => node,
                                message_id => "invalid_radix",
                            });
                        }
                    }
                }
            }
        },
        listeners => [
            r#"
              program:exit
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let scope = scope_manager.get_scope(node);

                let variable = ast_utils::get_variable_by_name(scope.clone(), "parseInt");
                if let Some(variable) = variable.as_ref().filter(|variable| {
                    !is_shadowed(variable)
                }) {
                    variable.references().for_each(|reference| {
                        let id_node = reference.identifier();

                        if ast_utils::is_callee(id_node) {
                            self.check_arguments(id_node.next_non_parentheses_ancestor(), context);
                        }
                    });
                }

                let variable = ast_utils::get_variable_by_name(scope, "Number");
                if let Some(variable) = variable.as_ref().filter(|variable| {
                    !is_shadowed(variable)
                }) {
                    variable.references().for_each(|reference| {
                        let parent_node = reference.identifier().parent().unwrap();
                        let maybe_callee = parent_node;

                        if is_parse_int_method(parent_node, context) && ast_utils::is_callee(maybe_callee) {
                            self.check_arguments(maybe_callee.next_non_parentheses_ancestor(), context);
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{get_instance_provider_factory, kind::CallExpression};

    #[test]
    fn test_radix_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
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
                            message_id => "missing_parameters",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt();",
                        errors => [{
                            message_id => "missing_parameters",
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
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", undefined);",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", true);",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", \"foo\");",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", \"123\");",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 1);",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 37);",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 10.5);",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt();",
                        errors => [{
                            message_id => "missing_parameters",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt();",
                        options => "as-needed",
                        errors => [{
                            message_id => "missing_parameters",
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
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt(\"10\", 37);",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Number.parseInt(\"10\", 10.5);",
                        errors => [{
                            message_id => "invalid_radix",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "parseInt(\"10\", 10);",
                        options => "as-needed",
                        errors => [{
                            message_id => "redundant_radix",
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
            get_instance_provider_factory(),
        )
    }
}
