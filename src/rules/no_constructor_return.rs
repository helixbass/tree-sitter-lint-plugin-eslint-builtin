use std::sync::Arc;

use tree_sitter_lint::{rule, tree_sitter::Node, violation, Rule};

use crate::{
    ast_helpers::{get_method_definition_kind, MethodDefinitionKind, NodeExtJs},
    kind::MethodDefinition,
    CodePathAnalyzer,
};

pub fn no_constructor_return_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-constructor-return",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected return statement in constructor.",
        ],
        state => {
            [per-file-run]
            stack: Vec<Node<'a>>,
        },
        listeners => [
            // ON_CODE_PATH_START => |node, context| {
            //     let code_path_analyzer = get_code_path_analyzer(context);
            //     let node = code_path_analyzer.get_on_code_path_start_payload();
            //     self.stack.push(node);
            // },
            // ON_CODE_PATH_END => |node, context| {
            //     self.stack.pop();
            // },
            // r#"
            //   (return_statement) @c
            // "# => |node, context| {
            //     let &last = self.stack.last().unwrap();

            //     if last.kind() == MethodDefinition &&
            //         get_method_definition_kind(last, context) == MethodDefinitionKind::Constructor && (
            //             node.parent().unwrap().parent().unwrap() == last ||
            //             node.has_non_comment_named_children()
            //         ) {
            //         context.report(violation! {
            //             node => node,
            //             message_id => "unexpected",
            //         });
            //     }
            // },
            r#"
              (program) @c
            "# => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use crate::{kind::ReturnStatement, CodePathAnalyzerInstanceProviderFactory};

    #[test]
    fn test_no_constructor_return_rule() {
        let errors = [RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected")
            .type_(ReturnStatement)
            .build()
            .unwrap()];

        RuleTester::run_with_from_file_run_context_instance_provider(
            no_constructor_return_rule(),
            rule_tests! {
                valid => [
                    "function fn() { return }",
                    "function fn(kumiko) { if (kumiko) { return kumiko } }",
                    "const fn = function () { return }",
                    "const fn = function () { if (kumiko) { return kumiko } }",
                    "const fn = () => { return }",
                    "const fn = () => { if (kumiko) { return kumiko } }",
                    {
                        code => "return 'Kumiko Oumae'",
                        // parserOptions: { ecmaFeatures: { globalReturn: true } }
                    },

                    "class C {  }",
                    "class C { constructor() {} }",
                    "class C { constructor() { let v } }",
                    "class C { method() { return '' } }",
                    "class C { get value() { return '' } }",
                    "class C { constructor(a) { if (!a) { return } else { a() } } }",
                    "class C { constructor() { function fn() { return true } } }",
                    "class C { constructor() { this.fn = function () { return true } } }",
                    "class C { constructor() { this.fn = () => { return true } } }"
                ],
                invalid => [
                    {
                        code => "class C { constructor() { return } }",
                        errors => errors,
                    },
                    {
                        code => "class C { constructor() { return '' } }",
                        errors => errors,
                    },
                    {
                        code => "class C { constructor(a) { if (!a) { return '' } else { a() } } }",
                        errors => errors,
                    }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
