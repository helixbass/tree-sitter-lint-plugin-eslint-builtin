use std::{collections::HashSet, sync::Arc};

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    except_methods: Option<Vec<String>>,
    enforce_for_class_fields: Option<bool>,
}

impl Options {
    fn enforce_for_class_fields(&self) -> bool {
        self.enforce_for_class_fields.unwrap_or(true)
    }
}

pub fn class_methods_use_this_rule() -> Arc<dyn Rule> {
    rule! {
        name => "class-methods-use-this",
        languages => [Javascript],
        messages => [
            missing_this => "Expected 'this' to be used by class {{name}}.",
        ],
        options_type => Options,
        state => {
            [per-config]
            enforce_for_class_fields: bool = options.enforce_for_class_fields(),
            except_methods: HashSet<String> = options.except_methods.clone().unwrap_or_default().into_iter().collect(),
            [per-file-run]
            stack: Vec<bool>,
        },
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
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
    use crate::kind::Function;

    #[test]
    fn test_class_methods_use_this_rule() {
        RuleTester::run(
            class_methods_use_this_rule(),
            rule_tests! {
                valid => [
                    { code => "class A { constructor() {} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {this} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {this.bar = 'bar';} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {bar(this);} }", environment => { ecma_version => 6 } },
                    { code => "class A extends B { foo() {super.foo();} }", environment => { ecma_version => 6 } },
                    { code => "class A { foo() { if(true) { return this; } } }", environment => { ecma_version => 6 } },
                    { code => "class A { static foo() {} }", environment => { ecma_version => 6 } },
                    { code => "({ a(){} });", environment => { ecma_version => 6 } },
                    { code => "class A { foo() { () => this; } }", environment => { ecma_version => 6 } },
                    { code => "({ a: function () {} });", environment => { ecma_version => 6 } },
                    { code => "class A { foo() {this} bar() {} }", options => { except_methods => ["bar"] }, environment => { ecma_version => 6 } },
                    { code => "class A { \"foo\"() { } }", options => { except_methods => ["foo"] }, environment => { ecma_version => 6 } },
                    { code => "class A { 42() { } }", options => { except_methods => ["42"] }, environment => { ecma_version => 6 } },
                    { code => "class A { foo = function() {this} }", environment => { ecma_version => 2022 } },
                    { code => "class A { foo = () => {this} }", environment => { ecma_version => 2022 } },
                    { code => "class A { foo = () => {super.toString} }", environment => { ecma_version => 2022 } },
                    { code => "class A { static foo = function() {} }", environment => { ecma_version => 2022 } },
                    { code => "class A { static foo = () => {} }", environment => { ecma_version => 2022 } },
                    { code => "class A { #bar() {} }", options => { except_methods => ["#bar"] }, environment => { ecma_version => 2022 } },
                    { code => "class A { foo = function () {} }", options => { enforce_for_class_fields => false }, environment => { ecma_version => 2022 } },
                    { code => "class A { foo = () => {} }", options => { enforce_for_class_fields => false }, environment => { ecma_version => 2022 } },
                    { code => "class A { foo() { return class { [this.foo] = 1 }; } }", environment => { ecma_version => 2022 } },
                    { code => "class A { static {} }", environment => { ecma_version => 2022 } }
                ],
                invalid => [
                    {
                        code => "class A { foo() {} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {/**this**/} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {var a = function () {this};} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {var a = function () {var b = function(){this}};} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {window.this} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {that.this = 'this';} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() { () => undefined; } }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {} bar() {} }",
                        options => { except_methods => ["bar"] },
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method 'foo'" } }
                        ]
                    },
                    {
                        code => "class A { foo() {} hasOwnProperty() {} }",
                        options => { except_methods => ["foo"] },
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 20, message_id => "missing_this", data => { name => "method 'hasOwnProperty'" } }
                        ]
                    },
                    {
                        code => "class A { [foo]() {} }",
                        options => { except_methods => ["foo"] },
                        environment => { ecma_version => 6 },
                        errors => [
                            { type => Function, line => 1, column => 11, message_id => "missing_this", data => { name => "method" } }
                        ]
                    },
                    {
                        code => "class A { #foo() { } foo() {} #bar() {} }",
                        options => { except_methods => ["#foo"] },
                        environment => { ecma_version => 2022 },
                        errors => [
                            { type => Function, line => 1, column => 22, message_id => "missing_this", data => { name => "method 'foo'" } },
                            { type => Function, line => 1, column => 31, message_id => "missing_this", data => { name => "private method #bar" } }
                        ]
                    },
                    {
                        code => "class A { foo(){} 'bar'(){} 123(){} [`baz`](){} [a](){} [f(a)](){} get quux(){} set[a](b){} *quuux(){} }",
                        environment => { ecma_version => 6 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, type => Function, column => 11 },
                            { message_id => "missing_this", data => { name => "method 'bar'" }, type => Function, column => 19 },
                            { message_id => "missing_this", data => { name => "method '123'" }, type => Function, column => 29 },
                            { message_id => "missing_this", data => { name => "method 'baz'" }, type => Function, column => 37 },
                            { message_id => "missing_this", data => { name => "method" }, type => Function, column => 49 },
                            { message_id => "missing_this", data => { name => "method" }, type => Function, column => 57 },
                            { message_id => "missing_this", data => { name => "getter 'quux'" }, type => Function, column => 68 },
                            { message_id => "missing_this", data => { name => "setter" }, type => Function, column => 81 },
                            { message_id => "missing_this", data => { name => "generator method 'quuux'" }, type => Function, column => 93 }
                        ]
                    },
                    {
                        code => "class A { foo = function() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 25 }
                        ]
                    },
                    {
                        code => "class A { foo = () => {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 17 }
                        ]
                    },
                    {
                        code => "class A { #foo = function() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private method #foo" }, column => 11, end_column => 26 }
                        ]
                    },
                    {
                        code => "class A { #foo = () => {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private method #foo" }, column => 11, end_column => 18 }
                        ]
                    },
                    {
                        code => "class A { #foo() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private method #foo" }, column => 11, end_column => 15 }
                        ]
                    },
                    {
                        code => "class A { get #foo() {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private getter #foo" }, column => 11, end_column => 19 }
                        ]
                    },
                    {
                        code => "class A { set #foo(x) {} }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "private setter #foo" }, column => 11, end_column => 19 }
                        ]
                    },
                    {
                        code => "class A { foo () { return class { foo = this }; } }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 15 }
                        ]
                    },
                    {
                        code => "class A { foo () { return function () { foo = this }; } }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 15 }
                        ]
                    },
                    {
                        code => "class A { foo () { return class { static { this; } } } }",
                        environment => { ecma_version => 2022 },
                        errors => [
                            { message_id => "missing_this", data => { name => "method 'foo'" }, column => 11, end_column => 15 }
                        ]
                    }
                ]
            },
        )
    }
}
