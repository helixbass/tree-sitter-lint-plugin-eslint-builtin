use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_new_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-new",
        languages => [Javascript],
        messages => [
            no_new_statement => "Do not use 'new' for side effects.",
        ],
        listeners => [
            r#"(
              (expression_statement
                (new_expression)
              ) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "no_new_statement",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::ExpressionStatement;

    #[test]
    fn test_no_new_rule() {
        RuleTester::run(
            no_new_rule(),
            rule_tests! {
                valid => [
                    "var a = new Date()",
                    "var a; if (a === new Date()) { a = false; }"
                ],
                invalid => [
                    {
                        code => "new Date()",
                        errors => [{
                            message_id => "no_new_statement",
                            type => ExpressionStatement
                        }]
                    }
                ]
            },
        )
    }
}
