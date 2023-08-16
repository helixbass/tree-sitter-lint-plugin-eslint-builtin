use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use once_cell::sync::Lazy;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::skip_parenthesized_expressions,
    kind::{BinaryExpression, False, Identifier, True, UnaryExpression},
    utils::ast_utils,
};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    default_assignment: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            default_assignment: true,
        }
    }
}

static BOOLEAN_OPERATORS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "==",
        "===",
        "!=",
        "!==",
        ">",
        ">=",
        "<",
        "<=",
        "in",
        "instanceof",
    ]
    .into()
});

static OPERATOR_INVERSES: Lazy<HashMap<&'static str, &'static str>> =
    Lazy::new(|| [("==", "!="), ("!=", "=="), ("===", "!=="), ("!==", "===")].into());

static OR_PRECEDENCE: Lazy<u32> =
    Lazy::new(|| ast_utils::get_binary_expression_operator_precedence("||"));

fn is_boolean_literal(node: Node) -> bool {
    matches!(node.kind(), True | False)
}

fn invert_expression(node: Node, context: &QueryMatchContext) -> String {
    if node.kind() == BinaryExpression {
        let operator_node = node.child_by_field_name("operator").unwrap();
        let operator = context.get_node_text(operator_node);
        if let Some(&operator_inverse) = OPERATOR_INVERSES.get(&*operator) {
            return format!(
                "{}{}{}",
                context.get_text_slice(node.start_byte()..operator_node.start_byte()),
                operator_inverse,
                context.get_text_slice(operator_node.end_byte()..node.end_byte()),
            );
        }
    }

    if ast_utils::get_precedence(node) < ast_utils::get_kind_precedence(UnaryExpression) {
        format!("!({})", context.get_node_text(node))
    } else {
        format!("!{}", ast_utils::get_parenthesised_text(context, node))
    }
}

fn is_boolean_expression(node: Node) -> bool {
    node.kind() == BinaryExpression && BOOLEAN_OPERATORS.contains(node.field("operator").kind())
        || node.kind() == UnaryExpression && node.field("operator").kind() == "!"
}

fn matches_default_assignment(node: Node, context: &QueryMatchContext) -> bool {
    let test = skip_parenthesized_expressions(node.child_by_field_name("condition").unwrap());
    let consequent =
        skip_parenthesized_expressions(node.child_by_field_name("consequence").unwrap());
    test.kind() == Identifier
        && consequent.kind() == Identifier
        && context.get_node_text(test) == context.get_node_text(consequent)
}

