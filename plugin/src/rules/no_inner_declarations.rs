use std::{collections::HashSet, sync::Arc};

use once_cell::sync::Lazy;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, Rule};

use crate::{
    kind::{
        ArrowFunction, ClassStaticBlock, ExportStatement, Function, FunctionDeclaration,
        GeneratorFunction, GeneratorFunctionDeclaration, Kind, MethodDefinition, Program,
        StatementBlock, VariableDeclaration,
    },
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Options {
    #[default]
    Functions,
    Both,
}

static VALID_PARENT: Lazy<HashSet<Kind>> =
    Lazy::new(|| [Program, ExportStatement].into_iter().collect());

static VALID_BLOCK_STATEMENT_PARENT: Lazy<HashSet<Kind>> = Lazy::new(|| {
    [
        FunctionDeclaration,
        Function,
        ArrowFunction,
        MethodDefinition,
        GeneratorFunction,
        GeneratorFunctionDeclaration,
        ClassStaticBlock,
    ]
    .into_iter()
    .collect()
});

fn get_allowed_body_description(mut node: Node) -> &'static str {
    while let Some(parent) = node.parent() {
        if parent.kind() == ClassStaticBlock {
            return "class static block body";
        }

        if ast_utils::is_function(parent) {
            return "function body";
        }

        node = parent;
    }

    "program"
}

