use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

pub fn default_case_last_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "default-case-last",
        languages => [Javascript],
        messages => [
            not_last => "Default clause should be the last clause.",
        ],
        listeners => [
            r#"
              (switch_statement
                body: (switch_body
                  (switch_default) @c
                  (switch_case)
                )
              )
            "# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "not_last",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::SwitchDefault;

    use super::*;

    use tree_sitter_lint::{
        rule_tests, RuleTestExpectedError, RuleTestExpectedErrorBuilder, RuleTester,
    };

    fn error(column: usize) -> RuleTestExpectedError {
        RuleTestExpectedErrorBuilder::default()
            .message_id("not_last")
            .type_(SwitchDefault)
            .column(column)
            .build()
            .unwrap()
    }

    #[test]
    fn test_default_case_last_rule() {
        RuleTester::run(
            default_case_last_rule(),
            rule_tests! {
                valid => [
                    "switch (foo) {}",
                    "switch (foo) { case 1: bar(); break; }",
                    "switch (foo) { case 1: break; }",
                    "switch (foo) { case 1: }",
                    "switch (foo) { case 1: bar(); break; case 2: baz(); break; }",
                    "switch (foo) { case 1: break; case 2: break; }",
                    "switch (foo) { case 1: case 2: break; }",
                    "switch (foo) { case 1: case 2: }",
                    "switch (foo) { default: bar(); break; }",
                    "switch (foo) { default: bar(); }",
                    "switch (foo) { default: break; }",
                    "switch (foo) { default: }",
                    "switch (foo) { case 1: break; default: break; }",
                    "switch (foo) { case 1: break; default: }",
                    "switch (foo) { case 1: default: break; }",
                    "switch (foo) { case 1: default: }",
                    "switch (foo) { case 1: baz(); break; case 2: quux(); break; default: quuux(); break; }",
                    "switch (foo) { case 1: break; case 2: break; default: break; }",
                    "switch (foo) { case 1: break; case 2: break; default: }",
                    "switch (foo) { case 1: case 2: break; default: break; }",
                    "switch (foo) { case 1: break; case 2: default: break; }",
                    "switch (foo) { case 1: break; case 2: default: }",
                    "switch (foo) { case 1: case 2: default: }"
                ],
                invalid => [
                    {
                        code => "switch (foo) { default: bar(); break; case 1: baz(); break; }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: break; case 1: break; }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: break; case 1: }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: case 1: break; }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: case 1: }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: break; case 1: break; case 2: break; }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: case 1: break; case 2: break; }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: case 1: case 2: break; }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { default: case 1: case 2: }",
                        errors => [error(16)]
                    },
                    {
                        code => "switch (foo) { case 1: break; default: break; case 2: break; }",
                        errors => [error(31)]
                    },
                    {
                        code => "switch (foo) { case 1: default: break; case 2: break; }",
                        errors => [error(24)]
                    },
                    {
                        code => "switch (foo) { case 1: break; default: case 2: break; }",
                        errors => [error(31)]
                    },
                    {
                        code => "switch (foo) { case 1: default: case 2: break; }",
                        errors => [error(24)]
                    },
                    {
                        code => "switch (foo) { case 1: default: case 2: }",
                        errors => [error(24)]
                    }
                ]
            },
        )
    }
}
