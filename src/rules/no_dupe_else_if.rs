use std::sync::Arc;

use squalid::{BoolExt, VecExt};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{
        is_binary_expression_with_one_of_operators, is_binary_expression_with_operator,
        is_logical_and, skip_parenthesized_expressions,
    },
    kind::{ElseClause, IfStatement},
    text::SourceTextProvider,
    utils::ast_utils,
};

fn is_subset_by_comparator<TItem>(
    comparator: impl Fn(&TItem, &TItem) -> bool,
    arr_a: &[TItem],
    arr_b: &[TItem],
) -> bool {
    arr_a
        .into_iter()
        .all(|a| arr_b.into_iter().any(|b| comparator(a, b)))
}

fn split_by_logical_operator<'a, 'b>(
    operator: &str,
    node: Node<'b>,
    source_text_provider: &impl SourceTextProvider<'a>,
) -> Vec<Node<'b>> {
    let node = skip_parenthesized_expressions(node);
    if is_binary_expression_with_operator(node, operator, source_text_provider) {
        split_by_logical_operator(
            operator,
            node.child_by_field_name("left").unwrap(),
            source_text_provider,
        )
        .and_extend(split_by_logical_operator(
            operator,
            node.child_by_field_name("right").unwrap(),
            source_text_provider,
        ))
    } else {
        vec![node]
    }
}

fn equal(context: &QueryMatchContext, a: Node, b: Node) -> bool {
    if a.kind_id() != b.kind_id() {
        return false;
    }

    if is_binary_expression_with_one_of_operators(a, &["&&", "||"], context)
        && matches!(
            b.child_by_field_name("operator"),
            Some(b_operator) if context.get_node_text(a.child_by_field_name("operator").unwrap()) ==
                context.get_node_text(b_operator)
        )
    {
        return equal(
            context,
            a.child_by_field_name("left").unwrap(),
            b.child_by_field_name("left").unwrap(),
        ) && equal(
            context,
            a.child_by_field_name("right").unwrap(),
            b.child_by_field_name("right").unwrap(),
        ) || equal(
            context,
            a.child_by_field_name("left").unwrap(),
            b.child_by_field_name("right").unwrap(),
        ) && equal(
            context,
            a.child_by_field_name("right").unwrap(),
            b.child_by_field_name("left").unwrap(),
        );
    }

    ast_utils::equal_tokens(a, b, context)
}

