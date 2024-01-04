use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn prefer_rest_params_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-rest-params",
        languages => [Javascript],
        messages => [
            prefer_rest_params => "Use the rest parameters instead of 'arguments'.",
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
    use crate::kind::Identifier;

    #[test]
    fn test_prefer_rest_params_rule() {
        RuleTester::run(
            prefer_rest_params_rule(),
            rule_tests! {
                valid => [
                    "arguments;",
                    "function foo(arguments) { arguments; }",
                    "function foo() { var arguments; arguments; }",
                    "var foo = () => arguments;", // Arrows don't have "arguments".,
                    "function foo(...args) { args; }",
                    "function foo() { arguments.length; }",
                    "function foo() { arguments.callee; }"
                ],
                invalid => [
                    { code => "function foo() { arguments; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] },
                    { code => "function foo() { arguments[0]; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] },
                    { code => "function foo() { arguments[1]; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] },
                    { code => "function foo() { arguments[Symbol.iterator]; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] }
                ]
            },
        )
    }
}
