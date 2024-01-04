use std::sync::Arc;

use tree_sitter_lint::{rule, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    kind::MemberExpression,
    scope::{Reference, Scope, ScopeManager, Variable},
};

fn get_variable_of_arguments<'a, 'b>(scope: Scope<'a, 'b>) -> Option<Variable<'a, 'b>> {
    scope
        .variables()
        .find(|variable| variable.name() == "arguments")
        .filter(|variable| variable.identifiers().next().is_none())
}

fn is_not_normal_member_access(reference: &Reference) -> bool {
    let id = reference.identifier();
    let parent = id.parent().unwrap();

    !(parent.kind() == MemberExpression && parent.field("object") == id)
}

fn report(reference: Reference, context: &QueryMatchContext) {
    context.report(violation! {
        node => reference.identifier(),
        message_id => "prefer_rest_params",
    });
}

pub fn prefer_rest_params_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-rest-params",
        languages => [Javascript],
        messages => [
            prefer_rest_params => "Use the rest parameters instead of 'arguments'.",
        ],
        listeners => [
            r#"
              function_declaration:exit,
              function:exit
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                if let Some(arguments_var) = get_variable_of_arguments(scope_manager.get_scope(node)) {
                    arguments_var
                        .references()
                        .filter(is_not_normal_member_access)
                        .for_each(|reference| {
                            report(reference, context);
                        });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::Identifier;

    #[test]
    fn test_prefer_rest_params_rule() {
        RuleTester::run(
            prefer_rest_params_rule(),
            rule_tests! {
                valid => [
                    "arguments;",
                    "function foo(arguments) { arguments; }",
                    "function foo() { var arguments; arguments; }",
                    "var foo = () => arguments;", // Arrows don't have "arguments".,
                    "function foo(...args) { args; }",
                    "function foo() { arguments.length; }",
                    "function foo() { arguments.callee; }"
                ],
                invalid => [
                    { code => "function foo() { arguments; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] },
                    { code => "function foo() { arguments[0]; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] },
                    { code => "function foo() { arguments[1]; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] },
                    { code => "function foo() { arguments[Symbol.iterator]; }", errors => [{ type => Identifier, message_id => "prefer_rest_params" }] }
                ]
            },
        )
    }
}