pub fn no_unneeded_ternary_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-unneeded-ternary",
        languages => [Javascript],
        messages => [
            unnecessary_conditional_expression =>
                "Unnecessary use of boolean literals in conditional expression.",
            unnecessary_conditional_assignment =>
                "Unnecessary use of conditional expression for default assignment.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-run]
            default_assignment: bool = options.default_assignment,
        },
        listeners => [
            r#"(
              (ternary_expression) @c
            )"# => |node, context| {
                let alternate = skip_parenthesized_expressions(
                    node.child_by_field_name("alternative").unwrap(),
                );
                let consequent = skip_parenthesized_expressions(
                    node.child_by_field_name("consequence").unwrap(),
                );
                if is_boolean_literal(alternate) && is_boolean_literal(consequent) {
                    context.report(violation! {
                        node => node,
                        message_id => "unnecessary_conditional_expression",
                        fix => |fixer| {
                            let test = node.child_by_field_name("condition").unwrap();
                            match (consequent.kind(), alternate.kind()) {
                                (True, True) | (False, False) => {
                                    if test.kind() == Identifier {
                                        fixer.replace_text(
                                            node,
                                            if consequent.kind() == True {
                                                "true"
                                            } else {
                                                "false"
                                            }
                                        );
                                    }
                                }
                                (False, True) => {
                                    fixer.replace_text(
                                        node,
                                        invert_expression(test, context),
                                    );
                                }
                                (True, False) => {
                                    fixer.replace_text(
                                        node,
                                        if is_boolean_expression(test) {
                                            ast_utils::get_parenthesised_text(context, test).into_owned()
                                        } else {
                                            format!("!{}", invert_expression(test, context))
                                        },
                                    );
                                }
                                _ => unreachable!()
                            }
                        }
                    });
                } else if !self.default_assignment && matches_default_assignment(node, context) {
                    context.report(violation! {
                        node => node,
                        message_id => "unnecessary_conditional_assignment",
                        fix => |fixer| {
                            let should_parenthesize_alternate = (
                                ast_utils::get_precedence(alternate) < *OR_PRECEDENCE ||
                                ast_utils::is_coalesce_expression(alternate)
                            ) && !ast_utils::is_parenthesised(alternate);
                            let alternate_text = if should_parenthesize_alternate {
                                format!("({})", context.get_node_text(alternate)).into()
                            } else {
                                ast_utils::get_parenthesised_text(context, alternate)
                            };
                            let test_text = ast_utils::get_parenthesised_text(context, node.child_by_field_name("condition").unwrap());

                            fixer.replace_text(
                                node,
                                format!("{test_text} || {alternate_text}")
                            );
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_unneeded_ternary_rule() {
        RuleTester::run(
            no_unneeded_ternary_rule(),
            rule_tests! {
                valid => [
                    "config.newIsCap = config.newIsCap !== false",
                    "var a = x === 2 ? 'Yes' : 'No';",
                    "var a = x === 2 ? true : 'No';",
                    "var a = x === 2 ? 'Yes' : false;",
                    "var a = x === 2 ? 'true' : 'false';",
                    "var a = foo ? foo : bar;",
                    "var value = 'a';var canSet = true;var result = value || (canSet ? 'unset' : 'can not set')",
                    "var a = foo ? bar : foo;",
                    "foo ? bar : foo;",
                    "var a = f(x ? x : 1)",
                    "f(x ? x : 1);",
                    "foo ? foo : bar;",
                    "var a = foo ? 'Yes' : foo;",
                    {
                        code => "var a = foo ? 'Yes' : foo;",
                        options => { default_assignment => false }
                    },
                    {
                        code => "var a = foo ? bar : foo;",
                        options => { default_assignment => false }
                    },
                    {
                        code => "foo ? bar : foo;",
                        options => { default_assignment => false }
                    }
                ],
                invalid => [
                    {
                        code => "var a = x === 2 ? true : false;",
                        output => "var a = x === 2;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 31
                        }]
                    },
                    {
                        code => "var a = x >= 2 ? true : false;",
                        output => "var a = x >= 2;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 30
                        }]
                    },
                    {
                        code => "var a = x ? true : false;",
                        output => "var a = !!x;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 25
                        }]
                    },
                    {
                        code => "var a = x === 1 ? false : true;",
                        output => "var a = x !== 1;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 31
                        }]
                    },
                    {
                        code => "var a = x != 1 ? false : true;",
                        output => "var a = x == 1;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 30
                        }]
                    },
                    {
                        code => "var a = foo() ? false : true;",
                        output => "var a = !foo();",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 29
                        }]
                    },
                    {
                        code => "var a = !foo() ? false : true;",
                        output => "var a = !!foo();",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 30
                        }]
                    },
                    {
                        code => "var a = foo + bar ? false : true;",
                        output => "var a = !(foo + bar);",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 33
                        }]
                    },
                    {
                        code => "var a = x instanceof foo ? false : true;",
                        output => "var a = !(x instanceof foo);",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 40
                        }]
                    },
                    {
                        code => "var a = foo ? false : false;",
                        output => "var a = false;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 28
                        }]
                    },
                    {
                        code => "var a = foo() ? false : false;",
                        output => None,
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 30
                        }]
                    },
                    {
                        code => "var a = x instanceof foo ? true : false;",
                        output => "var a = x instanceof foo;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 40
                        }]
                    },
                    {
                        code => "var a = !foo ? true : false;",
                        output => "var a = !foo;",
                        errors => [{
                            message_id => "unnecessary_conditional_expression",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 28
                        }]
                    },
                    {
                        code => r#"
                var value = 'a'
                var canSet = true
                var result = value ? value : canSet ? 'unset' : 'can not set'
                        "#,
                        output => r#"
                var value = 'a'
                var canSet = true
                var result = value || (canSet ? 'unset' : 'can not set')
                        "#,
                        options => { default_assignment => false },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 4,
                            column => 30,
                            end_line => 4,
                            end_column => 78
                        }]
                    },
                    {
                        code => "foo ? foo : (bar ? baz : qux)",
                        output => "foo || (bar ? baz : qux)",
                        options => { default_assignment => false },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 30
                        }]
                    },
                    {
                        code => "function* fn() { foo ? foo : yield bar }",
                        output => "function* fn() { foo || (yield bar) }",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 18,
                            end_line => 1,
                            end_column => 39
                        }]
                    },
                    {
                        code => "var a = foo ? foo : 'No';",
                        output => "var a = foo || 'No';",
                        options => { default_assignment => false },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 25
                        }]
                    },
                    {
                        code => "var a = ((foo)) ? (((((foo))))) : ((((((((((((((bar))))))))))))));",
                        output => "var a = ((foo)) || ((((((((((((((bar))))))))))))));",
                        options => { default_assignment => false },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 66
                        }]
                    },
                    {
                        code => "var a = b ? b : c => c;",
                        output => "var a = b || (c => c);",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2015 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 23
                        }]
                    },
                    {
                        code => "var a = b ? b : c = 0;",
                        output => "var a = b || (c = 0);",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2015 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 22
                        }]
                    },
                    {
                        code => "var a = b ? b : (c => c);",
                        output => "var a = b || (c => c);",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2015 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 25
                        }]
                    },
                    {
                        code => "var a = b ? b : (c = 0);",
                        output => "var a = b || (c = 0);",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2015 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 24
                        }]
                    },
                    {
                        code => "var a = b ? b : (c) => (c);",
                        output => "var a = b || ((c) => (c));",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2015 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 27
                        }]
                    },
                    {
                        code => "var a = b ? b : c, d; // this is ((b ? b : c), (d))",
                        output => "var a = b || c, d; // this is ((b ? b : c), (d))",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2015 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 18
                        }]
                    },
                    {
                        code => "var a = b ? b : (c, d);",
                        output => "var a = b || (c, d);",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2015 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 23
                        }]
                    },
                    {
                        code => "f(x ? x : 1);",
                        output => "f(x || 1);",
                        options => { default_assignment => false },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 3,
                            end_line => 1,
                            end_column => 12
                        }]
                    },
                    {
                        code => "x ? x : 1;",
                        output => "x || 1;",
                        options => { default_assignment => false },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 10
                        }]
                    },
                    {
                        code => "var a = foo ? foo : bar;",
                        output => "var a = foo || bar;",
                        options => { default_assignment => false },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 24
                        }]
                    },
                    {
                        code => "var a = foo ? foo : a ?? b;",
                        output => "var a = foo || (a ?? b);",
                        options => { default_assignment => false },
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_conditional_assignment",
                            type => "ternary_expression",
                            line => 1,
                            column => 9,
                            end_line => 1,
                            end_column => 27
                        }]
                    },

                    // // https://github.com/eslint/eslint/issues/17173
                    // {
                    //     code => "foo as any ? false : true",
                    //     output => "!(foo as any)",
                    //     // parser: parser("typescript-parsers/unneeded-ternary-1"),
                    //     // parserOptions: { ecmaVersion: 6 },
                    //     errors => [{
                    //         message_id => "unnecessary_conditional_expression",
                    //         type => "ternary_expression"
                    //     }]
                    // },
                    // {
                    //     code => "foo ? foo : bar as any",
                    //     output => "foo || (bar as any)",
                    //     options => { default_assignment => false },
                    //     // parser: parser("typescript-parsers/unneeded-ternary-2"),
                    //     // parserOptions: { ecmaVersion: 6 },
                    //     errors => [{
                    //         message_id => "unnecessary_conditional_assignment",
                    //         type => "ternary_expression"
                    //     }]
                    // }
                ]
            },
        )
    }
}
