use std::{borrow::Cow, sync::Arc};

use once_cell::sync::Lazy;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    kind::{BinaryExpression, TernaryExpression},
    utils::ast_utils,
};

static ARITHMETIC_OPERATORS: Lazy<Vec<&'static str>> =
    Lazy::new(|| vec!["+", "-", "*", "/", "%", "**"]);
static BITWISE_OPERATORS: Lazy<Vec<&'static str>> =
    Lazy::new(|| vec!["&", "|", "^", "~", "<<", ">>", ">>>"]);
static COMPARISON_OPERATORS: Lazy<Vec<&'static str>> =
    Lazy::new(|| vec!["==", "!=", "===", "!==", ">", ">=", "<", "<="]);
static LOGICAL_OPERATORS: Lazy<Vec<&'static str>> = Lazy::new(|| vec!["&&", "||"]);
static RELATIONAL_OPERATORS: Lazy<Vec<&'static str>> = Lazy::new(|| vec!["in", "instanceof"]);
static TERNARY_OPERATOR: Lazy<Vec<&'static str>> = Lazy::new(|| vec!["?:"]);
static COALESCE_OPERATOR: Lazy<Vec<&'static str>> = Lazy::new(|| vec!["??"]);
#[allow(dead_code)]
static ALL_OPERATORS: Lazy<Vec<&'static str>> = Lazy::new(|| {
    ARITHMETIC_OPERATORS
        .iter()
        .chain(&*BITWISE_OPERATORS)
        .chain(&*COMPARISON_OPERATORS)
        .chain(&*LOGICAL_OPERATORS)
        .chain(&*RELATIONAL_OPERATORS)
        .chain(&*TERNARY_OPERATOR)
        .chain(&*COALESCE_OPERATOR)
        .copied()
        .collect()
});
static DEFAULT_GROUPS: Lazy<Vec<Vec<Cow<'static, str>>>> = Lazy::new(|| {
    vec![
        ARITHMETIC_OPERATORS
            .iter()
            .copied()
            .map(Into::into)
            .collect(),
        BITWISE_OPERATORS.iter().copied().map(Into::into).collect(),
        COMPARISON_OPERATORS
            .iter()
            .copied()
            .map(Into::into)
            .collect(),
        LOGICAL_OPERATORS.iter().copied().map(Into::into).collect(),
        RELATIONAL_OPERATORS
            .iter()
            .copied()
            .map(Into::into)
            .collect(),
    ]
});

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    allow_same_precedence: bool,
    groups: Vec<Vec<Cow<'static, str>>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            allow_same_precedence: true,
            groups: DEFAULT_GROUPS.clone(),
        }
    }
}

fn includes_both_in_a_group(groups: &[Vec<Cow<'static, str>>], left: &str, right: &str) -> bool {
    groups.into_iter().any(|group| {
        group.iter().any(|operator| operator == left)
            && group.iter().any(|operator| operator == right)
    })
}

fn get_child_node(node: Node) -> Node {
    if node.kind() == TernaryExpression {
        node.field("condition")
    } else {
        node.field("left")
    }
}

fn should_ignore(
    node: Node,
    groups: &[Vec<Cow<'static, str>>],
    allow_same_precedence: bool,
) -> bool {
    let a = node;
    let b = node.parent().unwrap();

    !includes_both_in_a_group(
        groups,
        a.field("operator").kind(),
        if b.kind() == TernaryExpression {
            "?:"
        } else {
            b.field("operator").kind()
        },
    ) || allow_same_precedence && ast_utils::get_precedence(a) == ast_utils::get_precedence(b)
}

fn is_mixed_with_parent(node: Node) -> bool {
    let parent = node.parent().unwrap();
    parent.kind() != BinaryExpression
        || node.field("operator").kind() != parent.field("operator").kind()
}

fn get_operator_token<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> Node<'a> {
    context.get_token_after(
        get_child_node(node),
        Some(|node: Node| ast_utils::is_not_closing_paren_token(node, context)),
    )
}

fn report_both_operators<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
    let parent = node.parent().unwrap();
    let left = if get_child_node(parent) == node {
        node
    } else {
        parent
    };
    let right = if get_child_node(parent) != node {
        node
    } else {
        parent
    };
    let left_operator = if left.kind() == BinaryExpression {
        left.field("operator").kind()
    } else {
        "?:"
    };
    let right_operator = if right.kind() == BinaryExpression {
        right.field("operator").kind()
    } else {
        "?:"
    };

    context.report(violation! {
        node => left,
        range => get_operator_token(left, context).range(),
        message_id => "unexpected_mixed_operator",
        data => {
            left_operator => left_operator,
            right_operator => right_operator,
        }
    });
    context.report(violation! {
        node => right,
        range => get_operator_token(right, context).range(),
        message_id => "unexpected_mixed_operator",
        data => {
            left_operator => left_operator,
            right_operator => right_operator,
        }
    });
}

