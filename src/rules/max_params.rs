use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

use crate::{string_utils::upper_case_first, utils::ast_utils};

const DEFAULT_MAX: usize = 3;

#[derive(Deserialize)]
struct OptionsObject {
    #[serde(alias = "maximum")]
    max: Option<usize>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Usize(usize),
    Object(OptionsObject),
}

impl Options {
    pub fn max(&self) -> usize {
        match self {
            Self::Usize(value) => *value,
            Self::Object(OptionsObject { max }) => max.unwrap_or(DEFAULT_MAX),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Usize(DEFAULT_MAX)
    }
}

pub fn max_params_rule() -> Arc<dyn Rule> {
    rule! {
        name => "max-params",
        languages => [Javascript],
        messages => [
            exceed => "{{name}} has too many parameters ({{count}}). Maximum allowed is {{max}}.",
        ],
        options_type => Option<Options>,
        state => {
            [per-run]
            num_params: usize = options.unwrap_or_default().max(),
        },
        listeners => [
            r#"
              (function) @c
              (function_declaration) @c
              (arrow_function) @c
              (generator_function) @c
              (generator_function_declaration) @c
              (method_definition) @c
            "# => |node, context| {
                let num_params = node.child_by_field_name("parameters").map(|parameters| {
                    let mut cursor = parameters.walk();
                    parameters.named_children(&mut cursor).count()
                });
                if let Some(num_params) = num_params.filter(|&num_params| num_params > self.num_params) {
                    context.report(violation! {
                        range => ast_utils::get_function_head_range(node),
                        node => node,
                        message_id => "exceed",
                        data => {
                            name => upper_case_first(&ast_utils::get_function_name_with_kind(node, context)),
                            count => num_params,
                            max => self.num_params,
                        }
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
    fn test_max_params_rule() {
        RuleTester::run(
            max_params_rule(),
            rule_tests! {
                valid => [
                    "function test(d, e, f) {}",
                    { code => "var test = function(a, b, c) {};", options => 3 },
                    { code => "var test = (a, b, c) => {};", options => 3, /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var test = function test(a, b, c) {};", options => 3 },

                    // object property options
                    { code => "var test = function(a, b, c) {};", options => { max => 3 } }
                ],
                invalid => [
                    {
                        code => "function test(a, b, c) {}",
                        options => 2,
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function 'test'", count => 3, max => 2.0 },
                            type => "function_declaration"
                        }]
                    },
                    {
                        code => "function test(a, b, c, d) {}",
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function 'test'", count => 4, max => 3.0 },
                            type => "function_declaration"
                        }]
                    },
                    {
                        code => "var test = function(a, b, c, d) {};",
                        options => 3,
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function", count => 4, max => 3.0 },
                            type => "function"
                        }]
                    },
                    {
                        code => "var test = (a, b, c, d) => {};",
                        options => 3,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Arrow function", count => 4, max => 3.0 },
                            type => "arrow_function"
                        }]
                    },
                    {
                        code => "(function(a, b, c, d) {});",
                        options => 3,
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function", count => 4, max => 3.0 },
                            type => "function"
                        }]
                    },
                    {
                        code => "var test = function test(a, b, c) {};",
                        options => 1,
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function 'test'", count => 3, max => 1.0 },
                            type => "function"
                        }]
                    },

                    // object property options
                    {
                        code => "function test(a, b, c) {}",
                        options => { max => 2 },
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function 'test'", count => 3, max => 2.0 },
                            type => "function_declaration"
                        }]
                    },
                    {
                        code => "function test(a, b, c, d) {}",
                        options => {},
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function 'test'", count => 4, max => 3 }
                        }]
                    },
                    {
                        code => "function test(a) {}",
                        options => { max => 0 },
                        errors => [{
                            message_id => "exceed",
                            data => { name => "Function 'test'", count => 1, max => 0 }
                        }]
                    },

                    // Error location should not cover the entire function; just the name.
                    {
                        code => r#"function test(a, b, c) {
                          // Just to make it longer
                        }"#,
                        options => { max => 2 },
                        errors => [{
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 14
                        }]
                    }
                ]
            },
        )
    }
}
