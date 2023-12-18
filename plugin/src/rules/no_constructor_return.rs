use std::sync::Arc;

use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{
    ast_helpers::{get_method_definition_kind, MethodDefinitionKind},
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
        listeners => [
            r#"
              (return_statement) @c
            "# => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                let code_path = code_path_analyzer.get_innermost_code_path(node);
                let code_path = &code_path_analyzer.code_path_arena[code_path];
                let code_path_root_node =
                    code_path.root_node(&code_path_analyzer.code_path_segment_arena);
                if code_path_root_node.kind() == MethodDefinition
                    && get_method_definition_kind(code_path_root_node, context)
                        == MethodDefinitionKind::Constructor
                    && (node.parent().unwrap().parent().unwrap() == code_path_root_node
                        || node.has_non_comment_named_children(context))
                {
                    context.report(violation! {
                        node => node,
                        message_id => "unexpected",
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::{get_instance_provider_factory, kind::ReturnStatement};

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
            get_instance_provider_factory(),
        )
    }
}
