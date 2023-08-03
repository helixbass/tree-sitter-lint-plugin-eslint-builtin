use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Default, Deserialize)]
struct Options {
    ignore_non_declaration: Option<bool>,
}

impl Options {
    pub fn ignore_non_declaration(&self) -> bool {
        self.ignore_non_declaration.unwrap_or_default()
    }
}

pub fn no_multi_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-multi-assign",
        languages => [Javascript],
        messages => [
            unexpected_chain => "Unexpected chained assignment.",
        ],
        options_type => Option<Options>,
        state => {
            [per-run]
            ignore_non_declaration: bool = options.unwrap_or_default().ignore_non_declaration(),
        },
        listeners => [
            r#"
              (variable_declarator
                value: (assignment_expression) @c
              )
              (field_definition
                value: (assignment_expression) @c
              )
            "# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected_chain",
                });
            },
            r#"
              (assignment_expression
                right: (assignment_expression) @c
              )
            "# => |node, context| {
                if self.ignore_non_declaration {
                    return;
                }
                context.report(violation! {
                    node => node,
                    message_id => "unexpected_chain",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::AssignmentExpression;

    use super::*;

    use tree_sitter_lint::{
        rule_tests, RuleTestExpectedError, RuleTestExpectedErrorBuilder, RuleTester,
    };

    fn error_at(line: usize, column: usize, type_: &str) -> RuleTestExpectedError {
        RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected_chain")
            .type_(type_)
            .line(line)
            .column(column)
            .build()
            .unwrap()
    }

    #[test]
    fn test_no_multi_assign_rule() {
        RuleTester::run(
            no_multi_assign_rule(),
            rule_tests! {
                valid => [
                    "var a, b, c,\nd = 0;",
                    "var a = 1; var b = 2; var c = 3;\nvar d = 0;",
                    "var a = 1 + (b === 10 ? 5 : 4);",
                    { code => "const a = 1, b = 2, c = 3;", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "const a = 1;\nconst b = 2;\n const c = 3;", /*parserOptions: { ecmaVersion: 6 }*/ },
                    "for(var a = 0, b = 0;;){}",
                    { code => "for(let a = 0, b = 0;;){}", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "for(const a = 0, b = 0;;){}", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "export let a, b;", /*parserOptions: { ecmaVersion: 6, sourceType: "module" }*/ },
                    { code => "export let a,\n b = 0;", /*parserOptions: { ecmaVersion: 6, sourceType: "module" }*/ },
                    { code => "const x = {};const y = {};x.one = y.one = 1;", options => { ignore_non_declaration => true }, /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "let a, b;a = b = 1", options => { ignore_non_declaration => true }, /*parserOptions: { ecmaVersion: 6 }*/ },
                    {
                        code => "class C { [foo = 0] = 0 }",
                        // parserOptions: { ecmaVersion: 2022 }
                    }
                ],

                invalid => [
                    {
                        code => "var a = b = c;",
                        errors => [
                            error_at(1, 9, AssignmentExpression)
                        ]
                    },
                    {
                        code => "var a = b = c = d;",
                        errors => [
                            error_at(1, 9, AssignmentExpression),
                            error_at(1, 13, AssignmentExpression)
                        ]
                    },
                    {
                        code => "let foo = bar = cee = 100;",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            error_at(1, 11, AssignmentExpression),
                            error_at(1, 17, AssignmentExpression)
                        ]
                    },
                    {
                        code => "a=b=c=d=e",
                        errors => [
                            error_at(1, 3, AssignmentExpression),
                            error_at(1, 5, AssignmentExpression),
                            error_at(1, 7, AssignmentExpression)
                        ]
                    },
                    {
                        code => "a=b=c",
                        errors => [
                            error_at(1, 3, AssignmentExpression)
                        ]
                    },

                    {
                        code => "a\n=b\n=c",
                        errors => [
                            error_at(2, 2, AssignmentExpression)
                        ]
                    },

                    {
                        code => "var a = (b) = (((c)))",
                        errors => [
                            error_at(1, 9, AssignmentExpression)
                        ]
                    },

                    {
                        code => "var a = ((b)) = (c)",
                        errors => [
                            error_at(1, 9, AssignmentExpression)
                        ]
                    },

                    {
                        code => "var a = b = ( (c * 12) + 2)",
                        errors => [
                            error_at(1, 9, AssignmentExpression)
                        ]
                    },

                    {
                        code => "var a =\n((b))\n = (c)",
                        errors => [
                            error_at(2, 1, AssignmentExpression)
                        ]
                    },

                    {
                        code => "a = b = '=' + c + 'foo';",
                        errors => [
                            error_at(1, 5, AssignmentExpression)
                        ]
                    },
                    {
                        code => "a = b = 7 * 12 + 5;",
                        errors => [
                            error_at(1, 5, AssignmentExpression)
                        ]
                    },
                    {
                        code => "const x = {};\nconst y = x.one = 1;",
                        options => { ignore_non_declaration => true },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            error_at(2, 11, AssignmentExpression)
                        ]

                    },
                    {
                        code => "let a, b;a = b = 1",
                        options => {},
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            error_at(1, 14, AssignmentExpression)
                        ]
                    },
                    {
                        code => "let x, y;x = y = 'baz'",
                        options => { ignore_non_declaration => false },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            error_at(1, 14, AssignmentExpression)
                        ]
                    },
                    {
                        code => "const a = b = 1",
                        options => { ignore_non_declaration => true },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            error_at(1, 11, AssignmentExpression)
                        ]
                    },
                    {
                        code => "class C { field = foo = 0 }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            error_at(1, 19, AssignmentExpression)
                        ]
                    },
                    {
                        code => "class C { field = foo = 0 }",
                        options => { ignore_non_declaration => true },
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            error_at(1, 19, AssignmentExpression)
                        ]
                    }
                ]
            },
        )
    }
}
