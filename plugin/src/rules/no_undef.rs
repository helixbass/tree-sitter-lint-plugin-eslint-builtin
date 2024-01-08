use std::sync::Arc;

use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{ast_helpers::NodeExtJs, kind::UnaryExpression, scope::ScopeManager};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    #[serde(alias = "typeof")]
    typeof_: bool,
}

fn has_type_of_operator<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
    node.maybe_next_non_parentheses_ancestor(context)
        .matches(|parent| {
            parent.kind() == UnaryExpression && parent.field("operator").kind() == "typeof"
        })
}

pub fn no_undef_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-undef",
        languages => [Javascript],
        messages => [
            undef => "'{{name}}' is not defined.",
        ],
        options_type => Options,
        state => {
            [per-config]
            consider_typeof: bool = options.typeof_,
        },
        listeners => [
            "program:exit" => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();

                let global_scope = scope_manager.get_scope(node);

                global_scope.through().for_each(|ref_| {
                    let identifier = ref_.identifier();

                    if !self.consider_typeof && has_type_of_operator(identifier, context) {
                        return;
                    }

                    context.report(violation! {
                        node => identifier,
                        message_id => "undef",
                        data => {
                            name => identifier.text(context),
                        },
                    });
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{get_instance_provider_factory, kind::Identifier};

    #[test]
    fn test_no_undef_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_undef_rule(),
            rule_tests! {
                valid => [
                    "var a = 1, b = 2; a;",
                    "/*global b*/ function f() { b; }",
                    { code => "function f() { b; }", environment => { globals => { b => false } } },
                    "/*global b a:false*/  a;  function f() { b; a; }",
                    "function a(){}  a();",
                    "function f(b) { b; }",
                    "var a; a = 1; a++;",
                    "var a; function f() { a = 1; }",
                    "/*global b:true*/ b++;",
                    // TODO: support these?
                    // "/*eslint-env browser*/ window;",
                    // "/*eslint-env node*/ require(\"a\");",
                    "Object; isNaN();",
                    "toString()",
                    "hasOwnProperty()",
                    "function evilEval(stuffToEval) { var ultimateAnswer; ultimateAnswer = 42; eval(stuffToEval); }",
                    "typeof a",
                    "typeof (a)",
                    "var b = typeof a",
                    "typeof a === 'undefined'",
                    "if (typeof a === 'undefined') {}",
                    { code => "function foo() { var [a, b=4] = [1, 2]; return {a, b}; }", environment => { ecma_version => 6 } },
                    { code => "var toString = 1;", environment => { ecma_version => 6 } },
                    { code => "function myFunc(...foo) {  return foo;}", environment => { ecma_version => 6 } },
                    { code => "var React, App, a=1; React.render(<App attr={a} />);", environment => { ecma_version => 6/*, ecmaFeatures: { jsx: true }*/ } },
                    { code => "var console; [1,2,3].forEach(obj => {\n  console.log(obj);\n});", environment => { ecma_version => 6 } },
                    { code => "var Foo; class Bar extends Foo { constructor() { super();  }}", environment => { ecma_version => 6 } },
                    { code => "import Warning from '../lib/warning'; var warn = new Warning('text');", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "import * as Warning from '../lib/warning'; var warn = new Warning('text');", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "var a; [a] = [0];", environment => { ecma_version => 6 } },
                    { code => "var a; ({a} = {});", environment => { ecma_version => 6 } },
                    { code => "var a; ({b: a} = {});", environment => { ecma_version => 6 } },
                    { code => "var obj; [obj.a, obj.b] = [0, 1];", environment => { ecma_version => 6 } },
                    { code => "URLSearchParams;", environment => { env => { browser => true } } },
                    { code => "Intl;", environment => { env => { browser => true } } },
                    { code => "IntersectionObserver;", environment => { env => { browser => true } } },
                    { code => "Credential;", environment => { env => { browser => true } } },
                    { code => "requestIdleCallback;", environment => { env => { browser => true } } },
                    { code => "customElements;", environment => { env => { browser => true } } },
                    { code => "PromiseRejectionEvent;", environment => { env => { browser => true } } },
                    { code => "(foo, bar) => { foo ||= WeakRef; bar ??= FinalizationRegistry; }", environment => { env => { es2021 => true } } },

                    // Notifications of readonly are removed: https://github.com/eslint/eslint/issues/4504
                    "/*global b:false*/ function f() { b = 1; }",
                    { code => "function f() { b = 1; }", environment => { globals => { b => false } } },
                    "/*global b:false*/ function f() { b++; }",
                    "/*global b*/ b = 1;",
                    "/*global b:false*/ var b = 1;",
                    "Array = 1;",

                    // new.target: https://github.com/eslint/eslint/issues/5420
                    { code => "class A { constructor() { new.target; } }", environment => { ecma_version => 6 } },

                    // Rest property
                    {
                        code => "var {bacon, ...others} = stuff; foo(others)",
                        environment => {
                            ecma_version => 2018,
                            globals => { stuff => false, foo => false }
                        },
                    },

                    // export * as ns from "source"
                    {
                        code => r#"export * as ns from "source""#,
                        environment => { ecma_version => 2020, source_type => "module" }
                    },

                    // import.meta
                    {
                        code => "import.meta",
                        environment => { ecma_version => 2020, source_type => "module" }
                    },

                    // class static blocks
                    {
                        code => "let a; class C { static {} } a;",
                        environment => { ecma_version => 2022 },
                        // TODO: looks like I assume a mistake that "valid" tests have "errors"
                        // key in the ESLint version of these tests, upstream?
                        // errors => [{ message_id => "undef", data => { name => "a" } }]
                    },
                    {
                        code => "var a; class C { static {} } a;",
                        environment => { ecma_version => 2022 },
                        // errors => [{ message_id => "undef", data => { name => "a" } }]
                    },
                    {
                        code => "a; class C { static {} } var a;",
                        environment => { ecma_version => 2022 },
                        // errors => [{ message_id => "undef", data => { name => "a" } }]
                    },
                    {
                        code => "class C { static { C; } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "const C = class { static { C; } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { a; } } var a;",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { a; } } let a;",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { var a; a; } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { a; var a; } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { a; { var a; } } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { let a; a; } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { a; let a; } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { function a() {} a; } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    },
                    {
                        code => "class C { static { a; function a() {} } }",
                        environment => { ecma_version => 2022, source_type => "module" }
                    }
                ],
                invalid => [
                    { code => "a = 1;", errors => [{ message_id => "undef", data => { name => "a" }, type => Identifier }] },
                    // TODO: make macro support `typeof` keyword here?
                    { code => "if (typeof anUndefinedVar === 'string') {}", options => { /*typeof*/ typeof_ => true }, errors => [{ message_id => "undef", data => { name => "anUndefinedVar" }, type => Identifier }] },
                    { code => "var a = b;", errors => [{ message_id => "undef", data => { name => "b" }, type => Identifier }] },
                    { code => "function f() { b; }", errors => [{ message_id => "undef", data => { name => "b" }, type => Identifier }] },
                    { code => "window;", errors => [{ message_id => "undef", data => { name => "window" }, type => Identifier }] },
                    { code => "require(\"a\");", errors => [{ message_id => "undef", data => { name => "require" }, type => Identifier }] },
                    { code => "var React; React.render(<img attr={a} />);", environment => { ecma_version => 6, /*ecmaFeatures: { jsx: true }*/ }, errors => [{ message_id => "undef", data => { name => "a" } }] },
                    { code => "var React, App; React.render(<App attr={a} />);", environment => { ecma_version => 6, /*ecmaFeatures: { jsx: true }*/ }, errors => [{ message_id => "undef", data => { name => "a" } }] },
                    { code => "[a] = [0];", environment => { ecma_version => 6 }, errors => [{ message_id => "undef", data => { name => "a" } }] },
                    { code => "({a} = {});", environment => { ecma_version => 6 }, errors => [{ message_id => "undef", data => { name => "a" } }] },
                    { code => "({b: a} = {});", environment => { ecma_version => 6 }, errors => [{ message_id => "undef", data => { name => "a" } }] },
                    { code => "[obj.a, obj.b] = [0, 1];", environment => { ecma_version => 6 }, errors => [{ message_id => "undef", data => { name => "obj" } }, { message_id => "undef", data => { name => "obj" } }] },

                    // Experimental
                    {
                        code => "const c = 0; const a = {...b, c};",
                        environment => {
                            ecma_version => 2018
                        },
                        errors => [{ message_id => "undef", data => { name => "b" } }]
                    },

                    // class static blocks
                    {
                        code => "class C { static { a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" } }]
                    },
                    {
                        code => "class C { static { { let a; } a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 31 }]
                    },
                    {
                        code => "class C { static { { function a() {} } a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 40 }]
                    },
                    {
                        code => "class C { static { function foo() { var a; }  a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 47 }]
                    },
                    {
                        code => "class C { static { var a; } static { a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 38 }]
                    },
                    {
                        code => "class C { static { let a; } static { a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 38 }]
                    },
                    {
                        code => "class C { static { function a(){} } static { a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 46 }]
                    },
                    {
                        code => "class C { static { var a; } foo() { a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 37 }]
                    },
                    {
                        code => "class C { static { let a; } foo() { a; } }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 37 }]
                    },
                    {
                        code => "class C { static { var a; } [a]; }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 30 }]
                    },
                    {
                        code => "class C { static { let a; } [a]; }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 30 }]
                    },
                    {
                        code => "class C { static { function a() {} } [a]; }",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 39 }]
                    },
                    {
                        code => "class C { static { var a; } } a;",
                        environment => {
                            ecma_version => 2022
                        },
                        errors => [{ message_id => "undef", data => { name => "a" }, column => 31 }]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
