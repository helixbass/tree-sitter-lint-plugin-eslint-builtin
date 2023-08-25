use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    words: bool,
    nonwords: bool,
    overrides: HashMap<String, bool>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            words: true,
            nonwords: Default::default(),
            overrides: Default::default(),
        }
    }
}

pub fn space_unary_ops_rule() -> Arc<dyn Rule> {
    rule! {
        name => "space-unary-ops",
        languages => [Javascript],
        messages => [
            unexpected_before => "Unexpected space before unary operator '{{operator}}'.",
            unexpected_after => "Unexpected space after unary operator '{{operator}}'.",
            unexpected_after_word => "Unexpected space after unary word operator '{{word}}'.",
            word_operator => "Unary word operator '{{word}}' must be followed by whitespace.",
            operator => "Unary operator '{{operator}}' must be followed by whitespace.",
            before_unary_expressions => "Space is required before unary expressions '{{token}}'.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-run]
            words: bool = options.words,
            nonwords: bool = options.nonwords,
            overrides: HashMap<String, bool> = options.overrides,
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
    use crate::kind::{AwaitExpression, YieldExpression};

    #[test]
    fn test_space_unary_ops_rule() {
        RuleTester::run(
            space_unary_ops_rule(),
            rule_tests! {
                valid => [
                    {
                        code => "++this.a",
                        options => { words => true }
                    },
                    {
                        code => "--this.a",
                        options => { words => true }
                    },
                    {
                        code => "this.a++",
                        options => { words => true }
                    },
                    {
                        code => "this.a--",
                        options => { words => true }
                    },
                    "foo .bar++",
                    {
                        code => "foo.bar --",
                        options => { nonwords => true }
                    },

                    {
                        code => "delete foo.bar",
                        options => { words => true }
                    },
                    {
                        code => "delete foo[\"bar\"]",
                        options => { words => true }
                    },
                    {
                        code => "delete foo.bar",
                        options => { words => false }
                    },
                    {
                        code => "delete(foo.bar)",
                        options => { words => false }
                    },

                    {
                        code => "new Foo",
                        options => { words => true }
                    },
                    {
                        code => "new Foo()",
                        options => { words => true }
                    },
                    {
                        code => "new [foo][0]",
                        options => { words => true }
                    },
                    {
                        code => "new[foo][0]",
                        options => { words => false }
                    },

                    {
                        code => "typeof foo",
                        options => { words => true }
                    },
                    {
                        code => "typeof{foo:true}",
                        options => { words => false }
                    },
                    {
                        code => "typeof {foo:true}",
                        options => { words => true }
                    },
                    {
                        code => "typeof (foo)",
                        options => { words => true }
                    },
                    {
                        code => "typeof(foo)",
                        options => { words => false }
                    },
                    {
                        code => "typeof!foo",
                        options => { words => false }
                    },

                    {
                        code => "void 0",
                        options => { words => true }
                    },
                    {
                        code => "(void 0)",
                        options => { words => true }
                    },
                    {
                        code => "(void (0))",
                        options => { words => true }
                    },
                    {
                        code => "void foo",
                        options => { words => true }
                    },
                    {
                        code => "void foo",
                        options => { words => false }
                    },
                    {
                        code => "void(foo)",
                        options => { words => false }
                    },

                    {
                        code => "-1",
                        options => { nonwords => false }
                    },
                    {
                        code => "!foo",
                        options => { nonwords => false }
                    },
                    {
                        code => "!!foo",
                        options => { nonwords => false }
                    },
                    {
                        code => "foo++",
                        options => { nonwords => false }
                    },
                    {
                        code => "foo ++",
                        options => { nonwords => true }
                    },
                    {
                        code => "++foo",
                        options => { nonwords => false }
                    },
                    {
                        code => "++ foo",
                        options => { nonwords => true }
                    },
                    {
                        code => "function *foo () { yield (0) }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield +1 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield* 0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield * 0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { (yield)*0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { (yield) * 0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield*0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield *0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "async function foo() { await {foo: 1} }",
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await {bar: 2} }",
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await{baz: 3} }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await {qux: 4} }",
                        options => { words => false, overrides => { "await" => true } },
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await{foo: 5} }",
                        options => { words => true, overrides => { "await" => false } },
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "foo++",
                        options => { nonwords => true, overrides => { "++" => false } }
                    },
                    {
                        code => "foo++",
                        options => { nonwords => false, overrides => { "++" => false } }
                    },
                    {
                        code => "++foo",
                        options => { nonwords => true, overrides => { "++" => false } }
                    },
                    {
                        code => "++foo",
                        options => { nonwords => false, overrides => { "++" => false } }
                    },
                    {
                        code => "!foo",
                        options => { nonwords => true, overrides => { "!" => false } }
                    },
                    {
                        code => "!foo",
                        options => { nonwords => false, overrides => { "!" => false } }
                    },
                    {
                        code => "new foo",
                        options => { words => true, overrides => { new => false } }
                    },
                    {
                        code => "new foo",
                        options => { words => false, overrides => { new => false } }
                    },
                    {
                        code => "function *foo () { yield(0) }",
                        options => { words => true, overrides => { "yield" => false } },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo () { yield(0) }",
                        options => { words => false, overrides => { "yield" => false } },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "class C { #x; *foo(bar) { yield#x in bar; } }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 2022 }
                    }
                ],
                invalid => [
                    {
                        code => "delete(foo.bar)",
                        output => "delete (foo.bar)",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "delete" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "delete(foo[\"bar\"]);",
                        output => "delete (foo[\"bar\"]);",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "delete" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "delete (foo.bar)",
                        output => "delete(foo.bar)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "delete" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "new(Foo)",
                        output => "new (Foo)",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "new" },
                            type => "NewExpression"
                        }]
                    },
                    {
                        code => "new (Foo)",
                        output => "new(Foo)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "new" },
                            type => "NewExpression"
                        }]
                    },
                    {
                        code => "new(Foo())",
                        output => "new (Foo())",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "new" },
                            type => "NewExpression"
                        }]
                    },
                    {
                        code => "new [foo][0]",
                        output => "new[foo][0]",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "new" },
                            type => "NewExpression"
                        }]
                    },

                    {
                        code => "typeof(foo)",
                        output => "typeof (foo)",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "typeof" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "typeof (foo)",
                        output => "typeof(foo)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "typeof" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "typeof[foo]",
                        output => "typeof [foo]",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "typeof" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "typeof [foo]",
                        output => "typeof[foo]",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "typeof" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "typeof{foo:true}",
                        output => "typeof {foo:true}",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "typeof" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "typeof {foo:true}",
                        output => "typeof{foo:true}",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "typeof" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "typeof!foo",
                        output => "typeof !foo",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "typeof" },
                            type => "UnaryExpression"
                        }]
                    },

                    {
                        code => "void(0);",
                        output => "void (0);",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "void" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "void(foo);",
                        output => "void (foo);",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "void" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "void[foo];",
                        output => "void [foo];",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "void" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "void{a:0};",
                        output => "void {a:0};",
                        options => { words => true },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "void" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "void (foo)",
                        output => "void(foo)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "void" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "void [foo]",
                        output => "void[foo]",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "void" },
                            type => "UnaryExpression"
                        }]
                    },

                    {
                        code => "! foo",
                        output => "!foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "!" }
                        }]
                    },
                    {
                        code => "!foo",
                        output => "! foo",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" }
                        }]
                    },

                    {
                        code => "!! foo",
                        output => "!!foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "!" },
                            type => "UnaryExpression",
                            line => 1,
                            column => 2
                        }]
                    },
                    {
                        code => "!!foo",
                        output => "!! foo",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" },
                            type => "UnaryExpression",
                            line => 1,
                            column => 2
                        }]
                    },

                    {
                        code => "- 1",
                        output => "-1",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "-" },
                            type => "UnaryExpression"
                        }]
                    },
                    {
                        code => "-1",
                        output => "- 1",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "-" },
                            type => "UnaryExpression"
                        }]
                    },

                    {
                        code => "foo++",
                        output => "foo ++",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "beforeUnaryExpressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "foo ++",
                        output => "foo++",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedBefore",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "++ foo",
                        output => "++foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "++foo",
                        output => "++ foo",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "foo .bar++",
                        output => "foo .bar ++",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "beforeUnaryExpressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "foo.bar --",
                        output => "foo.bar--",
                        errors => [{
                            message_id => "unexpectedBefore",
                            data => { operator => "--" }
                        }]
                    },
                    {
                        code => "+ +foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "+" }
                        }]
                    },
                    {
                        code => "+ ++foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "+" }
                        }]
                    },
                    {
                        code => "- -foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "-" }
                        }]
                    },
                    {
                        code => "- --foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "-" }
                        }]
                    },
                    {
                        code => "+ -foo",
                        output => "+-foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpectedAfter",
                            data => { operator => "+" }
                        }]
                    },
                    {
                        code => "function *foo() { yield(0) }",
                        output => "function *foo() { yield (0) }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "function *foo() { yield (0) }",
                        output => "function *foo() { yield(0) }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "function *foo() { yield+0 }",
                        output => "function *foo() { yield +0 }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "foo++",
                        output => "foo ++",
                        options => { nonwords => true, overrides => { "++" => true } },
                        errors => [{
                            message_id => "beforeUnaryExpressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "foo++",
                        output => "foo ++",
                        options => { nonwords => false, overrides => { "++" => true } },
                        errors => [{
                            message_id => "beforeUnaryExpressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "++foo",
                        output => "++ foo",
                        options => { nonwords => true, overrides => { "++" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "++foo",
                        output => "++ foo",
                        options => { nonwords => false, overrides => { "++" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "!foo",
                        output => "! foo",
                        options => { nonwords => true, overrides => { "!" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" }
                        }]
                    },
                    {
                        code => "!foo",
                        output => "! foo",
                        options => { nonwords => false, overrides => { "!" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" }
                        }]
                    },
                    {
                        code => "new(Foo)",
                        output => "new (Foo)",
                        options => { words => true, overrides => { new => true } },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "new" }
                        }]
                    },
                    {
                        code => "new(Foo)",
                        output => "new (Foo)",
                        options => { words => false, overrides => { new => true } },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "new" }
                        }]
                    },
                    {
                        code => "function *foo() { yield(0) }",
                        output => "function *foo() { yield (0) }",
                        options => { words => true, overrides => { "yield" => true } },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "function *foo() { yield(0) }",
                        output => "function *foo() { yield (0) }",
                        options => { words => false, overrides => { "yield" => true } },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "async function foo() { await{foo: 'bar'} }",
                        output => "async function foo() { await {foo: 'bar'} }",
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "async function foo() { await{baz: 'qux'} }",
                        output => "async function foo() { await {baz: 'qux'} }",
                        options => { words => false, overrides => { "await" => true } },
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "wordOperator",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "async function foo() { await {foo: 1} }",
                        output => "async function foo() { await{foo: 1} }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "async function foo() { await {bar: 2} }",
                        output => "async function foo() { await{bar: 2} }",
                        options => { words => true, overrides => { "await" => false } },
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "class C { #x; *foo(bar) { yield #x in bar; } }",
                        output => "class C { #x; *foo(bar) { yield#x in bar; } }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 27
                        }]
                    }
                ]
            },
        )
    }
}
