use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

pub fn no_nested_ternary_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-debugger",
        languages => [Javascript],
        messages => [
            no_nested_ternary => "Do not nest ternary expressions.",
        ],
        listeners => [
            r#"[
              (ternary_expression
                consequence: (ternary_expression)
              )
              (ternary_expression
                alternative: (ternary_expression)
              )
            ] @c
            "# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "no_nested_ternary",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::TernaryExpression;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_nested_ternary_rule() {
        RuleTester::run(
            no_nested_ternary_rule(),
            rule_tests! {
                valid => [
                    "foo ? doBar() : doBaz();",
                    "var foo = bar === baz ? qux : quxx;"
                ],
                invalid => [
                    {
                        code => "foo ? bar : baz === qux ? quxx : foobar;",
                        errors => [{
                            message_id => "no_nested_ternary",
                            type => TernaryExpression,
                        }]
                    },
                    {
                        code => "foo ? baz === qux ? quxx : foobar : bar;",
                        errors => [{
                            message_id => "no_nested_ternary",
                            type => TernaryExpression,
                        }]
                    }
                ]
            },
        )
    }
}
