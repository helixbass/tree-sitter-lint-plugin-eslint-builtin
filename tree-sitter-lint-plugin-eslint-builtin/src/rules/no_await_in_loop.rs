use std::sync::Arc;

use tree_sitter_lint::{rule, tree_sitter::Node, violation, QueryMatchContext, Rule};

use crate::{
    ast_helpers::is_for_of_await,
    kind::{
        ArrowFunction, DoStatement, ForInStatement, ForStatement, Function, FunctionDeclaration,
        MethodDefinition, WhileStatement,
    },
};

pub fn no_await_in_loop_rule() -> Arc<dyn Rule> {
    fn is_boundary(node: Node, context: &QueryMatchContext) -> bool {
        let t = node.kind();

        matches!(
            t,
            FunctionDeclaration | Function | ArrowFunction | MethodDefinition
        ) || t == ForInStatement && is_for_of_await(node, context)
    }

    fn is_looped(node: Node, parent: Node) -> bool {
        match parent.kind() {
            ForStatement => {
                Some(node) == parent.child_by_field_name("condition")
                    || Some(node) == parent.child_by_field_name("increment")
                    || Some(node) == parent.child_by_field_name("body")
            }

            ForInStatement => Some(node) == parent.child_by_field_name("body"),

            WhileStatement | DoStatement => {
                Some(node) == parent.child_by_field_name("condition")
                    || Some(node) == parent.child_by_field_name("body")
            }

            _ => false,
        }
    }

    fn validate(await_node: Node, context: &mut QueryMatchContext) {
        let mut node = await_node;
        let mut parent = node.parent();

        while let Some(parent_present) = parent.filter(|&parent| !is_boundary(parent, context)) {
            if is_looped(node, parent_present) {
                context.report(violation! {
                    node => await_node,
                    message => "Unexpected `await` inside a loop."
                });
            }
            node = parent_present;
            parent = parent_present.parent();
        }
    }

    rule! {
        name => "no-await-in-loop",
        languages => [Javascript],
        listeners => [
            r#"[
              (await_expression)
              (for_in_statement
                "await"
                operator: "of"
              )
            ] @c"# => |node, context| {
                validate(node, context);
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    #[test]
    fn test_no_await_in_loop_rule() {
        let error = &RuleTestExpectedErrorBuilder::default()
            // .message_id("unexpected_await")
            .message("Unexpected `await` inside a loop.")
            .type_("await_expression")
            .build()
            .unwrap();

        RuleTester::run(
            no_await_in_loop_rule(),
            rule_tests! {
                valid => [
                    "async function foo() { await bar; }",
                    "async function foo() { for (var bar in await baz) { } }",
                    "async function foo() { for (var bar of await baz) { } }",
                    "async function foo() { for await (var bar of await baz) { } }",
                    "async function foo() { for (var bar = await baz in qux) {} }",

                    // While loops
                    "async function foo() { while (true) { async function foo() { await bar; } } }", // Blocked by a function declaration
                    // For loops
                    "async function foo() { for (var i = await bar; i < n; i++) {  } }",

                    // Do while loops
                    "async function foo() { do { } while (bar); }",

                    // Blocked by a function expression
                    "async function foo() { while (true) { var y = async function() { await bar; } } }",

                    // Blocked by an arrow function
                    "async function foo() { while (true) { var y = async () => await foo; } }",
                    "async function foo() { while (true) { var y = async () => { await foo; } } }",

                    // Blocked by a class method
                    "async function foo() { while (true) { class Foo { async foo() { await bar; } } } }",

                    // Asynchronous iteration intentionally
                    "async function foo() { for await (var x of xs) { await f(x) } }"
                ],
                invalid => [
                    // While loops
                    { code => "async function foo() { while (baz) { await bar; } }", errors => [error] },
                    { code => "async function foo() { while (await foo()) {  } }", errors => [error] },
                    {
                        code => "async function foo() { while (baz) { for await (x of xs); } }",
                        errors => [error.with_type("for_in_statement")]
                    },

                    // For of loops
                    { code => "async function foo() { for (var bar of baz) { await bar; } }", errors => [error] },
                    { code => "async function foo() { for (var bar of baz) await bar; }", errors => [error] },

                    // For in loops
                    { code => "async function foo() { for (var bar in baz) { await bar; } }", errors => [error] },

                    // For loops
                    { code => "async function foo() { for (var i; i < n; i++) { await bar; } }", errors => [error] },
                    { code => "async function foo() { for (var i; await foo(i); i++) {  } }", errors => [error] },
                    { code => "async function foo() { for (var i; i < n; i = await bar) {  } }", errors => [error] },

                    // Do while loops
                    { code => "async function foo() { do { await bar; } while (baz); }", errors => [error] },
                    { code => "async function foo() { do { } while (await bar); }", errors => [error] },

                    // Deep in a loop body
                    { code => "async function foo() { while (true) { if (bar) { foo(await bar); } } }", errors => [error] },

                    // Deep in a loop condition
                    { code => "async function foo() { while (xyz || 5 > await x) {  } }", errors => [error] },

                    // In a nested loop of for-await-of
                    { code => "async function foo() { for await (var x of xs) { while (1) await f(x) } }", errors => [error] }
                ]
            },
        )
    }
}
