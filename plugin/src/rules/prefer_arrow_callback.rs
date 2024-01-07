use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    allow_named_functions: bool,
    allow_unbound_this: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            allow_named_functions: false,
            allow_unbound_this: true,
        }
    }
}

pub fn prefer_arrow_callback_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-arrow-callback",
        languages => [Javascript],
        messages => [
            prefer_arrow_callback => "Unexpected function expression.",
        ],
        fixable => true,
        options_type => Options,
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
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::Function;

    #[test]
    fn test_prefer_arrow_callback_rule() {
        let errors = vec![
            RuleTestExpectedErrorBuilder::default()
                .message_id("prefer_arrow_callback")
                .type_(Function)
                .build().unwrap()
        ];

        RuleTester::run(
            prefer_arrow_callback_rule(),
            rule_tests! {
                valid => [
                    "foo(a => a);",
                    "foo(function*() {});",
                    "foo(function() { this; });",
                    { code => "foo(function bar() {});", options => { allow_named_functions => true } },
                    "foo(function() { (() => this); });",
                    "foo(function() { this; }.bind(obj));",
                    "foo(function() { this; }.call(this));",
                    "foo(a => { (function() {}); });",
                    "var foo = function foo() {};",
                    "(function foo() {})();",
                    "foo(function bar() { bar; });",
                    "foo(function bar() { arguments; });",
                    "foo(function bar() { arguments; }.bind(this));",
                    "foo(function bar() { new.target; });",
                    "foo(function bar() { new.target; }.bind(this));",
                    "foo(function bar() { this; }.bind(this, somethingElse));",
                    "foo((function() {}).bind.bar)",
                    "foo((function() { this.bar(); }).bind(obj).bind(this))"
                ],
                invalid => [
                    {
                        code => "foo(function bar() {});",
                        output => "foo(() => {});",
                        errors => errors,
                    },
                    {
                        code => "foo(function() {});",
                        output => "foo(() => {});",
                        options => { allow_named_functions => true },
                        errors => errors,
                    },
                    {
                        code => "foo(function bar() {});",
                        output => "foo(() => {});",
                        options => { allow_named_functions => false },
                        errors => errors,
                    },
                    {
                        code => "foo(function() {});",
                        output => "foo(() => {});",
                        errors => errors,
                    },
                    {
                        code => "foo(nativeCb || function() {});",
                        output => "foo(nativeCb || (() => {}));",
                        errors => errors,
                    },
                    {
                        code => "foo(bar ? function() {} : function() {});",
                        output => "foo(bar ? () => {} : () => {});",
                        errors => [errors[0], errors[0]],
                    },
                    {
                        code => "foo(function() { (function() { this; }); });",
                        output => "foo(() => { (function() { this; }); });",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { this; }.bind(this));",
                        output => "foo(() => { this; });",
                        errors => errors,
                    },
                    {
                        code => "foo(bar || function() { this; }.bind(this));",
                        output => "foo(bar || (() => { this; }));",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { (() => this); }.bind(this));",
                        output => "foo(() => { (() => this); });",
                        errors => errors,
                    },
                    {
                        code => "foo(function bar(a) { a; });",
                        output => "foo((a) => { a; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function(a) { a; });",
                        output => "foo((a) => { a; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function(arguments) { arguments; });",
                        output => "foo((arguments) => { arguments; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { this; });",
                        output => None, // No fix applied
                        options => { allow_unbound_this => false },
                        errors => errors,
                    },
                    {
                        code => "foo(function() { (() => this); });",
                        output => None, // No fix applied
                        options => { allow_unbound_this => false },
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo, bar, baz) { return foo * 2; })",
                        output => "qux((foo, bar, baz) => { return foo * 2; })",
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo, bar, baz) { return foo * bar; }.bind(this))",
                        output => "qux((foo, bar, baz) => { return foo * bar; })",
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo, bar, baz) { return foo * this.qux; }.bind(this))",
                        output => "qux((foo, bar, baz) => { return foo * this.qux; })",
                        errors => errors,
                    },
                    {
                        code => "foo(function() {}.bind(this, somethingElse))",
                        output => "foo((() => {}).bind(this, somethingElse))",
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo = 1, [bar = 2] = [], {qux: baz = 3} = {foo: 'bar'}) { return foo + bar; });",
                        output => "qux((foo = 1, [bar = 2] = [], {qux: baz = 3} = {foo: 'bar'}) => { return foo + bar; });",
                        errors => errors,
                    },
                    {
                        code => "qux(function(baz, baz) { })",
                        output => None, // Duplicate parameter names are a SyntaxError in arrow functions
                        errors => errors,
                    },
                    {
                        code => "qux(function( /* no params */ ) { })",
                        output => "qux(( /* no params */ ) => { })",
                        errors => errors,
                    },
                    {
                        code => "qux(function( /* a */ foo /* b */ , /* c */ bar /* d */ , /* e */ baz /* f */ ) { return foo; })",
                        output => "qux(( /* a */ foo /* b */ , /* c */ bar /* d */ , /* e */ baz /* f */ ) => { return foo; })",
                        errors => errors,
                    },
                    {
                        code => "qux(async function (foo = 1, bar = 2, baz = 3) { return baz; })",
                        output => "qux(async (foo = 1, bar = 2, baz = 3) => { return baz; })",
                        errors => errors,
                    },
                    {
                        code => "qux(async function (foo = 1, bar = 2, baz = 3) { return this; }.bind(this))",
                        output => "qux(async (foo = 1, bar = 2, baz = 3) => { return this; })",
                        errors => errors,
                    },
                    {
                        code => "foo((bar || function() {}).bind(this))",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo(function() {}.bind(this).bind(obj))",
                        output => "foo((() => {}).bind(obj))",
                        errors => errors,
                    },

                    // Optional chaining
                    {
                        code => "foo?.(function() {});",
                        output => "foo?.(() => {});",
                        errors => errors,
                    },
                    {
                        code => "foo?.(function() { return this; }.bind(this));",
                        output => "foo?.(() => { return this; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { return this; }?.bind(this));",
                        output => "foo(() => { return this; });",
                        errors => errors,
                    },
                    {
                        code => "foo((function() { return this; }?.bind)(this));",
                        output => None,
                        errors => errors,
                    },

                    // https://github.com/eslint/eslint/issues/16718
                    {
                        code => r#"
test(
    function ()
    { }
);
                        "#,
                        output => r#"
test(
    () =>
    { }
);
                        "#,
                        errors => errors,
                    },
                    {
                        code => r#"
test(
    function (
        ...args
    ) /* Lorem ipsum
    dolor sit amet. */ {
        return args;
    }
);
                        "#,
                        output => r#"
test(
    (
        ...args
    ) => /* Lorem ipsum
    dolor sit amet. */ {
        return args;
    }
);
                        "#,
                        errors => errors,
                    }
                ]
            },
        )
    }
}
