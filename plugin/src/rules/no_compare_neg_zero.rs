use std::sync::Arc;

use tree_sitter_lint::{rule, violation, NodeExt, Rule};

pub fn no_compare_neg_zero_rule() -> Arc<dyn Rule> {
    const NEGATIVE_ZERO: &str = r#"
      (unary_expression
        operator: "-"
        argument: (number) @unary_argument (#eq? @unary_argument "0")
      )
    "#;

    const OPERATOR: &str = r#"
      [
        ">"
        ">="
        "<"
        "<="
        "=="
        "==="
        "!="
        "!=="
      ]
    "#;

    rule! {
        name => "no-compare-neg-zero",
        languages => [Javascript],
        messages => [
            unexpected => "Do not use the '{{operator}}' operator to compare against -0.",
        ],
        listeners => [
            format!(r#"[
              (binary_expression
                left: {NEGATIVE_ZERO}
                operator: {OPERATOR}
              )
              (binary_expression
                operator: {OPERATOR}
                right: {NEGATIVE_ZERO}
              )
            ] @binary_expression"#) => {
                capture_name => "binary_expression",
                callback => |node, context| {
                    context.report(violation! {
                        node => node,
                        message_id => "unexpected",
                        data => {
                            operator => node.field("operator").kind(),
                        }
                    });
                },
            }
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_compare_neg_zero_rule() {
        RuleTester::run(
            no_compare_neg_zero_rule(),
            rule_tests! {
                valid => [
                    "x === 0",
                    "0 === x",
                    "x == 0",
                    "0 == x",
                    "x === '0'",
                    "'0' === x",
                    "x == '0'",
                    "'0' == x",
                    "x === '-0'",
                    "'-0' === x",
                    "x == '-0'",
                    "'-0' == x",
                    "x === -1",
                    "-1 === x",
                    "x < 0",
                    "0 < x",
                    "x <= 0",
                    "0 <= x",
                    "x > 0",
                    "0 > x",
                    "x >= 0",
                    "0 >= x",
                    "x != 0",
                    "0 != x",
                    "x !== 0",
                    "0 !== x",
                    "Object.is(x, -0)"
                ],
                invalid => [
                    {
                        code => "x === -0",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "===" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "-0 === x",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "===" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "x == -0",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "==" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "-0 == x",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "==" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "x > -0",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => ">" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "-0 > x",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => ">" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "x >= -0",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => ">=" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "-0 >= x",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => ">=" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "x < -0",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "<" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "-0 < x",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "<" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "x <= -0",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "<=" },
                            type => "binary_expression"
                        }]
                    },
                    {
                        code => "-0 <= x",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "<=" },
                            type => "binary_expression"
                        }]
                    }
                ]
            },
        )
    }
}