pub fn no_mixed_operators_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-mixed-operators",
        languages => [Javascript],
        messages => [
            unexpected_mixed_operator => "Unexpected mix of '{{left_operator}}' and '{{right_operator}}'. Use parentheses to clarify the intended order of operations.",
        ],
        options_type => Options,
        state => {
            [per-config]
            allow_same_precedence: bool = options.allow_same_precedence,
            groups: Vec<Vec<Cow<'static, str>>> = options.groups,
        },
        listeners => [
            r#"
              (binary_expression
                (binary_expression) @c
              )
              (ternary_expression
                (binary_expression) @c
              )
            "# => |node, context| {
                if is_mixed_with_parent(node) &&
                    !should_ignore(node, &self.groups, self.allow_same_precedence) {
                    report_both_operators(node, context);
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
    fn test_no_mixed_operators_rule() {
        RuleTester::run(
            no_mixed_operators_rule(),
            rule_tests! {
                valid => [
                    "a && b && c && d",
                    "a || b || c || d",
                    "(a || b) && c && d",
                    "a || (b && c && d)",
                    "(a || b || c) && d",
                    "a || b || (c && d)",
                    "a + b + c + d",
                    "a * b * c * d",
                    "a == 0 && b == 1",
                    "a == 0 || b == 1",
                    {
                        code => "(a == 0) && (b == 1)",
                        options => { groups => [["&&", "=="]] }
                    },
                    {
                        code => "a + b - c * d / e",
                        options => { groups => [["&&", "||"]] }
                    },
                    "a + b - c",
                    "a * b / c",
                    {
                        code => "a + b - c",
                        options => { allow_same_precedence => true }
                    },
                    {
                        code => "a * b / c",
                        options => { allow_same_precedence => true }
                    },
                    {
                        code => "(a || b) ? c : d",
                        options => { groups => [["&&", "||", "?:"]] }
                    },
                    {
                        code => "a ? (b || c) : d",
                        options => { groups => [["&&", "||", "?:"]] }
                    },
                    {
                        code => "a ? b : (c || d)",
                        options => { groups => [["&&", "||", "?:"]] }
                    },
                    {
                        code => "a || (b ? c : d)",
                        options => { groups => [["&&", "||", "?:"]] }
                    },
                    {
                        code => "(a ? b : c) || d",
                        options => { groups => [["&&", "||", "?:"]] }
                    },
                    "a || (b ? c : d)",
                    "(a || b) ? c : d",
                    "a || b ? c : d",
                    "a ? (b || c) : d",
                    "a ? b || c : d",
                    "a ? b : (c || d)",
                    "a ? b : c || d"
                ],
                invalid => [
                    {
                        code => "a && b || c",
                        errors => [
                            {
                                column => 3,
                                end_column => 5,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            },
                            {
                                column => 8,
                                end_column => 10,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            }
                        ]
                    },
                    {
                        code => "a && b > 0 || c",
                        options => { groups => [["&&", "||", ">"]] },
                        errors => [
                            {
                                column => 3,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            },
                            {
                                column => 3,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => ">"
                                }
                            },
                            {
                                column => 8,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => ">"
                                }
                            },
                            {
                                column => 12,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            }
                        ]
                    },
                    {
                        code => "a && b > 0 || c",
                        options => { groups => [["&&", "||"]] },
                        errors => [
                            {
                                column => 3,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            },
                            {
                                column => 12,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            }
                        ]
                    },
                    {
                        code => "a && b + c - d / e || f",
                        options => { groups => [["&&", "||"], ["+", "-", "*", "/"]] },
                        errors => [
                            {
                                column => 3,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            },
                            {
                                column => 12,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "-",
                                    right_operator => "/"
                                }
                            },
                            {
                                column => 16,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "-",
                                    right_operator => "/"
                                }
                            },
                            {
                                column => 20,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            }
                        ]
                    },
                    {
                        code => "a && b + c - d / e || f",
                        options => { groups => [["&&", "||"], ["+", "-", "*", "/"]], allow_same_precedence => true },
                        errors => [
                            {
                                column => 3,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            },
                            {
                                column => 12,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "-",
                                    right_operator => "/"
                                }
                            },
                            {
                                column => 16,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "-",
                                    right_operator => "/"
                                }
                            },
                            {
                                column => 20,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "||"
                                }
                            }
                        ]
                    },
                    {
                        code => "a + b - c",
                        options => { allow_same_precedence => false },
                        errors => [
                            {
                                column => 3,
                                end_column => 4,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "+",
                                    right_operator => "-"
                                }
                            },
                            {
                                column => 7,
                                end_column => 8,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "+",
                                    right_operator => "-"
                                }
                            }
                        ]
                    },
                    {
                        code => "a * b / c",
                        options => { allow_same_precedence => false },
                        errors => [
                            {
                                column => 3,
                                end_column => 4,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "*",
                                    right_operator => "/"
                                }
                            },
                            {
                                column => 7,
                                end_column => 8,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "*",
                                    right_operator => "/"
                                }
                            }
                        ]
                    },
                    {
                        code => "a || b ? c : d",
                        options => { groups => [["&&", "||", "?:"]] },
                        errors => [
                            {
                                column => 3,
                                end_column => 5,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "||",
                                    right_operator => "?:"
                                }
                            },
                            {
                                column => 8,
                                end_column => 9,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "||",
                                    right_operator => "?:"
                                }
                            }
                        ]
                    },
                    {
                        code => "a && b ? 1 : 2",
                        options => { groups => [["&&", "||", "?:"]] },
                        errors => [
                            {
                                column => 3,
                                end_column => 5,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "?:"
                                }
                            },
                            {
                                column => 8,
                                end_column => 9,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "&&",
                                    right_operator => "?:"
                                }
                            }
                        ]
                    },
                    {
                        code => "x ? a && b : 0",
                        options => { groups => [["&&", "||", "?:"]] },
                        errors => [
                            {
                                column => 3,
                                end_column => 4,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "?:",
                                    right_operator => "&&"
                                }
                            },
                            {
                                column => 7,
                                end_column => 9,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "?:",
                                    right_operator => "&&"
                                }
                            }
                        ]
                    },
                    {
                        code => "x ? 0 : a && b",
                        options => { groups => [["&&", "||", "?:"]] },
                        errors => [
                            {
                                column => 3,
                                end_column => 4,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "?:",
                                    right_operator => "&&"
                                }
                            },
                            {
                                column => 11,
                                end_column => 13,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "?:",
                                    right_operator => "&&"
                                }
                            }
                        ]
                    },
                    {
                        code => "a + b ?? c",
                        options => { groups => [["+", "??"]] },
                        // parserOptions => { ecmaVersion: 2020 },
                        errors => [
                            {
                                column => 3,
                                end_column => 4,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "+",
                                    right_operator => "??"
                                }
                            },
                            {
                                column => 7,
                                end_column => 9,
                                message_id => "unexpected_mixed_operator",
                                data => {
                                    left_operator => "+",
                                    right_operator => "??"
                                }
                            }
                        ]
                    }
                ]
            },
        )
    }
}
