use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

use crate::{
    ast_helpers::{is_async_function, is_for_of_await},
    kind::{GeneratorFunction, GeneratorFunctionDeclaration},
    utils::ast_utils,
};

fn capitalize_first_letter(text: &str) -> String {
    format!("{}{}", text[0..1].to_uppercase(), &text[1..])
}

pub fn require_await_rule() -> Arc<dyn Rule> {
    rule! {
        name => "require-await",
        languages => [Javascript],
        messages => [
            missing_await => "{{name}} has no 'await' expression.",
        ],
        state => {
            [per-file-run]
            scope_info: Vec<bool>,
        },
        listeners => [
            r#"
              (function_declaration) @c
              (function) @c
              (generator_function_declaration) @c
              (generator_function) @c
              (method_definition) @c
              (arrow_function) @c
            "# => |node, context| {
                self.scope_info.push(false);
            },
            r#"
              function_declaration:exit,
              function:exit,
              generator_function_declaration:exit,
              generator_function:exit,
              method_definition:exit,
              arrow_function:exit
            "# => |node, context| {
                if !matches!(
                    node.kind(),
                    GeneratorFunctionDeclaration | GeneratorFunction
                ) && is_async_function(node) &&
                    !*self.scope_info.last().unwrap() &&
                    !ast_utils::is_empty_function(node) {
                    context.report(violation! {
                        node => node,
                        range => ast_utils::get_function_head_range(
                            node,
                        ),
                        message_id => "missing_await",
                        data => {
                            name => capitalize_first_letter(
                                &ast_utils::get_function_name_with_kind(
                                    node,
                                    context,
                                )
                            ),
                        }
                    });
                }
                self.scope_info.pop().unwrap();
            },
            r#"
              (await_expression) @c
            "# => |node, context| {
                if let Some(has_await) = self.scope_info.last_mut() {
                    *has_await = true;
                }
            },
            r#"
              (for_in_statement) @c
            "# => |node, context| {
                if let Some(has_await) = self.scope_info.last_mut() {
                    if is_for_of_await(node, context) {
                        *has_await = true;
                    }
                }
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
