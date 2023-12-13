use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

use crate::scope::ScopeManager;

pub fn no_obj_calls_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-obj-calls",
        languages => [Javascript],
        messages => [
            unexpected_call => "'{{name}}' is not a function.",
            // TODO: support ref keyword here?
            unexpected_ref_call => "'{{name}}' is reference to '{{ref_}}', which is not a function.",
        ],
        listeners => [
            r#"
              (program) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let scope = scope_manager.get_scope(node);

                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::{CallExpression, NewExpression};

    #[test]
    fn test_no_obj_calls_rule() {
        RuleTester::run(
            no_obj_calls_rule(),
            rule_tests! {
                valid => [
                    "var x = Math;",
                    "var x = Math.random();",
                    "var x = Math.PI;",
                    "var x = foo.Math();",
                    "var x = new foo.Math();",
                    "var x = new Math.foo;",
                    "var x = new Math.foo();",
                    "JSON.parse(foo)",
                    "new JSON.parse",
                    {
                        code => "Reflect.get(foo, 'x')",
                        environment => {
                            env => { es6 => true }
                        },
                    },
                    {
                        code => "new Reflect.foo(a, b)",
                        environment => {
                            env => { es6 => true }
                        }
                    },
                    {
                        code => "Atomics.load(foo, 0)",
                        environment => {
                            env => { es2017 => true }
                        }
                    },
                    {
                        code => "new Atomics.foo()",
                        environment => {
                            env => { es2017 => true }
                        }
                    },
                    {
                        code => "new Intl.Segmenter()",
                        environment => {
                            env => { browser => true }
                        }
                    },
                    {
                        code => "Intl.foo()",
                        environment => {
                            env => { browser => true }
                        }
                    },

                    { code => "globalThis.Math();", environment => { env => { es6 => true } } },
                    { code => "var x = globalThis.Math();", environment => { env => { es6 => true } } },
                    { code => "f(globalThis.Math());", environment => { env => { es6 => true } } },
                    { code => "globalThis.Math().foo;", environment => { env => { es6 => true } } },
                    { code => "var x = globalThis.JSON();", environment => { env => { es6 => true } } },
                    { code => "x = globalThis.JSON(str);", environment => { env => { es6 => true } } },
                    { code => "globalThis.Math( globalThis.JSON() );", environment => { env => { es6 => true } } },
                    { code => "var x = globalThis.Reflect();", environment => { env => { es6 => true } } },
                    { code => "var x = globalThis.Reflect();", environment => { env => { es2017 => true } } },
                    { code => "/*globals Reflect: true*/ globalThis.Reflect();", environment => { env => { es2017 => true } } },
                    { code => "var x = globalThis.Atomics();", environment => { env => { es2017 => true } } },
                    { code => "var x = globalThis.Atomics();", environment => { globals => { Atomics => false }, env => { es2017 => true } } },
                    { code => "var x = globalThis.Intl();", environment => { env => { browser => true } } },
                    { code => "var x = globalThis.Intl();", environment => { globals => { Intl => false }, env => { browser => true } } },

                    // non-existing variables
                    "/*globals Math: off*/ Math();",
                    "/*globals Math: off*/ new Math();",
                    {
                        code => "JSON();",
                        environment => {
                            globals => { JSON => "off" }
                        }
                    },
                    {
                        code => "new JSON();",
                        environment => {
                            globals => { JSON => "off" }
                        }
                    },
                    "Reflect();",
                    "Atomics();",
                    "new Reflect();",
                    "new Atomics();",
                    {
                        code => "Atomics();",
                        environment => {
                            env => { es6 => true }
                        }
                    },
                    "Intl()",
                    "new Intl()",

                    // shadowed variables
                    "var Math; Math();",
                    "var Math; new Math();",
                    {
                        code => "let JSON; JSON();",
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "let JSON; new JSON();",
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "if (foo) { const Reflect = 1; Reflect(); }",
                        environment => {
                            ecma_version => 2015,
                            env => { es6 => true }
                        },
                    },
                    {
                        code => "if (foo) { const Reflect = 1; new Reflect(); }",
                        environment => {
                            ecma_version => 2015,
                            env => { es6 => true }
                        },
                    },
                    "function foo(Math) { Math(); }",
                    "function foo(JSON) { new JSON(); }",
                    {
                        code => "function foo(Atomics) { Atomics(); }",
                        environment => {
                            env => { es2017 => true }
                        }
                    },
                    {
                        code => "function foo() { if (bar) { let Atomics; if (baz) { new Atomics(); } } }",
                        environment => {
                            ecma_version => 2015,
                            env => { es2017 => true }
                        },
                    },
                    "function foo() { var JSON; JSON(); }",
                    {
                        code => "function foo() { var Atomics = bar(); var baz = Atomics(5); }",
                        environment => {
                            globals => { Atomics => false }
                        }
                    },
                    {
                        code => "var construct = typeof Reflect !== \"undefined\" ? Reflect.construct : undefined; construct();",
                        environment => {
                            globals => { Reflect => false }
                        }
                    },
                    {
                        code => "function foo(Intl) { Intl(); }",
                        environment => {
                            env => { browser => true }
                        }
                    },
                    {
                        code => "if (foo) { const Intl = 1; Intl(); }",
                        environment => {
                            ecma_version => 2015,
                            env => { browser => true }
                        }
                    },
                    {
                        code => "if (foo) { const Intl = 1; new Intl(); }",
                        environment => {
                            ecma_version => 2015,
                            env => { browser => true }
                        }
                    }
                ],
                invalid => [
                    {
                        code => "Math();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression }]
                    },
                    {
                        code => "var x = Math();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression }]
                    },
                    {
                        code => "f(Math());",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression, column => 3, end_column => 9 }]
                    },
                    {
                        code => "Math().foo;",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression, column => 1, end_column => 7 }]
                    },
                    {
                        code => "new Math;",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => NewExpression }]
                    },
                    {
                        code => "new Math();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => NewExpression }]
                    },
                    {
                        code => "new Math(foo);",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => NewExpression }]
                    },
                    {
                        code => "new Math().foo;",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => NewExpression }]
                    },
                    {
                        code => "(new Math).foo();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => NewExpression }]
                    },
                    {
                        code => "var x = JSON();",
                        errors => [{ message_id => "unexpected_call", data => { name => "JSON" }, type => CallExpression }]
                    },
                    {
                        code => "x = JSON(str);",
                        errors => [{ message_id => "unexpected_call", data => { name => "JSON" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new JSON();",
                        errors => [{ message_id => "unexpected_call", data => { name => "JSON" }, type => NewExpression }]
                    },
                    {
                        code => "Math( JSON() );",
                        errors => [
                            { message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression, column => 1, end_column => 15 },
                            { message_id => "unexpected_call", data => { name => "JSON" }, type => CallExpression, column => 7, end_column => 13 }
                        ]
                    },
                    {
                        code => "var x = Reflect();",
                        environment => { env => { es6 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new Reflect();",
                        environment => { env => { es6 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => NewExpression }]
                    },
                    {
                        code => "var x = Reflect();",
                        environment => { env => { es2017 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => CallExpression }]
                    },
                    {
                        code => "/*globals Reflect: true*/ Reflect();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => CallExpression }]
                    },
                    {
                        code => "/*globals Reflect: true*/ new Reflect();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => NewExpression }]
                    },
                    {
                        code => "var x = Atomics();",
                        environment => { env => { es2017 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Atomics" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new Atomics();",
                        environment => { env => { es2017 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Atomics" }, type => NewExpression }]
                    },
                    {
                        code => "var x = Atomics();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Atomics" }, type => CallExpression }]
                    },
                    {
                        code => "var x = Atomics();",
                        environment => { globals => { Atomics => false } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Atomics" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new Atomics();",
                        environment => { globals => { Atomics => "writable" } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Atomics" }, type => NewExpression }]
                    },
                    {
                        code => "var x = Intl();",
                        environment => { env => { browser => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Intl" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new Intl();",
                        environment => { env => { browser => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Intl" }, type => NewExpression }]
                    },
                    {
                        code => "/*globals Intl: true*/ Intl();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Intl" }, type => CallExpression }]
                    },
                    {
                        code => "/*globals Intl: true*/ new Intl();",
                        errors => [{ message_id => "unexpected_call", data => { name => "Intl" }, type => NewExpression }]
                    },
                    {
                        code => "var x = globalThis.Math();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new globalThis.Math();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => NewExpression }]
                    },
                    {
                        code => "f(globalThis.Math());",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression, column => 3, end_column => 20 }]
                    },
                    {
                        code => "globalThis.Math().foo;",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression, column => 1, end_column => 18 }]
                    },
                    {
                        code => "new globalThis.Math().foo;",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Math" }, type => NewExpression, column => 1, end_column => 22 }]
                    },
                    {
                        code => "var x = globalThis.JSON();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "JSON" }, type => CallExpression }]
                    },
                    {
                        code => "x = globalThis.JSON(str);",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "JSON" }, type => CallExpression }]
                    },
                    {
                        code => "globalThis.Math( globalThis.JSON() );",
                        environment => { env => { es2020 => true } },
                        errors => [
                            { message_id => "unexpected_call", data => { name => "Math" }, type => CallExpression, column => 1, end_column => 37 },
                            { message_id => "unexpected_call", data => { name => "JSON" }, type => CallExpression, column => 18, end_column => 35 }
                        ]
                    },
                    {
                        code => "var x = globalThis.Reflect();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new globalThis.Reflect;",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => NewExpression }]
                    },
                    {
                        code => "/*globals Reflect: true*/ Reflect();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => CallExpression }]
                    },
                    {
                        code => "var x = globalThis.Atomics();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Atomics" }, type => CallExpression }]
                    },
                    {
                        code => "var x = globalThis.Intl();",
                        environment => { env => { browser => true, es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Intl" }, type => CallExpression }]
                    },
                    {
                        code => "var x = new globalThis.Intl;",
                        environment => { env => { browser => true, es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Intl" }, type => NewExpression }]
                    },
                    {
                        code => "/*globals Intl: true*/ Intl();",
                        environment => { env => { browser => true, es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Intl" }, type => CallExpression }]
                    },
                    {
                        code => "var foo = bar ? baz: JSON; foo();",
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "JSON" }, type => CallExpression }]
                    },
                    {
                        code => "var foo = bar ? baz: JSON; new foo();",
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "JSON" }, type => NewExpression }]
                    },
                    {
                        code => "var foo = bar ? baz: globalThis.JSON; foo();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "JSON" }, type => CallExpression }]
                    },
                    {
                        code => "var foo = bar ? baz: globalThis.JSON; new foo();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "JSON" }, type => NewExpression }]
                    },
                    {
                        code => "var foo = window.Atomics; foo();",
                        environment => { env => { es2020 => true, browser => true } },
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "Atomics" }, type => CallExpression }]
                    },
                    {
                        code => "var foo = window.Atomics; new foo;",
                        environment => { env => { es2020 => true, browser => true } },
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "Atomics" }, type => NewExpression }]
                    },
                    {
                        code => "var foo = window.Intl; foo();",
                        environment => { env => { es2020 => true, browser => true } },
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "Intl" }, type => CallExpression }]
                    },
                    {
                        code => "var foo = window.Intl; new foo;",
                        environment => { env => { es2020 => true, browser => true } },
                        errors => [{ message_id => "unexpectedRefCall", data => { name => "foo", ref_ => "Intl" }, type => NewExpression }]
                    },

                    // Optional chaining
                    {
                        code => "var x = globalThis?.Reflect();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => CallExpression }]
                    },
                    {
                        code => "var x = (globalThis?.Reflect)();",
                        environment => { env => { es2020 => true } },
                        errors => [{ message_id => "unexpected_call", data => { name => "Reflect" }, type => CallExpression }]
                    }
                ]
            },
        )
    }
}
