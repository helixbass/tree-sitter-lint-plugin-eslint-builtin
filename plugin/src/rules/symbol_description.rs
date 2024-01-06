use std::sync::Arc;

use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, QueryMatchContext, Rule};

use crate::{ast_helpers::get_call_expression_arguments, scope::ScopeManager, utils::ast_utils};

fn check_argument<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
    if get_call_expression_arguments(node).matches(|mut arguments| arguments.next().is_none()) {
        context.report(violation! {
            node => node,
            message_id => "expected",
        });
    }
}

pub fn symbol_description_rule() -> Arc<dyn Rule> {
    rule! {
        name => "symbol-description",
        languages => [Javascript],
        messages => [
            expected => "Expected Symbol to have a description.",
        ],
        listeners => [
            "program:exit" => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let scope = scope_manager.get_scope(node);
                if let Some(variable) = ast_utils::get_variable_by_name(scope, "Symbol").filter(|variable| {
                    variable.defs().next().is_none()
                }) {
                    variable.references().for_each(|reference| {
                        let id_node = reference.identifier();

                        if ast_utils::is_callee(id_node, context) {
                            check_argument(id_node.parent().unwrap(), context);
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use squalid::json_object;
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{get_instance_provider_factory, kind::CallExpression};

    #[test]
    fn test_symbol_description_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            symbol_description_rule(),
            rule_tests! {
                valid => [
                    "Symbol(\"Foo\");",
                    "var foo = \"foo\"; Symbol(foo);",

                    // Ignore if it's shadowed.
                    "var Symbol = function () {}; Symbol();",
                    "Symbol(); var Symbol = function () {};",
                    "function bar() { var Symbol = function () {}; Symbol(); }",

                    // Ignore if it's an argument.
                    "function bar(Symbol) { Symbol(); }"
                ],
                invalid => [
                    {
                        code => "Symbol();",
                        errors => [{
                            message_id => "expected",
                            type => CallExpression
                        }]
                    },
                    {
                        code => "Symbol(); Symbol = function () {};",
                        errors => [{
                            message_id => "expected",
                            type => CallExpression
                        }]
                    }
                ]
            },
            get_instance_provider_factory(),
            json_object!({
                "env": {
                    "es6": true,
                }
            }),
        )
    }
}
