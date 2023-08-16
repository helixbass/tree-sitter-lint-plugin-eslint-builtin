use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

use crate::utils::ast_utils::get_static_property_name;

pub fn no_proto_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-proto",
        languages => [Javascript],
        messages => [
            unexpected_proto => "The '__proto__' property is deprecated.",
        ],
        listeners => [
            r#"
              (member_expression) @c
              (subscript_expression) @c
            "# => |node, context| {
                if get_static_property_name(node, context).as_deref() == Some("__proto__") {
                    context.report(violation! {
                        node => node,
                        message_id => "unexpected_proto",
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::{MemberExpression, SubscriptExpression};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_proto_rule() {
        RuleTester::run(
            no_proto_rule(),
            rule_tests! {
                valid => [
                    "var a = test[__proto__];",
                    "var __proto__ = null;",
                    { code => "foo[`__proto`] = null;", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo[`__proto__\n`] = null;", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "class C { #__proto__; foo() { this.#__proto__; } }", /*parserOptions: { ecmaVersion: 2022 }*/ }
                ],
                invalid => [
                    { code => "var a = test.__proto__;", errors => [{ message_id => "unexpected_proto", type => MemberExpression }] },
                    { code => "var a = test['__proto__'];", errors => [{ message_id => "unexpected_proto", type => SubscriptExpression }] },
                    { code => "var a = test[`__proto__`];", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected_proto", type => SubscriptExpression }] },
                    { code => "test[`__proto__`] = function () {};", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected_proto", type => SubscriptExpression }] }
                ]
            },
        )
    }
}
