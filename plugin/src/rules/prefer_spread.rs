use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn prefer_spread_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-spread",
        languages => [Javascript],
        messages => [
            prefer_spread => "Use the spread operator instead of '.apply()'.",
        ],
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
    use crate::kind::CallExpression;

    #[test]
    fn test_prefer_spread_rule() {
        let errors = vec![
            RuleTestExpectedErrorBuilder::default()
                .message_id("prefer_spread")
                .type_(CallExpression)
                .build().unwrap()
        ];

        RuleTester::run(
            prefer_spread_rule(),
            rule_tests! {
                valid => [
                    "foo.apply(obj, args);",
                    "obj.foo.apply(null, args);",
                    "obj.foo.apply(otherObj, args);",
                    "a.b(x, y).c.foo.apply(a.b(x, z).c, args);",
                    "a.b.foo.apply(a.b.c, args);",

                    // ignores non variadic.
                    "foo.apply(undefined, [1, 2]);",
                    "foo.apply(null, [1, 2]);",
                    "obj.foo.apply(obj, [1, 2]);",

                    // ignores computed property.
                    "var apply; foo[apply](null, args);",

                    // ignores incomplete things.
                    "foo.apply();",
                    "obj.foo.apply();",
                    "obj.foo.apply(obj, ...args)",

                    // Optional chaining
                    "(a?.b).c.foo.apply(a?.b.c, args);",
                    "a?.b.c.foo.apply((a?.b).c, args);",

                    // Private fields
                    "class C { #apply; foo() { foo.#apply(undefined, args); } }"
                ],
                invalid => [
                    {
                        code => "foo.apply(undefined, args);",
                        errors => errors,
                    },
                    {
                        code => "foo.apply(void 0, args);",
                        errors => errors,
                    },
                    {
                        code => "foo.apply(null, args);",
                        errors => errors,
                    },
                    {
                        code => "obj.foo.apply(obj, args);",
                        errors => errors,
                    },
                    {
                        code => "a.b.c.foo.apply(a.b.c, args);",
                        errors => errors,
                    },
                    {
                        code => "a.b(x, y).c.foo.apply(a.b(x, y).c, args);",
                        errors => errors,
                    },
                    {
                        code => "[].concat.apply([ ], args);",
                        errors => errors,
                    },
                    {
                        code => "[].concat.apply([\n/*empty*/\n], args);",
                        errors => errors,
                    },

                    // Optional chaining
                    {
                        code => "foo.apply?.(undefined, args);",
                        errors => errors,
                    },
                    {
                        code => "foo?.apply(undefined, args);",
                        errors => errors,
                    },
                    {
                        code => "foo?.apply?.(undefined, args);",
                        errors => errors,
                    },
                    {
                        code => "(foo?.apply)(undefined, args);",
                        errors => errors,
                    },
                    {
                        code => "(foo?.apply)?.(undefined, args);",
                        errors => errors,
                    },
                    {
                        code => "(obj?.foo).apply(obj, args);",
                        errors => errors,
                    },
                    {
                        code => "a?.b.c.foo.apply(a?.b.c, args);",
                        errors => errors,
                    },
                    {
                        code => "(a?.b.c).foo.apply(a?.b.c, args);",
                        errors => errors,
                    },
                    {
                        code => "(a?.b).c.foo.apply((a?.b).c, args);",
                        errors => errors,
                    },

                    // Private fields
                    {
                        code => "class C { #foo; foo() { obj.#foo.apply(obj, args); } }",
                        errors => errors,
                    }
                ]
            },
        )
    }
}
