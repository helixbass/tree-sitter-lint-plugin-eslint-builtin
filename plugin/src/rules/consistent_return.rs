use std::{collections::HashMap, sync::Arc};

use id_arena::Id;
use serde::Deserialize;
use squalid::{EverythingExt, OptionExt};
use tree_sitter_lint::{
    rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule, ViolationData,
};

use crate::{
    ast_helpers::{get_method_definition_kind, MethodDefinitionKind, NodeExtJs},
    kind::{ArrowFunction, MethodDefinition, Pair, Program, UnaryExpression, Undefined},
    string_utils::upper_case_first,
    utils::ast_utils,
    CodePath, CodePathAnalyzer,
};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    treat_undefined_as_unspecified: bool,
}

#[derive(Debug)]
struct FuncInfo {
    has_return_value: bool,
    message_id: &'static str,
    data: ViolationData,
}

fn is_class_constructor(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() == MethodDefinition
        && get_method_definition_kind(node, context) == MethodDefinitionKind::Constructor
}

pub fn consistent_return_rule() -> Arc<dyn Rule> {
    rule! {
        name => "consistent-return",
        languages => [Javascript],
        messages => [
            missing_return => "Expected to return a value at the end of {{name}}.",
            missing_return_value => "{{name}} expected a return value.",
            unexpected_return_value => "{{name}} expected no return value.",
        ],
        options_type => Options,
        state => {
            [per-run]
            treat_undefined_as_unspecified: bool = options.treat_undefined_as_unspecified,

            [per-file-run]
            func_infos: HashMap<Id<CodePath<'a>>, FuncInfo>,
        },
        listeners => [
            r#"
              (return_statement) @c
            "# => |node, context| {
                let has_return_value =
                    node.maybe_first_non_comment_named_child()
                        .matches(|argument| {
                            !(self.treat_undefined_as_unspecified
                                && (argument.kind() == Undefined
                                    || argument.kind() == UnaryExpression
                                        && argument.field("operator").kind() == "void"))
                        });

                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                let code_path = code_path_analyzer.get_innermost_code_path(node);

                match self.func_infos.get(&code_path) {
                    None => {
                        self.func_infos.insert(
                            code_path,
                            FuncInfo {
                                has_return_value,
                                message_id: if has_return_value {
                                    "missing_return_value"
                                } else {
                                    "unexpected_return_value"
                                },
                                data: [(
                                    "name".to_owned(),
                                    code_path_analyzer.code_path_arena[code_path]
                                        .root_node(&code_path_analyzer.code_path_segment_arena)
                                        .thrush(|root_node| {
                                            if root_node.kind() == Program {
                                                "Program".to_owned()
                                            } else {
                                                upper_case_first(
                                                    &ast_utils::get_function_name_with_kind(
                                                        root_node, context,
                                                    ),
                                                )
                                            }
                                        }),
                                )]
                                .into(),
                            },
                        );
                    }
                    Some(func_info) if func_info.has_return_value != has_return_value => {
                        context.report(violation! {
                            node => node,
                            message_id => func_info.message_id.to_owned(),
                            data => func_info.data.clone(),
                        });
                    }
                    _ => (),
                }
            },
            "program:exit" => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                for &code_path in code_path_analyzer.code_paths.iter().filter(|&&code_path| {
                    self.func_infos
                        .get(&code_path)
                        .matches(|func_info| func_info.has_return_value)
                        && code_path_analyzer.code_path_arena[code_path]
                            .state
                            .head_segments(&code_path_analyzer.fork_context_arena)
                            .reachable(&code_path_analyzer.code_path_segment_arena)
                        && {
                            let root_node = code_path_analyzer.code_path_arena[code_path]
                                .root_node(&code_path_analyzer.code_path_segment_arena);
                            !ast_utils::is_es5_constructor(root_node, context)
                                && !is_class_constructor(root_node, context)
                        }
                }) {
                    let root_node = code_path_analyzer.code_path_arena[code_path]
                        .root_node(&code_path_analyzer.code_path_segment_arena);

                    let mut name: Option<String> = Default::default();

                    let range = if root_node.kind() == Program {
                        name = Some("program".to_owned());
                        root_node
                    } else if root_node.kind() == ArrowFunction {
                        root_node.get_first_child_of_kind("=>")
                    } else if root_node.kind() == MethodDefinition {
                        root_node.field("name")
                    } else if root_node.parent().unwrap().kind() == Pair {
                        root_node.parent().unwrap().field("key")
                    } else {
                        root_node.child_by_field_name("name").unwrap_or_else(|| {
                            context.get_first_token(root_node, Option::<fn(Node) -> bool>::None)
                        })
                    }
                    .range();

                    let name = name.unwrap_or_else(|| {
                        ast_utils::get_function_name_with_kind(root_node, context)
                    });

                    context.report(violation! {
                        node => root_node,
                        range => range,
                        message_id => "missing_return",
                        data => {
                            name => name,
                        },
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kind::{ArrowFunction, Function, FunctionDeclaration, Program, ReturnStatement},
        CodePathAnalyzerInstanceProviderFactory,
    };

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_consistent_return_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            consistent_return_rule(),
            rule_tests! {
                valid => [
                    "function foo() { return; }",
                    "function foo() { if (true) return; }",
                    "function foo() { if (true) return; else return; }",
                    "function foo() { if (true) return true; else return false; }",
                    "f(function() { return; })",
                    "f(function() { if (true) return; })",
                    "f(function() { if (true) return; else return; })",
                    "f(function() { if (true) return true; else return false; })",
                    "function foo() { function bar() { return true; } return; }",
                    "function foo() { function bar() { return; } return false; }",
                    "function Foo() { if (!(this instanceof Foo)) return new Foo(); }",
                    { code => "function foo() { if (true) return; else return undefined; }", options => { treat_undefined_as_unspecified => true } },
                    { code => "function foo() { if (true) return; else return void 0; }", options => { treat_undefined_as_unspecified => true } },
                    { code => "function foo() { if (true) return undefined; else return; }", options => { treat_undefined_as_unspecified => true } },
                    { code => "function foo() { if (true) return undefined; else return void 0; }", options => { treat_undefined_as_unspecified => true } },
                    { code => "function foo() { if (true) return void 0; else return; }", options => { treat_undefined_as_unspecified => true } },
                    { code => "function foo() { if (true) return void 0; else return undefined; }", options => { treat_undefined_as_unspecified => true } },
                    { code => "var x = () => {  return {}; };", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "if (true) { return 1; } return 0;", /*parserOptions: { ecmaVersion: 6, ecmaFeatures: { globalReturn: true } }*/ },

                    // https://github.com/eslint/eslint/issues/7790
                    { code => "class Foo { constructor() { if (true) return foo; } }", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var Foo = class { constructor() { if (true) return foo; } }", /*parserOptions: { ecmaVersion: 6 }*/ }
                ],
                invalid => [
                    {
                        code => "function foo() { if (true) return true; else return; }",
                        errors => [
                            {
                                message_id => "missing_return_value",
                                data => { name => "Function 'foo'" },
                                type => ReturnStatement,
                                line => 1,
                                column => 46,
                                end_line => 1,
                                end_column => 53
                            }
                        ]
                    },
                    {
                        code => "var foo = () => { if (true) return true; else return; }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_return_value",
                                data => { name => "Arrow function" },
                                type => ReturnStatement,
                                line => 1,
                                column => 47,
                                end_line => 1,
                                end_column => 54
                            }
                        ]
                    },
                    {
                        code => "function foo() { if (true) return; else return false; }",
                        errors => [
                            {
                                message_id => "unexpected_return_value",
                                data => { name => "Function 'foo'" },
                                type => ReturnStatement,
                                line => 1,
                                column => 41,
                                end_line => 1,
                                end_column => 54
                            }
                        ]
                    },
                    {
                        code => "f(function() { if (true) return true; else return; })",
                        errors => [
                            {
                                message_id => "missing_return_value",
                                data => { name => "Function" },
                                type => ReturnStatement,
                                line => 1,
                                column => 44,
                                end_line => 1,
                                end_column => 51
                            }
                        ]
                    },
                    {
                        code => "f(function() { if (true) return; else return false; })",
                        errors => [
                            {
                                message_id => "unexpected_return_value",
                                data => { name => "Function" },
                                type => ReturnStatement,
                                line => 1,
                                column => 39,
                                end_line => 1,
                                end_column => 52
                            }
                        ]
                    },
                    {
                        code => "f(a => { if (true) return; else return false; })",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "unexpected_return_value",
                                data => { name => "Arrow function" },
                                type => ReturnStatement,
                                line => 1,
                                column => 33,
                                end_line => 1,
                                end_column => 46
                            }
                        ]
                    },
                    {
                        code => "function foo() { if (true) return true; return undefined; }",
                        options => { treat_undefined_as_unspecified => true },
                        errors => [
                            {
                                message_id => "missing_return_value",
                                data => { name => "Function 'foo'" },
                                type => ReturnStatement,
                                line => 1,
                                column => 41,
                                end_line => 1,
                                end_column => 58
                            }
                        ]
                    },
                    {
                        code => "function foo() { if (true) return true; return void 0; }",
                        options => { treat_undefined_as_unspecified => true },
                        errors => [
                            {
                                message_id => "missing_return_value",
                                data => { name => "Function 'foo'" },
                                type => ReturnStatement,
                                line => 1,
                                column => 41,
                                end_line => 1,
                                end_column => 55
                            }
                        ]
                    },
                    {
                        code => "function foo() { if (true) return undefined; return true; }",
                        options => { treat_undefined_as_unspecified => true },
                        errors => [
                            {
                                message_id => "unexpected_return_value",
                                data => { name => "Function 'foo'" },
                                type => ReturnStatement,
                                line => 1,
                                column => 46,
                                end_line => 1,
                                end_column => 58
                            }
                        ]
                    },
                    {
                        code => "function foo() { if (true) return void 0; return true; }",
                        options => { treat_undefined_as_unspecified => true },
                        errors => [
                            {
                                message_id => "unexpected_return_value",
                                data => { name => "Function 'foo'" },
                                type => ReturnStatement,
                                line => 1,
                                column => 43,
                                end_line => 1,
                                end_column => 55
                            }
                        ]
                    },
                    {
                        code => "if (true) { return 1; } return;",
                        // parserOptions: { ecmaFeatures: { globalReturn: true } },
                        errors => [
                            {
                                message_id => "missing_return_value",
                                data => { name => "Program" },
                                type => ReturnStatement,
                                line => 1,
                                column => 25,
                                end_line => 1,
                                end_column => 32
                            }
                        ]
                    },
                    {
                        code => "function foo() { if (a) return true; }",
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "function 'foo'" },
                                type => FunctionDeclaration,
                                line => 1,
                                column => 10,
                                end_line => 1,
                                end_column => 13
                            }
                        ]
                    },
                    {
                        code => "function _foo() { if (a) return true; }",
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "function '_foo'" },
                                type => FunctionDeclaration,
                                line => 1,
                                column => 10,
                                end_line => 1,
                                end_column => 14
                            }
                        ]
                    },
                    {
                        code => "f(function foo() { if (a) return true; });",
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "function 'foo'" },
                                type => Function,
                                line => 1,
                                column => 12,
                                end_line => 1,
                                end_column => 15
                            }
                        ]
                    },
                    {
                        code => "f(function() { if (a) return true; });",
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "function" },
                                type => Function,
                                line => 1,
                                column => 3,
                                end_line => 1,
                                end_column => 11
                            }
                        ]
                    },
                    {
                        code => "f(() => { if (a) return true; });",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "arrow function" },
                                type => ArrowFunction,
                                line => 1,
                                column => 6,
                                end_line => 1,
                                end_column => 8
                            }
                        ]
                    },
                    {
                        code => "var obj = {foo() { if (a) return true; }};",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "method 'foo'" },
                                type => MethodDefinition,
                                line => 1,
                                column => 12,
                                end_line => 1,
                                end_column => 15
                            }
                        ]
                    },
                    {
                        code => "class A {foo() { if (a) return true; }};",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "method 'foo'" },
                                type => MethodDefinition,
                                line => 1,
                                column => 10,
                                end_line => 1,
                                end_column => 13
                            }
                        ]
                    },
                    {
                        code => "if (a) return true;",
                        // parserOptions: { ecmaFeatures: { globalReturn: true } },
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "program" },
                                type => Program,
                                line => 1,
                                column => 1,
                                // end_line => void 0,
                                // end_column => void 0
                            }
                        ]
                    },
                    {
                        code => "class A { CapitalizedFunction() { if (a) return true; } }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "method 'CapitalizedFunction'" },
                                type => MethodDefinition,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 30
                            }
                        ]
                    },
                    {
                        code => "({ constructor() { if (a) return true; } });",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_return",
                                data => { name => "method 'constructor'" },
                                type => MethodDefinition,
                                line => 1,
                                column => 4,
                                end_line => 1,
                                end_column => 15
                            }
                        ]
                    }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
