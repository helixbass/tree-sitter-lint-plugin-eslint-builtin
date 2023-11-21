use std::sync::Arc;

use squalid::OptionExt;
use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{kind::NewExpression, scope::ScopeManager};

pub fn no_new_symbol_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-new-symbol",
        languages => [Javascript],
        messages => [
            no_new_symbol => "`Symbol` cannot be called as a constructor.",
        ],
        listeners => [
            "program:exit" => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let global_scope = scope_manager.get_scope(node);
                let set = global_scope.set();
                let variable = set.get("Symbol");

                if let Some(variable) = variable.filter(|variable| variable.defs().next().is_none()) {
                    variable.references().for_each(|ref_| {
                        let id_node = ref_.identifier();

                        if id_node.parent().matches(|parent| {
                            parent.kind() == NewExpression && parent.field("constructor") == id_node
                        }) {
                            context.report(violation! {
                                node => id_node,
                                message_id => "no_new_symbol",
                            });
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
    use crate::get_instance_provider_factory;

    #[test]
    fn test_no_new_symbol_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            no_new_symbol_rule(),
            rule_tests! {
                valid => [
                    "var foo = Symbol('foo');",
                    "function bar(Symbol) { var baz = new Symbol('baz');}",
                    "function Symbol() {} new Symbol();",
                    "new foo(Symbol);",
                    "new foo(bar, Symbol);"
                ],
                invalid => [
                    {
                        code => "var foo = new Symbol('foo');",
                        errors => [{ message_id => "no_new_symbol" }]
                    },
                    {
                        code => "function bar() { return function Symbol() {}; } var baz = new Symbol('baz');",
                        errors => [{ message_id => "no_new_symbol" }]
                    }
                ]
            },
            get_instance_provider_factory(),
            // TODO: this is {env: {es6: true}} in the ESLint version, support that?
            json_object!({
                "ecma_version": 6,
            }),
        )
    }
}
