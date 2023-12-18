use std::sync::Arc;

use itertools::Itertools;
use tree_sitter_lint::{rule, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    scope::{ScopeManager, Variable},
    utils::ast_utils,
};

fn check_variable(variable: Variable, context: &QueryMatchContext) {
    ast_utils::get_modifying_references(&variable.references().collect_vec())
        .into_iter()
        .for_each(|reference| {
            context.report(violation! {
                node => reference.identifier(),
                message_id => "const_",
                data => {
                    name => reference.identifier().text(context),
                }
            });
        });
}

pub fn no_const_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-const-assign",
        languages => [Javascript],
        messages => [
            // TODO: rename to const per https://github.com/helixbass/tree-sitter-lint/issues/68?
            const_ => "'{{name}}' is constant.",
        ],
        listeners => [
            r#"
              (lexical_declaration
                kind: "const"
              ) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                scope_manager.get_declared_variables(node).for_each(|variable| {
                    check_variable(variable, context);
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use squalid::json_object;
    use tree_sitter_lint::{instance_provider_factory, rule_tests, RuleTester};

    use super::*;
    use crate::{
        kind::{Identifier, ShorthandPropertyIdentifierPattern},
        ProvidedTypes,
    };

    #[test]
    fn test_no_const_assign_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            no_const_assign_rule(),
            rule_tests! {
                valid => [
                    "const x = 0; { let x; x = 1; }",
                    "const x = 0; function a(x) { x = 1; }",
                    "const x = 0; foo(x);",
                    "for (const x in [1,2,3]) { foo(x); }",
                    "for (const x of [1,2,3]) { foo(x); }",
                    "const x = {key: 0}; x.key = 1;",

                    // ignores non constant.
                    "var x = 0; x = 1;",
                    "let x = 0; x = 1;",
                    "function x() {} x = 1;",
                    "function foo(x) { x = 1; }",
                    "class X {} X = 1;",
                    "try {} catch (x) { x = 1; }"
                ],
                invalid => [
                    {
                        code => "const x = 0; x = 1;",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    },
                    {
                        code => "const {a: x} = {a: 0}; x = 1;",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    },
                    {
                        code => "const x = 0; ({x} = {x: 1});",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => ShorthandPropertyIdentifierPattern }]
                    },
                    {
                        code => "const x = 0; ({a: x = 1} = {});",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    },
                    {
                        code => "const x = 0; x += 1;",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    },
                    {
                        code => "const x = 0; ++x;",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    },
                    {
                        code => "for (const i = 0; i < 10; ++i) { foo(i); }",
                        errors => [{ message_id => "const_", data => { name => "i" }, type => Identifier }]
                    },
                    {
                        code => "const x = 0; x = 1; x = 2;",
                        errors => [
                            { message_id => "const_", data => { name => "x" }, type => Identifier, line => 1, column => 14 },
                            { message_id => "const_", data => { name => "x" }, type => Identifier, line => 1, column => 21 }
                        ]
                    },
                    {
                        code => "const x = 0; function foo() { x = x + 1; }",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    },
                    {
                        code => "const x = 0; function foo(a) { x = a; }",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    },
                    {
                        code => "const x = 0; while (true) { x = x + 1; }",
                        errors => [{ message_id => "const_", data => { name => "x" }, type => Identifier }]
                    }
                ]
            },
            Box::new(instance_provider_factory!(ProvidedTypes)),
            json_object!({"ecma_version": 6}),
        )
    }
}
