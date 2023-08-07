use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

pub fn no_async_promise_executor_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-async-promise-executor",
        languages => [Javascript],
        listeners => [
            r#"(
              (new_expression
                constructor: (identifier) @callee (#eq? @callee "Promise")
                arguments: (arguments
                  .
                  [
                    (arrow_function
                      "async" @async_keyword
                    )
                    (function
                      "async" @async_keyword
                    )
                    ; ok crazy test case
                    (parenthesized_expression
                      (parenthesized_expression
                        (parenthesized_expression
                          (parenthesized_expression
                            (arrow_function
                              "async" @async_keyword
                            )
                          )
                        )
                      )
                    )
                  ]
                )
              )
            )"# => |captures, context| {
                context.report(violation! {
                    node => captures["async_keyword"],
                    message => "Promise executor functions should not be async.",
                })
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_async_promise_executor_rule() {
        RuleTester::run(
            no_async_promise_executor_rule(),
            rule_tests! {
                valid => [
                    "new Promise((resolve, reject) => {})",
                    "new Promise((resolve, reject) => {}, async function unrelated() {})",
                    "new Foo(async (resolve, reject) => {})"
                ],
                invalid => [
                    {
                        code => "new Promise(async function foo(resolve, reject) {})",
                        errors => [
                            {
                                message => "Promise executor functions should not be async.",
                                line => 1,
                                column => 13,
                                end_line => 1,
                                end_column => 18
                            }]
                    },
                    {
                        code => "new Promise(async (resolve, reject) => {})",
                        errors => [{
                            message => "Promise executor functions should not be async.",
                            line => 1,
                            column => 13,
                            end_line => 1,
                            end_column => 18
                        }]
                    },
                    {
                        code => "new Promise(((((async () => {})))))",
                        errors => [{
                            message => "Promise executor functions should not be async.",
                            line => 1,
                            column => 17,
                            end_line => 1,
                            end_column => 22
                        }]
                    }
                ]
            },
        )
    }
}
