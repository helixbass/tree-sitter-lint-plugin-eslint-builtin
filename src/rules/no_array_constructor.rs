use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

use crate::ast_helpers::get_num_call_expression_arguments;

pub fn no_array_constructor_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-array-constructor",
        languages => [Javascript],
        messages => [
            prefer_literal => "The array literal notation [] is preferable.",
        ],
        listeners => [
            r#"
              (call_expression
                function: (identifier) @function_name (#eq? @function_name "Array")
              ) @call_expression
              (new_expression
                constructor: (identifier) @function_name (#eq? @function_name "Array")
              ) @call_expression
            "# => {
                capture_name => "call_expression",
                callback => |node, context| {
                    if get_num_call_expression_arguments(node) != Some(1) {
                        context.report(violation! {
                            node => node,
                            message_id => "prefer_literal",
                        });
                    }
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_array_constructor_rule() {
        RuleTester::run(
            no_array_constructor_rule(),
            rule_tests! {
                valid => [
                    "new Array(x)",
                    "Array(x)",
                    "new Array(9)",
                    "Array(9)",
                    "new foo.Array()",
                    "foo.Array()",
                    "new Array.foo",
                    "Array.foo()"
                ],
                invalid => [
                    { code => "new Array()", errors => [{ message_id => "prefer_literal", type => "new_expression" }] },
                    { code => "new Array", errors => [{ message_id => "prefer_literal", type => "new_expression" }] },
                    { code => "new Array(x, y)", errors => [{ message_id => "prefer_literal", type => "new_expression" }] },
                    { code => "new Array(0, 1, 2)", errors => [{ message_id => "prefer_literal", type => "new_expression" }] }
                ]
            },
        )
    }
}
