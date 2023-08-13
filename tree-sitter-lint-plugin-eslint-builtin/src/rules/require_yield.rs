use std::sync::Arc;

use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{
    ast_helpers::{is_generator_method_definition, NodeExtJs},
    kind::MethodDefinition,
};

pub fn require_yield_rule() -> Arc<dyn Rule> {
    rule! {
        name => "require-yield",
        languages => [Javascript],
        messages => [
            missing_yield => "This generator function does not have 'yield'.",
        ],
        state => {
            [per-file-run]
            stack: Vec<usize>,
        },
        listeners => [
            r#"
              (generator_function) @c
              (generator_function_declaration) @c
              (method_definition
                "*"
              ) @c
            "# => |node, context| {
                self.stack.push(0);
            },
            r#"
              generator_function:exit,
              generator_function_declaration:exit,
              method_definition:exit
            "# => |node, context| {
                if node.kind() == MethodDefinition && !is_generator_method_definition(node, context) {
                    return;
                }
                let count_yield = self.stack.pop().unwrap();
                if count_yield == 0 && node.field("body").has_non_comment_named_children() {
                    context.report(violation! {
                        node => node,
                        message_id => "missing_yield",
                    });
                }
            },
            r#"
              (yield_expression) @c
            "# => |node, context| {
                if let Some(last_count) = self.stack.last_mut() {
                    *last_count += 1;
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::{GeneratorFunction, GeneratorFunctionDeclaration, MethodDefinition};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_require_yield_rule() {
        RuleTester::run(
            require_yield_rule(),
            rule_tests! {
                valid => [
                    "function foo() { return 0; }",
                    "function* foo() { yield 0; }",
                    "function* foo() { }",
                    "(function* foo() { yield 0; })();",
                    "(function* foo() { })();",
                    "var obj = { *foo() { yield 0; } };",
                    "var obj = { *foo() { } };",
                    "class A { *foo() { yield 0; } };",
                    "class A { *foo() { } };"
                ],
                invalid => [
                    {
                        code => "function* foo() { return 0; }",
                        errors => [{ message_id => "missing_yield", type => GeneratorFunctionDeclaration }]
                    },
                    {
                        code => "(function* foo() { return 0; })();",
                        errors => [{ message_id => "missing_yield", type => GeneratorFunction }]
                    },
                    {
                        code => "var obj = { *foo() { return 0; } }",
                        errors => [{ message_id => "missing_yield", type => MethodDefinition }]
                    },
                    {
                        code => "class A { *foo() { return 0; } }",
                        errors => [{ message_id => "missing_yield", type => MethodDefinition }]
                    },
                    {
                        code => "function* foo() { function* bar() { yield 0; } }",
                        errors => [{
                            message_id => "missing_yield",
                            type => GeneratorFunctionDeclaration,
                            column => 1
                        }]
                    },
                    {
                        code => "function* foo() { function* bar() { return 0; } yield 0; }",
                        errors => [{
                            message_id => "missing_yield",
                            type => GeneratorFunctionDeclaration,
                            column => 19
                        }]
                    }
                ]
            },
        )
    }
}
