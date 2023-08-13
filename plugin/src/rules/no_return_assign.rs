use std::sync::Arc;

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter_grep::return_if_none, violation, NodeExt, Rule};

use crate::kind::{ArrowFunction, ParenthesizedExpression, ReturnStatement};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Always {
    #[default]
    ExceptParens,
    Always,
}

static SENTINEL_TYPE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(?:[a-z_]+_statement|arrow_function|function|class)$"#).unwrap());

pub fn no_return_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-return-assign",
        languages => [Javascript],
        messages => [
            return_assignment => "Return statement should not contain assignment.",
            arrow_assignment => "Arrow function should not return assignment.",
        ],
        options_type => Always,
        state => {
            [per-run]
            always: bool = options != Always::ExceptParens,
        },
        listeners => [
            r#"
              (assignment_expression) @c
            "# => |node, context| {
                if !self.always && node.parent().unwrap().kind() == ParenthesizedExpression {
                    return;
                }

                let mut current_child = node;
                let mut parent = current_child.parent().unwrap();
                while !SENTINEL_TYPE.is_match(parent.kind()) {
                    current_child = parent;
                    parent = return_if_none!(parent.parent());
                }

                if parent.kind() == ReturnStatement {
                    context.report(violation! {
                        node => parent,
                        message_id => "return_assignment",
                    });
                } else if parent.kind() == ArrowFunction &&
                    parent.field("body") == current_child {
                    context.report(violation! {
                        node => parent,
                        message_id => "arrow_assignment",
                    });
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::{ArrowFunction, ReturnStatement};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_return_assign_rule() {
        RuleTester::run(
            no_return_assign_rule(),
            rule_tests! {
                valid => [
                    {
                        code => "module.exports = {'a': 1};",
                        // parserOptions: {
                        //     sourceType: "module"
                        // }
                    },
                    "var result = a * b;",
                    "function x() { var result = a * b; return result; }",
                    "function x() { return (result = a * b); }",
                    {
                        code => "function x() { var result = a * b; return result; }",
                        options => "except-parens"
                    },
                    {
                        code => "function x() { return (result = a * b); }",
                        options => "except-parens"
                    },
                    {
                        code => "function x() { var result = a * b; return result; }",
                        options => "always"
                    },
                    {
                        code => "function x() { return function y() { result = a * b }; }",
                        options => "always"
                    },
                    {
                        code => "() => { return (result = a * b); }",
                        options => "except-parens"
                    },
                    {
                        code => "() => (result = a * b)",
                        options => "except-parens"
                    },
                    "const foo = (a,b,c) => ((a = b), c)",
                    r#"function foo(){
                        return (a = b)
                    }"#,
                    r#"function bar(){
                        return function foo(){
                            return (a = b) && c
                        }
                    }"#,
                    {
                        code => "const foo = (a) => (b) => (a = b)",
                        // parserOptions: { ecmaVersion: 6 }
                    }
                ],
                invalid => [
                    {
                        code => "function x() { return result = a * b; };",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "function x() { return (result) = (a * b); };",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "function x() { return result = a * b; };",
                        options => "except-parens",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "function x() { return (result) = (a * b); };",
                        options => "except-parens",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "() => { return result = a * b; }",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "() => result = a * b",
                        errors => [
                            {
                                message_id => "arrow_assignment",
                                type => ArrowFunction
                            }
                        ]
                    },
                    {
                        code => "function x() { return result = a * b; };",
                        options => "always",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "function x() { return (result = a * b); };",
                        options => "always",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "function x() { return result || (result = a * b); };",
                        options => "always",
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => r#"function foo(){
                            return a = b
                        }"#,
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => r#"function doSomething() {
                            return foo = bar && foo > 0;
                        }"#,
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => r#"function doSomething() {
                            return foo = function(){
                                return (bar = bar1)
                            }
                        }"#,
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => r#"function doSomething() {
                            return foo = () => a
                        }"#,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "return_assignment",
                                type => ReturnStatement
                            }
                        ]
                    },
                    {
                        code => r#"function doSomething() {
                            return () => a = () => b
                        }"#,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "arrow_assignment",
                                type => ArrowFunction
                            }
                        ]
                    },
                    {
                        code => r#"function foo(a){
                            return function bar(b){
                                return a = b
                            }
                        }"#,
                        errors => [{ message_id => "return_assignment", type => ReturnStatement }]
                    },
                    {
                        code => "const foo = (a) => (b) => a = b",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "arrow_assignment",
                                type => ArrowFunction
                            }
                        ]
                    }
                ]
            },
        )
    }
}