pub fn no_inner_declarations_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-inner-declarations",
        languages => [Javascript],
        messages => [
            move_decl_to_root => "Move {{type}} declaration to {{body}} root.",
        ],
        options_type => Options,
        state => {
            [per-config]
            both: bool = options == Options::Both,
        },
        listeners => [
            r#"
              (function_declaration) @c
              (variable_declaration) @c
            "# => |node, context| {
                if !self.both && node.kind() == VariableDeclaration {
                    return;
                }

                let parent = node.parent().unwrap();

                if parent.kind() == StatementBlock && VALID_BLOCK_STATEMENT_PARENT.contains(&parent.parent().unwrap().kind()) {
                    return;
                }

                if VALID_PARENT.contains(&parent.kind()) {
                    return;
                }

                context.report(violation! {
                    node => node,
                    message_id => "move_decl_to_root",
                    data => {
                        type => match node.kind() {
                            FunctionDeclaration => "function",
                            _ => "variable",
                        },
                        body => get_allowed_body_description(node),
                    }
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_no_inner_declarations_rule() {
        RuleTester::run(
            no_inner_declarations_rule(),
            rule_tests! {
                // Examples of code that should not trigger the rule
                valid => [
                    "function doSomething() { }",
                    "function doSomething() { function somethingElse() { } }",
                    "(function() { function doSomething() { } }());",
                    "if (test) { var fn = function() { }; }",
                    "if (test) { var fn = function expr() { }; }",
                    "function decl() { var fn = function expr() { }; }",
                    "function decl(arg) { var fn; if (arg) { fn = function() { }; } }",
                    { code => "var x = {doSomething() {function doSomethingElse() {}}}", environment => { ecma_version => 6 } },
                    { code => "function decl(arg) { var fn; if (arg) { fn = function expr() { }; } }", environment => { ecma_version => 6 } },
                    "function decl(arg) { var fn; if (arg) { fn = function expr() { }; } }",
                    "if (test) { var foo; }",
                    { code => "if (test) { let x = 1; }", options => "both", environment => { ecma_version => 6 } },
                    { code => "if (test) { const x = 1; }", options => "both", environment => { ecma_version => 6 } },
                    "function doSomething() { while (test) { var foo; } }",
                    { code => "var foo;", options => "both" },
                    { code => "var foo = 42;", options => "both" },
                    { code => "function doSomething() { var foo; }", options => "both" },
                    { code => "(function() { var foo; }());", options => "both" },
                    { code => "foo(() => { function bar() { } });", environment => { ecma_version => 6 } },
                    { code => "var fn = () => {var foo;}", options => "both", environment => { ecma_version => 6 } },
                    {
                        code => "var x = {doSomething() {var foo;}}",
                        options => "both",
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "export var foo;",
                        options => "both",
                        environment => { source_type => "module", ecma_version => 6 }
                    },
                    {
                        code => "export function bar() {}",
                        options => "both",
                        environment => { source_type => "module", ecma_version => 6 }
                    },
                    {
                        code => "export default function baz() {}",
                        options => "both",
                        environment => { source_type => "module", ecma_version => 6 }
                    },
                    {
                        code => "exports.foo = () => {}",
                        options => "both",
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "exports.foo = function(){}",
                        options => "both"
                    },
                    {
                        code => "module.exports = function foo(){}",
                        options => "both"
                    },
                    {
                        code => "class C { method() { function foo() {} } }",
                        options => "both",
                        environment => { ecma_version => 2022 }
                    },
                    {
                        code => "class C { method() { var x; } }",
                        options => "both",
                        environment => { ecma_version => 2022 }
                    },
                    {
                        code => "class C { static { function foo() {} } }",
                        options => "both",
                        environment => { ecma_version => 2022 }
                    },
                    {
                        code => "class C { static { var x; } }",
                        options => "both",
                        environment => { ecma_version => 2022 }
                    }
                ],
                // Examples of code that should trigger the rule
                invalid => [
                    {
                        code => "if (test) { function doSomething() { } }",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "program"
                            },
                            type => FunctionDeclaration
                        }]
                    }, {
                        code => "if (foo) var a; ",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "program"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "if (foo) /* some comments */ var a; ",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "program"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "if (foo){ function f(){ if(bar){ var a; } } }",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "program"
                            },
                            type => FunctionDeclaration
                        }, {
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "function body"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "if (foo) function f(){ if(bar) var a; } ",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "program"
                            },
                            type => FunctionDeclaration
                        }, {
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "function body"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "if (foo) { var fn = function(){} } ",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "program"
                            },
                            type => VariableDeclaration
                        }]
                    },
                    {
                        code => "if (foo)  function f(){} ",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "program"
                            },
                            type => FunctionDeclaration
                        }]
                    },
                    {
                        code => "function bar() { if (foo) function f(){}; }",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "function body"
                            },
                            type => FunctionDeclaration
                        }]
                    },
                    {
                        code => "function bar() { if (foo) var a; }",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "function body"
                            },
                            type => VariableDeclaration
                        }]
                    },
                    {
                        code => "if (foo){ var a; }",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "program"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "function doSomething() { do { function somethingElse() { } } while (test); }",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "function body"
                            },
                            type => FunctionDeclaration
                        }]
                    }, {
                        code => "(function() { if (test) { function doSomething() { } } }());",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "function body"
                            },
                            type => FunctionDeclaration
                        }]
                    }, {
                        code => "while (test) { var foo; }",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "program"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "function doSomething() { if (test) { var foo = 42; } }",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "function body"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "(function() { if (test) { var foo; } }());",
                        options => "both",
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "function body"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "const doSomething = () => { if (test) { var foo = 42; } }",
                        options => "both",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "function body"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "class C { method() { if(test) { var foo; } } }",
                        options => "both",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "function body"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "class C { static { if (test) { function foo() {} } } }",
                        options => "both",
                        environment => { ecma_version => 2022 },
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "function",
                                body => "class static block body"
                            },
                            type => FunctionDeclaration
                        }]
                    }, {
                        code => "class C { static { if (test) { var foo; } } }",
                        options => "both",
                        environment => { ecma_version => 2022 },
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "class static block body"
                            },
                            type => VariableDeclaration
                        }]
                    }, {
                        code => "class C { static { if (test) { if (anotherTest) { var foo; } } } }",
                        options => "both",
                        environment => { ecma_version => 2022 },
                        errors => [{
                            message_id => "move_decl_to_root",
                            data => {
                                type => "variable",
                                body => "class static block body"
                            },
                            type => VariableDeclaration
                        }]
                    }
                ]
            },
        )
    }
}