pub fn no_dupe_else_if_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-dupe-else-if",
        languages => [Javascript],
        messages => [
            unexpected => "This branch can never execute. Its condition is a duplicate or covered by previous conditions in the if-else-if chain.",
        ],
        listeners => [
            r#"(
              (if_statement) @c
            )"# => |node, context| {
                let test = skip_parenthesized_expressions(node.child_by_field_name("condition").unwrap());
                let conditions_to_check = if is_logical_and(test, context) {
                    vec![test].and_extend(split_by_logical_operator("&&", test, context))
                } else {
                    vec![test]
                };

                let mut current = node;
                let mut list_to_check = conditions_to_check
                    .into_iter()
                    .map(|c| {
                        split_by_logical_operator("||", c, context)
                            .into_iter()
                            .map(|node| split_by_logical_operator("&&", node, context))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>();

                while let Some(parent) = current.parent().and_then(|else_clause| {
                    (else_clause.kind() == ElseClause).then_and(|| else_clause.parent())
                }).filter(|parent| parent.kind() == IfStatement) {
                    current = parent;

                    let current_or_operands = split_by_logical_operator(
                        "||",
                        current.child_by_field_name("condition").unwrap(),
                        context,
                    ).into_iter().map(|node| split_by_logical_operator("&&", node, context))
                        .collect::<Vec<_>>();

                    list_to_check = list_to_check.into_iter().map(|or_operands| {
                        or_operands.into_iter().filter(|or_operand| {
                            !current_or_operands.iter().any(|current_or_operand| {
                                is_subset_by_comparator(|&a, &b| equal(context, a, b), current_or_operand, or_operand)
                            })
                        }).collect::<Vec<_>>()
                    }).collect();

                    if list_to_check.iter().any(|or_operands| or_operands.is_empty()) {
                        context.report(violation! {
                            node => test,
                            message_id => "unexpected",
                        });
                        break;
                    }
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
    fn test_no_dupe_else_if_rule() {
        RuleTester::run(
            no_dupe_else_if_rule(),
            rule_tests! {
                valid => [
                    // different test conditions
                    "if (a) {} else if (b) {}",
                    "if (a); else if (b); else if (c);",
                    "if (true) {} else if (false) {} else {}",
                    "if (1) {} else if (2) {}",
                    "if (f) {} else if (f()) {}",
                    "if (f(a)) {} else if (g(a)) {}",
                    "if (f(a)) {} else if (f(b)) {}",
                    "if (a === 1) {} else if (a === 2) {}",
                    "if (a === 1) {} else if (b === 1) {}",

                    // not an if-else-if chain
                    "if (a) {}",
                    "if (a);",
                    "if (a) {} else {}",
                    "if (a) if (a) {}",
                    "if (a) if (a);",
                    "if (a) { if (a) {} }",
                    "if (a) {} else { if (a) {} }",
                    "if (a) {} if (a) {}",
                    "if (a); if (a);",
                    "while (a) if (a);",
                    "if (a); else a ? a : a;",

                    // not same conditions in the chain
                    "if (a) { if (b) {} } else if (b) {}",
                    "if (a) if (b); else if (a);",

                    // not equal tokens
                    "if (a) {} else if (!!a) {}",
                    "if (a === 1) {} else if (a === (1)) {}",

                    // more complex valid chains (may contain redundant subconditions, but the branch can be executed)
                    "if (a || b) {} else if (c || d) {}",
                    "if (a || b) {} else if (a || c) {}",
                    "if (a) {} else if (a || b) {}",
                    "if (a) {} else if (b) {} else if (a || b || c) {}",
                    "if (a && b) {} else if (a) {} else if (b) {}",
                    "if (a && b) {} else if (b && c) {} else if (a && c) {}",
                    "if (a && b) {} else if (b || c) {}",
                    "if (a) {} else if (b && (a || c)) {}",
                    "if (a) {} else if (b && (c || d && a)) {}",
                    "if (a && b && c) {} else if (a && b && (c || d)) {}"
                ],
                invalid => [
                    // basic tests
                    {
                        code => "if (a) {} else if (a) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 20 }]
                    },
                    {
                        code => "if (a); else if (a);",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 18 }]
                    },
                    {
                        code => "if (a) {} else if (a) {} else {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 20 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (a) {} else if (c) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (a) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (c) {} else if (a) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 50 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (b) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (b) {} else {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (c) {} else if (b) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 50 }]
                    },
                    {
                        code => "if (a); else if (b); else if (c); else if (b); else if (d); else;",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 44 }]
                    },
                    {
                        code => "if (a); else if (b); else if (c); else if (d); else if (b); else if (e);",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 57 }]
                    },

                    // multiple duplicates of the same condition
                    {
                        code => "if (a) {} else if (a) {} else if (a) {}",
                        errors => [
                            { message_id => "unexpected", type => "identifier", column => 20 },
                            { message_id => "unexpected", type => "identifier", column => 35 }
                        ]
                    },

                    // multiple duplicates of different conditions
                    {
                        code => "if (a) {} else if (b) {} else if (a) {} else if (b) {} else if (a) {}",
                        errors => [
                            { message_id => "unexpected", type => "identifier", column => 35 },
                            { message_id => "unexpected", type => "identifier", column => 50 },
                            { message_id => "unexpected", type => "identifier", column => 65 }
                        ]
                    },

                    // inner if statements do not affect chain
                    {
                        code => "if (a) { if (b) {} } else if (a) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 31 }]
                    },

                    // various kinds of test conditions
                    {
                        code => "if (a === 1) {} else if (a === 1) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 26 }]
                    },
                    {
                        code => "if (1 < a) {} else if (1 < a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 24 }]
                    },
                    {
                        code => "if (true) {} else if (true) {}",
                        errors => [{ message_id => "unexpected", type => "true", column => 23 }]
                    },
                    {
                        code => "if (a && b) {} else if (a && b) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a && b || c)  {} else if (a && b || c) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 31 }]
                    },
                    {
                        code => "if (f(a)) {} else if (f(a)) {}",
                        errors => [{ message_id => "unexpected", type => "call_expression", column => 23 }]
                    },

                    // spaces and comments do not affect comparison
                    {
                        code => "if (a === 1) {} else if (a===1) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 26 }]
                    },
                    {
                        code => "if (a === 1) {} else if (a === /* comment */ 1) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 26 }]
                    },

                    // extra parens around the whole test condition do not affect comparison
                    {
                        code => "if (a === 1) {} else if ((a === 1)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 27 }]
                    },

                    // more complex errors with `||` and `&&`
                    {
                        code => "if (a || b) {} else if (a) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 25 }]
                    },
                    {
                        code => "if (a || b) {} else if (a) {} else if (b) {}",
                        errors => [
                            { message_id => "unexpected", type => "identifier", column => 25 },
                            { message_id => "unexpected", type => "identifier", column => 40 }
                        ]
                    },
                    {
                        code => "if (a || b) {} else if (b || a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (a || b) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 35 }]
                    },
                    {
                        code => "if (a || b) {} else if (c || d) {} else if (a || d) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 45 }]
                    },
                    {
                        code => "if ((a === b && fn(c)) || d) {} else if (fn(c) && a === b) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 42 }]
                    },
                    {
                        code => "if (a) {} else if (a && b) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 20 }]
                    },
                    {
                        code => "if (a && b) {} else if (b && a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a && b) {} else if (a && b && c) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a || c) {} else if (a && b || c) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (c && a || b) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (c && (a || b)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (b && c) {} else if (d && (a || e && c && b)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 40 }]
                    },
                    {
                        code => "if (a || b && c) {} else if (b && c && d) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 30 }]
                    },
                    {
                        code => "if (a || b) {} else if (b && c) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if ((a || b) && c) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 35 }]
                    },
                    {
                        code => "if ((a && (b || c)) || d) {} else if ((c || b) && e && a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 39 }]
                    },
                    {
                        code => "if (a && b || b && c) {} else if (a && b && c) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (b && c) {} else if (d && (c && e && b || a)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 40 }]
                    },
                    {
                        code => "if (a || (b && (c || d))) {} else if ((d || c) && b) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 39 }]
                    },
                    {
                        code => "if (a || b) {} else if ((b || a) && c) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a || b) {} else if (c) {} else if (d) {} else if (b && (a || c)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 55 }]
                    },
                    {
                        code => "if (a || b || c) {} else if (a || (b && d) || (c && e)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 30 }]
                    },
                    {
                        code => "if (a || (b || c)) {} else if (a || (b && c)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 32 }]
                    },
                    {
                        code => "if (a || b) {} else if (c) {} else if (d) {} else if ((a || c) && (b || d)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 55 }]
                    },
                    {
                        code => "if (a) {} else if (b) {} else if (c && (a || d && b)) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 35 }]
                    },
                    {
                        code => "if (a) {} else if (a || a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 20 }]
                    },
                    {
                        code => "if (a || a) {} else if (a || a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a || a) {} else if (a) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 25 }]
                    },
                    {
                        code => "if (a) {} else if (a && a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 20 }]
                    },
                    {
                        code => "if (a && a) {} else if (a && a) {}",
                        errors => [{ message_id => "unexpected", type => "binary_expression", column => 25 }]
                    },
                    {
                        code => "if (a && a) {} else if (a) {}",
                        errors => [{ message_id => "unexpected", type => "identifier", column => 25 }]
                    }
                ]
            },
        )
    }
}
