use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, NodeExt, Rule};

use crate::{ast_helpers::NodeExtJs, kind::IfStatement};

const NEGATED_EXPRESSION: &str = r#"
  [
    (unary_expression
      operator: "!"
    )
    (binary_expression
      operator: [
        "!="
        "!=="
      ]
    )
  ]
"#;

pub fn no_negated_condition_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-negated-condition",
        languages => [Javascript],
        messages => [
            unexpected_negated => "Unexpected negated condition.",
        ],
        listeners => [
            format!(r#"
              (if_statement
                condition: (parenthesized_expression
                  {NEGATED_EXPRESSION}
                )
                alternative: (else_clause)
              ) @c
              (ternary_expression
                condition: {NEGATED_EXPRESSION}
              ) @c
            "#) => |node, context| {
                if node.kind() == IfStatement &&
                    node.field("alternative").first_non_comment_named_child().kind() == IfStatement {
                    return;
                }
                context.report(violation! {
                    node => node,
                    message_id => "unexpected_negated",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::{IfStatement, TernaryExpression};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_negated_condition_rule() {
        RuleTester::run(
            no_negated_condition_rule(),
            rule_tests! {
                // Examples of code that should not trigger the rule
                valid => [
                    "if (a) {}",
                    "if (a) {} else {}",
                    "if (!a) {}",
                    "if (!a) {} else if (b) {}",
                    "if (!a) {} else if (b) {} else {}",
                    "if (a == b) {}",
                    "if (a == b) {} else {}",
                    "if (a != b) {}",
                    "if (a != b) {} else if (b) {}",
                    "if (a != b) {} else if (b) {} else {}",
                    "if (a !== b) {}",
                    "if (a === b) {} else {}",
                    "a ? b : c"
                ],

                // Examples of code that should trigger the rule
                invalid => [
                    {
                        code => "if (!a) {;} else {;}",
                        errors => [{
                            message_id => "unexpected_negated",
                            type => IfStatement
                        }]
                    },
                    {
                        code => "if (a != b) {;} else {;}",
                        errors => [{
                            message_id => "unexpected_negated",
                            type => IfStatement
                        }]
                    },
                    {
                        code => "if (a !== b) {;} else {;}",
                        errors => [{
                            message_id => "unexpected_negated",
                            type => IfStatement
                        }]
                    },
                    {
                        code => "!a ? b : c",
                        errors => [{
                            message_id => "unexpected_negated",
                            type => TernaryExpression
                        }]
                    },
                    {
                        code => "a != b ? c : d",
                        errors => [{
                            message_id => "unexpected_negated",
                            type => TernaryExpression
                        }]
                    },
                    {
                        code => "a !== b ? c : d",
                        errors => [{
                            message_id => "unexpected_negated",
                            type => TernaryExpression
                        }]
                    }
                ]
            },
        )
    }
}
