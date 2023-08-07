use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

pub fn no_ternary_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-ternary",
        languages => [Javascript],
        messages => [
            no_ternary_operator => "Ternary operator used.",
        ],
        listeners => [
            r#"
              (ternary_expression) @c
            "# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "no_ternary_operator",
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
    fn test_no_ternary_rule() {
        RuleTester::run(
            no_ternary_rule(),
            rule_tests! {
                valid => [
                    "\"x ? y\";"
                ],
                invalid => [
                    { code => "var foo = true ? thing : stuff;", errors => [{ message_id => "no_ternary_operator", type => TernaryExpression }] },
                    { code => "true ? thing() : stuff();", errors => [{ message_id => "no_ternary_operator", type => TernaryExpression }] },
                    { code => "function foo(bar) { return bar ? baz : qux; }", errors => [{ message_id => "no_ternary_operator", type => TernaryExpression }] }
                ]
            },
        )
    }
}
