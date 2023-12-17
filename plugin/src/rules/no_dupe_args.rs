use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

use crate::scope::{ScopeManager, VariableType};

pub fn no_dupe_args_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-dupe-args",
        languages => [Javascript],
        messages => [
            unexpected => "Duplicate param '{{name}}'.",
        ],
        listeners => [
            r#"
              (function) @c
              (function_declaration) @c
              (generator_function) @c
              (generator_function_declaration) @c
              (method_definition) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let variables = scope_manager.get_declared_variables(node);

                for variable in variables {
                    if variable.defs().filter(|def| def.type_() == VariableType::Parameter).count() >= 2 {
                        context.report(violation! {
                            node => node,
                            message_id => "unexpected",
                            data => {
                                name => variable.name().to_owned(),
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
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::get_instance_provider_factory;

    #[test]
    fn test_no_dupe_args_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_dupe_args_rule(),
            rule_tests! {
                valid => [
                    "function a(a, b, c){}",
                    "var a = function(a, b, c){}",
                    { code => "function a({a, b}, {c, d}){}", environment => { ecma_version => 6 } },
                    { code => "function a([ , a]) {}", environment => { ecma_version => 6 } },
                    { code => "function foo([[a, b], [c, d]]) {}", environment => { ecma_version => 6 } }
                ],
                invalid => [
                    { code => "function a(a, b, b) {}", errors => [{ message_id => "unexpected", data => { name => "b" } }] },
                    { code => "function a(a, a, a) {}", errors => [{ message_id => "unexpected", data => { name => "a" } }] },
                    { code => "function a(a, b, a) {}", errors => [{ message_id => "unexpected", data => { name => "a" } }] },
                    { code => "function a(a, b, a, b) {}", errors => [{ message_id => "unexpected", data => { name => "a" } }, { message_id => "unexpected", data => { name => "b" } }] },
                    { code => "var a = function(a, b, b) {}", errors => [{ message_id => "unexpected", data => { name => "b" } }] },
                    { code => "var a = function(a, a, a) {}", errors => [{ message_id => "unexpected", data => { name => "a" } }] },
                    { code => "var a = function(a, b, a) {}", errors => [{ message_id => "unexpected", data => { name => "a" } }] },
                    { code => "var a = function(a, b, a, b) {}", errors => [{ message_id => "unexpected", data => { name => "a" } }, { message_id => "unexpected", data => { name => "b" } }] }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
