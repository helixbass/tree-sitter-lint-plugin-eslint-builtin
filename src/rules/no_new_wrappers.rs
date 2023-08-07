use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

use crate::ast_helpers::NodeExtJs;

pub fn no_new_wrappers_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-new_wrapper",
        languages => [Javascript],
        messages => [
            no_constructor => "Do not use {{fn_}} as a constructor.",
        ],
        listeners => [
            r#"
              (new_expression
                constructor: (_) @constructor (#match? @constructor "^(?:String|Number|Boolean)$")
              ) @new_expression
            "# => |captures, context| {
                context.report(violation! {
                    node => captures["new_expression"],
                    message_id => "no_constructor",
                    data => {
                        fn_ => captures["constructor"].text(context),
                    }
                });
            }
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::NewExpression;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_new_wrappers_rule() {
        RuleTester::run(
            no_new_wrappers_rule(),
            rule_tests! {
                valid => [
                    "var a = new Object();",
                    "var a = String('test'), b = String.fromCharCode(32);",
                ],
                invalid => [
                    {
                        code => "var a = new String('hello');",
                        errors => [{
                            message_id => "no_constructor",
                            data => {
                                fn_ => "String"
                            },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "var a = new Number(10);",
                        errors => [{
                            message_id => "no_constructor",
                            data => {
                                fn_ => "Number"
                            },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "var a = new Boolean(false);",
                        errors => [{
                            message_id => "no_constructor",
                            data => {
                                fn_ => "Boolean"
                            },
                            type => NewExpression
                        }]
                    }
                ]
            },
        )
    }
}
