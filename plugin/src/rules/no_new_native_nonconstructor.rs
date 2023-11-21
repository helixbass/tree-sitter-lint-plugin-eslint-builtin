use std::sync::Arc;

use squalid::OptionExt;
use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{kind::NewExpression, scope::ScopeManager};

pub fn no_new_native_nonconstructor_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-new-native-nonconstructor",
        languages => [Javascript],
        messages => [
            no_new_nonconstructor => "`{{name}}` cannot be called as a constructor.",
        ],
        listeners => [
            "program:exit" => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let global_scope = scope_manager.get_scope(node);

                for non_constructor_name in ["Symbol", "BigInt"] {
                    let set = global_scope.set();
                    let variable = set.get(non_constructor_name);

                    if let Some(variable) = variable.filter(|variable| variable.defs().next().is_none()) {
                        variable.references().for_each(|ref_| {
                            let id_node = ref_.identifier();

                            if id_node.parent().matches(|parent| {
                                parent.kind() == NewExpression && parent.field("constructor") == id_node
                            }) {
                                context.report(violation! {
                                    node => id_node,
                                    message_id => "no_new_nonconstructor",
                                    data => {
                                        name => non_constructor_name.to_owned(),
                                    }
                                });
                            }
                        });
                    }
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
    fn test_no_new_native_nonconstructor_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            no_new_native_nonconstructor_rule(),
            rule_tests! {
                valid => [
                    // Symbol
                    "var foo = Symbol('foo');",
                    "function bar(Symbol) { var baz = new Symbol('baz');}",
                    "function Symbol() {} new Symbol();",
                    "new foo(Symbol);",
                    "new foo(bar, Symbol);",

                    // BigInt
                    "var foo = BigInt(9007199254740991);",
                    "function bar(BigInt) { var baz = new BigInt(9007199254740991);}",
                    "function BigInt() {} new BigInt();",
                    "new foo(BigInt);",
                    "new foo(bar, BigInt);"
                ],
                invalid => [
                    // Symbol
                    {
                        code => "var foo = new Symbol('foo');",
                        errors => [{
                            message => "`Symbol` cannot be called as a constructor."
                        }],
                    },
                    {
                        code => "function bar() { return function Symbol() {}; } var baz = new Symbol('baz');",
                        errors => [{
                            message => "`Symbol` cannot be called as a constructor."
                        }]
                    },

                    // BigInt
                    {
                        code => "var foo = new BigInt(9007199254740991);",
                        errors => [{
                            message => "`BigInt` cannot be called as a constructor."
                        }]
                    },
                    {
                        code => "function bar() { return function BigInt() {}; } var baz = new BigInt(9007199254740991);",
                        errors => [{
                            message => "`BigInt` cannot be called as a constructor."
                        }]
                    }
                ]
            },
            get_instance_provider_factory(),
            // TODO: this is {env: {es2022: true}} in the ESLint version, support that?
            json_object!({
                "ecma_version": 2022,
            }),
        )
    }
}
