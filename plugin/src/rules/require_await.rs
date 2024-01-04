use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn require_await_rule() -> Arc<dyn Rule> {
    rule! {
        name => "require-await",
        languages => [Javascript],
        messages => [
            missing_await => "{{name}} has no 'await' expression.",
        ],
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_require_await_rule() {
        RuleTester::run(
            require_await_rule(),
            rule_tests! {
                valid => [
                    "async function foo() { await doSomething() }",
                    "(async function() { await doSomething() })",
                    "async () => { await doSomething() }",
                    "async () => await doSomething()",
                    "({ async foo() { await doSomething() } })",
                    "class A { async foo() { await doSomething() } }",
                    "(class { async foo() { await doSomething() } })",
                    "async function foo() { await (async () => { await doSomething() }) }",

                    // empty functions are ok.
                    "async function foo() {}",
                    "async () => {}",

                    // normal functions are ok.
                    "function foo() { doSomething() }",

                    // for-await-of
                    "async function foo() { for await (x of xs); }",

                    // global await
                    {
                        code => "await foo()",
                        // environment => {
                        //     parser: require("../../fixtures/parsers/typescript-parsers/global-await")
                        // }
                    },
                    {
                        code => r#"
                            for await (let num of asyncIterable) {
                                console.log(num);
                            }
                        "#,
                        // environment => {
                        //     parser: require("../../fixtures/parsers/typescript-parsers/global-for-await-of")
                        // }
                    },
                    {
                        code => "async function* run() { yield * anotherAsyncGenerator() }",
                        environment => { ecma_version => 9 }
                    },
                    {
                        code => r#"async function* run() {
                            await new Promise(resolve => setTimeout(resolve, 100));
                            yield 'Hello';
                            console.log('World');
                        }
                        "#,
                        environment => { ecma_version => 9 }
                    },
                    {
                        code => "async function* run() { }",
                        environment => { ecma_version => 9 }
                    },
                    {
                        code => "const foo = async function *(){}",
                        environment => { ecma_version => 9 }
                    },
                    {
                        code => r#"const foo = async function *(){ console.log("bar") }"#,
                        environment => { ecma_version => 9 }
                    },
                    {
                        code => r#"async function* run() { console.log("bar") }"#,
                        environment => { ecma_version => 9 }
                    }

                ],
                invalid => [
                    {
                        code => "async function foo() { doSomething() }",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async function 'foo'" }
                        }]
                    },
                    {
                        code => "(async function() { doSomething() })",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async function" }
                        }]
                    },
                    {
                        code => "async () => { doSomething() }",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async arrow function" }
                        }]
                    },
                    {
                        code => "async () => doSomething()",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async arrow function" }
                        }]
                    },
                    {
                        code => "({ async foo() { doSomething() } })",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async method 'foo'" }
                        }]
                    },
                    {
                        code => "class A { async foo() { doSomething() } }",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async method 'foo'" }
                        }]
                    },
                    {
                        code => "(class { async foo() { doSomething() } })",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async method 'foo'" }
                        }]
                    },
                    {
                        code => "(class { async ''() { doSomething() } })",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async method ''" }
                        }]
                    },
                    {
                        code => "async function foo() { async () => { await doSomething() } }",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async function 'foo'" }
                        }]
                    },
                    {
                        code => "async function foo() { await (async () => { doSomething() }) }",
                        errors => [{
                            message_id => "missing_await",
                            data => { name => "Async arrow function" }
                        }]
                    }
                ]
            },
        )
    }
}
