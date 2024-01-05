use std::sync::Arc;

use itertools::Itertools;
use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{ast_helpers::{get_call_expression_arguments, get_function_params}, kind::{Undefined, Identifier, CallExpression}, utils::ast_utils, scope::ScopeManager};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_empty_reject: bool,
}

fn is_promise_reject_call(node: Node, context: &QueryMatchContext) -> bool {
    ast_utils::is_specific_member_access(
        node.field("function"),
        Some("Promise"),
        Some("reject"),
        context,
    )
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
        methods => {
            fn check_reject_call(&self, call_expression: Node, context: &QueryMatchContext) {
                let Some(args) = get_call_expression_arguments(call_expression) else {
                    return;
                };
                let args = args.collect_vec();
                if args.is_empty() && self.allow_empty_reject {
                    return;
                }
                if args.is_empty() ||
                    !ast_utils::could_be_error(args[0], context) ||
                    args[0].kind() == Undefined {
                    context.report(violation! {
                        node => call_expression,
                        message_id => "reject_an_error",
                    });
                }
            }
        },
        listeners => [
            r#"
              (call_expression
                function: (member_expression
                  object: (identifier) @promise (#eq? @promise "Promise")
                )
              ) @call_expression
            "# => |captures, context| {
                let node = captures["call_expression"];
                if !is_promise_reject_call(node, context) {
                    return;
                }
                self.check_reject_call(node, context);
            },
            r#"
              (new_expression
                constructor: (identifier) @promise (#eq? @promise "Promise")
              ) @new_expression
            "# => |captures, context| {
                let node = captures["new_expression"];
                let Some(first_arg) = get_call_expression_arguments(node).unwrap().next().filter(|&arg| {
                    ast_utils::is_function(arg) &&
                        get_function_params(arg).nth(1).matches(|param| param.kind() == Identifier)
                }) else {
                    return;
                };
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                scope_manager.get_declared_variables(first_arg)
                    .find(|variable| {
                        variable.name() == &*get_function_params(first_arg).nth(1).unwrap().text(context)
                    })
                    .unwrap()
                    .references()
                    .filter(|ref_| ref_.is_read())
                    .filter(|ref_| {
                        ref_.identifier().parent().unwrap().kind() == CallExpression &&
                            ref_.identifier() == ref_.identifier().parent().unwrap().field("function")
                    })
                    .for_each(|ref_| {
                        self.check_reject_call(
                            ref_.identifier().parent().unwrap(),
                            context,
                        );
                    });
            }
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
