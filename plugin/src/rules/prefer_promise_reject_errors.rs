use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_empty_reject: bool,
}

pub fn prefer_promise_reject_errors_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-promise-reject-errors",
        languages => [Javascript],
        messages => [
            reject_an_error => "Expected the Promise rejection reason to be an Error.",
        ],
        options_type => Options,
        state => {
            [per-config]
            allow_empty_reject: bool = options.allow_empty_reject,
        },
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
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::CallExpression;

    #[test]
    fn test_prefer_promise_reject_errors_rule() {
        let errors = vec![RuleTestExpectedErrorBuilder::default()
            .message_id("reject_an_error")
            .type_(CallExpression)
            .build()
            .unwrap()];

        RuleTester::run(
            prefer_promise_reject_errors_rule(),
            rule_tests! {
                valid => [
                    "Promise.resolve(5)",
                    "Foo.reject(5)",
                    "Promise.reject(foo)",
                    "Promise.reject(foo.bar)",
                    "Promise.reject(foo.bar())",
                    "Promise.reject(new Error())",
                    "Promise.reject(new TypeError)",
                    "Promise.reject(new Error('foo'))",
                    "Promise.reject(foo || 5)",
                    "Promise.reject(5 && foo)",
                    "new Foo((resolve, reject) => reject(5))",
                    "new Promise(function(resolve, reject) { return function(reject) { reject(5) } })",
                    "new Promise(function(resolve, reject) { if (foo) { const reject = somethingElse; reject(5) } })",
                    "new Promise(function(resolve, {apply}) { apply(5) })",
                    "new Promise(function(resolve, reject) { resolve(5, reject) })",
                    "async function foo() { Promise.reject(await foo); }",
                    {
                        code => "Promise.reject()",
                        options => { allow_empty_reject => true }
                    },
                    {
                        code => "new Promise(function(resolve, reject) { reject() })",
                        options => { allow_empty_reject => true }
                    },

                    // Optional chaining
                    "Promise.reject(obj?.foo)",
                    "Promise.reject(obj?.foo())",

                    // Assignments
                    "Promise.reject(foo = new Error())",
                    "Promise.reject(foo ||= 5)",
                    "Promise.reject(foo.bar ??= 5)",
                    "Promise.reject(foo[bar] ??= 5)",

                    // Private fields
                    "class C { #reject; foo() { Promise.#reject(5); } }",
                    "class C { #error; foo() { Promise.reject(this.#error); } }"
                ],
                invalid => [
                    {
                        code => "Promise.reject(5)",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject('foo')",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(`foo`)",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(!foo)",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(void foo)",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject()",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(undefined)",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject({ foo: 1 })",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject([1, 2, 3])",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject()",
                        options => { allow_empty_reject => false },
                        errors => errors,
                    },
                    {
                        code => "new Promise(function(resolve, reject) { reject() })",
                        options => { allow_empty_reject => false },
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(undefined)",
                        options => { allow_empty_reject => true },
                        errors => errors,
                    },
                    {
                        code => "Promise.reject('foo', somethingElse)",
                        errors => errors,
                    },
                    {
                        code => "new Promise(function(resolve, reject) { reject(5) })",
                        errors => errors,
                    },
                    {
                        code => "new Promise((resolve, reject) => { reject(5) })",
                        errors => errors,
                    },
                    {
                        code => "new Promise((resolve, reject) => reject(5))",
                        errors => errors,
                    },
                    {
                        code => "new Promise((resolve, reject) => reject())",
                        errors => errors,
                    },
                    {
                        code => "new Promise(function(yes, no) { no(5) })",
                        errors => errors,
                    },
                    {
                        code => r#"
                          new Promise((resolve, reject) => {
                            fs.readFile('foo.txt', (err, file) => {
                              if (err) reject('File not found')
                              else resolve(file)
                            })
                          })
                        "#,
                        errors => errors,
                    },
                    {
                        code => "new Promise(({foo, bar, baz}, reject) => reject(5))",
                        errors => errors,
                    },
                    {
                        code => "new Promise(function(reject, reject) { reject(5) })",
                        errors => errors,
                    },
                    {
                        code => "new Promise(function(foo, arguments) { arguments(5) })",
                        errors => errors,
                    },
                    {
                        code => "new Promise((foo, arguments) => arguments(5))",
                        errors => errors,
                    },
                    {
                        code => "new Promise(function({}, reject) { reject(5) })",
                        errors => errors,
                    },
                    {
                        code => "new Promise(({}, reject) => reject(5))",
                        errors => errors,
                    },
                    {
                        code => "new Promise((resolve, reject, somethingElse = reject(5)) => {})",
                        errors => errors,
                    },

                    // Optional chaining
                    {
                        code => "Promise.reject?.(5)",
                        errors => errors,
                    },
                    {
                        code => "Promise?.reject(5)",
                        errors => errors,
                    },
                    {
                        code => "Promise?.reject?.(5)",
                        errors => errors,
                    },
                    {
                        code => "(Promise?.reject)(5)",
                        errors => errors,
                    },
                    {
                        code => "(Promise?.reject)?.(5)",
                        errors => errors,
                    },

                    // Assignments with mathematical operators will either evaluate to a primitive value or throw a TypeError
                    {
                        code => "Promise.reject(foo += new Error())",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(foo -= new Error())",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(foo **= new Error())",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(foo <<= new Error())",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(foo |= new Error())",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(foo &= new Error())",
                        errors => errors,
                    },

                    // evaluates either to a falsy value of `foo` (which, then, cannot be an Error object), or to `5`
                    {
                        code => "Promise.reject(foo && 5)",
                        errors => errors,
                    },
                    {
                        code => "Promise.reject(foo &&= 5)",
                        errors => errors,
                    },
                ]
            },
        )
    }
}
