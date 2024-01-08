use std::sync::Arc;

use itertools::Itertools;
use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{
    scope::{ScopeManager, VariableType},
    utils::ast_utils,
};

pub fn no_func_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-func-assign",
        languages => [Javascript],
        messages => [
            is_a_function => "'{{name}}' is a function.",
        ],
        listeners => [
            r#"
              (function) @c
              (function_declaration) @c
              (generator_function) @c
              (generator_function_declaration) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();

                scope_manager.get_declared_variables(node)
                    .filter(|variable| {
                        variable.defs().next().unwrap().type_() == VariableType::FunctionName
                    })
                    .for_each(|variable| {
                        ast_utils::get_modifying_references(&variable.references().collect_vec())
                            .into_iter()
                            .for_each(|reference| {
                                context.report(violation! {
                                    node => reference.identifier(),
                                    message_id => "is_a_function",
                                    data => {
                                        name => reference.identifier().text(context),
                                    }
                                });
                            });
                    });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{
        get_instance_provider_factory, kind::Identifier, tests::helpers::tracing_subscribe,
    };

    #[test]
    fn test_no_func_assign_rule() {
        tracing_subscribe();
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_func_assign_rule(),
            rule_tests! {
                valid => [
                    "function foo() { var foo = bar; }",
                    "function foo(foo) { foo = bar; }",
                    "function foo() { var foo; foo = bar; }",
                    { code => "var foo = () => {}; foo = bar;", environment => { ecma_version => 6 } },
                    "var foo = function() {}; foo = bar;",
                    "var foo = function() { foo = bar; };",
                    { code => "import bar from 'bar'; function foo() { var foo = bar; }", environment => { ecma_version => 6, source_type => "module" } }
                ],
                invalid => [
                    {
                        code => "function foo() {}; foo = bar;",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "function foo() { foo = bar; }",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "foo = bar; function foo() { };",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "[foo] = bar; function foo() { };",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "({x: foo = 0} = bar); function foo() { };",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "function foo() { [foo] = bar; }",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "(function() { ({x: foo = 0} = bar); function foo() { }; })();",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "var a = function foo() { foo = 123; };",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
