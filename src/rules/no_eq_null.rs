use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

pub fn no_eq_null_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-eq-null",
        languages => [Javascript],
        messages => [
            unexpected => "Use '===' to compare with null.",
        ],
        listeners => [
            r#"[
              (binary_expression
                left: (null)
                operator: [
                  "=="
                  "!="
                ]
              )
              (binary_expression
                operator: [
                  "=="
                  "!="
                ]
                right: (null)
              )
            ] @c"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_eq_null_rule() {
        RuleTester::run(
            no_eq_null_rule(),
            rule_tests! {
                valid => [
                    "if (x === null) { }",
                    "if (null === f()) { }"
                ],
                invalid => [
                    { code => "if (x == null) { }", errors => [{ message_id => "unexpected", type => "binary_expression" }] },
                    { code => "if (x != null) { }", errors => [{ message_id => "unexpected", type => "binary_expression" }] },
                    { code => "do {} while (null == x)", errors => [{ message_id => "unexpected", type => "binary_expression" }] }
                ]
            },
        )
    }
}
