use std::sync::Arc;

use itertools::Itertools;
use squalid::OptionExt;
use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{scope::ScopeManager, utils::ast_utils};

pub fn no_new_object_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-new-object",
        languages => [Javascript],
        messages => [
            prefer_literal => "The object literal notation {} is preferable.",
        ],
        listeners => [
            r#"
              (new_expression
                constructor: (identifier) @callee (#eq? @callee "Object")
              ) @new_expression
            "# => {
                capture_name => "new_expression",
                callback => |node, context| {
                    let scope_manager = context.retrieve::<ScopeManager<'a>>();

                    let variable = ast_utils::get_variable_by_name(
                        scope_manager.get_scope(node),
                        &node.field("constructor").text(context),
                    );

                    if variable.matches(|variable| !variable.identifiers().collect_vec().is_empty()) {
                        return;
                    }

                    context.report(violation! {
                        node => node,
                        message_id => "prefer_literal",
                    });
                },
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{get_instance_provider_factory, kind::NewExpression};

    #[test]
    fn test_no_new_object_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_new_object_rule(),
            rule_tests! {
                valid => [
                    "var myObject = {};",
                    "var myObject = new CustomObject();",
                    "var foo = new foo.Object()",
                    "var Object = function Object() {};
                        new Object();",
                    "var x = something ? MyClass : Object;
                    var y = new x();",
                    {
                        code => "
                            class Object {
                                constructor(){

                                }
                            }
                            new Object();
                        ",
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "
                            import { Object } from './'
                            new Object();
                        ",
                        environment => { ecma_version => 6, source_type => "module" }
                    }
                ],
                invalid => [
                    {
                        code => "var foo = new Object()",
                        errors => [
                            {
                                message_id => "prefer_literal",
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "new Object();",
                        errors => [{ message_id => "prefer_literal", type => NewExpression }]
                    },
                    {
                        code => "const a = new Object()",
                        environment => { ecma_version => 6 },
                        errors => [{ message_id => "prefer_literal", type => NewExpression }]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
