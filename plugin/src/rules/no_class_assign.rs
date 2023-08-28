use std::sync::Arc;

use itertools::Itertools;
use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{scope::ScopeManager, utils::ast_utils};

pub fn no_class_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-class-assign",
        languages => [Javascript],
        messages => [
            class => "'{{name}}' is a class.",
        ],
        listeners => [
            r#"
              (class_declaration) @c
              (class) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();

                scope_manager.get_declared_variables(node).for_each(|variable| {
                    ast_utils::get_modifying_references(&variable.references().collect_vec())
                        .into_iter()
                        .for_each(|reference| {
                            context.report(violation! {
                                node => reference.identifier(),
                                message_id => "class",
                                data => {
                                    name => reference.identifier().text(context)
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
    use squalid::json_object;
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{
        get_instance_provider_factory,
        kind::{Identifier, ShorthandPropertyIdentifierPattern},
    };

    #[test]
    fn test_no_class_assign_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            no_class_assign_rule(),
            rule_tests! {
                valid => [
                    "class A { } foo(A);",
                    "let A = class A { }; foo(A);",
                    "class A { b(A) { A = 0; } }",
                    "class A { b() { let A; A = 0; } }",
                    "let A = class { b() { A = 0; } }",

                    // ignores non class.
                    "var x = 0; x = 1;",
                    "let x = 0; x = 1;",
                    "const x = 0; x = 1;",
                    "function x() {} x = 1;",
                    "function foo(x) { x = 1; }",
                    "try {} catch (x) { x = 1; }"
                ],
                invalid => [
                    {
                        code => "class A { } A = 0;",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "class A { } ({A} = 0);",
                        errors => [{ message_id => "class", data => { name => "A" }, type => ShorthandPropertyIdentifierPattern }]
                    },
                    {
                        code => "class A { } ({b: A = 0} = {});",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "A = 0; class A { }",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "class A { b() { A = 0; } }",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "let A = class A { b() { A = 0; } }",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "class A { } A = 0; A = 1;",
                        errors => [
                            { message_id => "class", data => { name => "A" }, type => Identifier, line => 1, column => 13 },
                            { message_id => "class", data => { name => "A" }, type => Identifier, line => 1, column => 20 }
                        ]
                    }
                ]
            },
            get_instance_provider_factory(),
            json_object!({"ecma_version": 6}),
        )
    }
}
