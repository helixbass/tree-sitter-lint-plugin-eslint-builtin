use std::sync::Arc;

use tree_sitter_lint::{
    rule, tree_sitter::Node, violation, FromFileRunContextInstanceProviderFactory, NodeExt,
    QueryMatchContext, Rule, ROOT_EXIT,
};

use crate::ast_helpers::NodeExtJs;

fn pop_stack(
    stack: &mut Vec<(Node, usize)>,
    node: Node,
    context: &QueryMatchContext<impl FromFileRunContextInstanceProviderFactory>,
) {
    while !stack.is_empty() {
        let (current, count_yield) = stack.last().copied().unwrap();
        if !node.is_descendant_of(current) {
            stack.pop().unwrap();
            if count_yield == 0 && current.field("body").has_non_comment_named_children() {
                context.report(violation! {
                    node => current,
                    message_id => "missing_yield",
                });
            }
        } else {
            return;
        }
    }
}

pub fn require_yield_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "require-yield",
        languages => [Javascript],
        messages => [
            missing_yield => "This generator function does not have 'yield'.",
        ],
        state => {
            [per-file-run]
            stack: Vec<(Node<'a>, usize)>,
        },
        listeners => [
            r#"
              (generator_function) @c
              (generator_function_declaration) @c
              (method_definition
                "*"
              ) @c
            "# => |node, context| {
                pop_stack(&mut self.stack, node, context);
                self.stack.push((node, 0));
            },
            r#"
              (yield_expression) @c
            "# => |node, context| {
                pop_stack(&mut self.stack, node, context);
                if let Some((_, last_count)) = self.stack.last_mut() {
                    *last_count += 1;
                }
            },
            ROOT_EXIT => |node, context| {
                pop_stack(&mut self.stack, node, context);
            }
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
