use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, Rule};

use crate::{
    ast_helpers::{get_last_expression_of_sequence_expression, is_outermost_chain_expression},
    kind::{
        AwaitExpression, BinaryExpression, CallExpression, ClassHeritage, MemberExpression, Object,
        ParenthesizedExpression, SequenceExpression, SubscriptExpression, TernaryExpression,
    },
};
use tree_sitter_lint::QueryMatchContext;

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    disallow_arithmetic_operators: bool,
}

fn check_undefined_short_circuit(
    node: Node,
    report_func: &impl Fn(Node),
    context: &QueryMatchContext,
) {
    match node.kind() {
        BinaryExpression => match node.field("operator").kind() {
            "||" | "??" => check_undefined_short_circuit(node.field("right"), report_func, context),
            "&&" => {
                check_undefined_short_circuit(node.field("left"), report_func, context);
                check_undefined_short_circuit(node.field("right"), report_func, context);
            }
            _ => (),
        },
        SequenceExpression => {
            check_undefined_short_circuit(
                get_last_expression_of_sequence_expression(node),
                report_func, context,
            );
        }
        TernaryExpression => {
            check_undefined_short_circuit(node.field("consequence"), report_func, context);
            check_undefined_short_circuit(node.field("alternative"), report_func, context);
        }
        AwaitExpression | ParenthesizedExpression | ClassHeritage => {
            check_undefined_short_circuit(node.first_non_comment_named_child(context), report_func, context);
        }
        CallExpression | MemberExpression | SubscriptExpression => {
            if is_outermost_chain_expression(node) {
                report_func(node);
            }
        }
        _ => (),
    }
}

pub fn no_unsafe_optional_chaining_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-unsafe-optional-chaining",
        languages => [Javascript],
        messages => [
            unsafe_optional_chain => "Unsafe usage of optional chaining. If it short-circuits with 'undefined' the evaluation will throw TypeError.",
            unsafe_arithmetic => "Unsafe arithmetic operation on optional chaining. It can result in NaN.",
        ],
        options_type => Options,
        state => {
            [per-run]
            // disallow_arithmetic_operators: bool = options.disallow_arithmetic_operators,
            disallow_arithmetic_operators: bool = {
                options.disallow_arithmetic_operators
            },
        },
        listeners => [
            r#"
              (assignment_expression
                left: [
                  (object_pattern)
                  (array_pattern)
                ]
                right: (_) @c
              )
              (assignment_pattern
                left: [
                  (object_pattern)
                  (array_pattern)
                ]
                right: (_) @c
              )
              (class_declaration
                (class_heritage) @c
              )
              (class
                (class_heritage) @c
              )
              (call_expression
                function: (_) @c
                !optional_chain
              )
              (new_expression
                constructor: (_) @c
              )
              (variable_declarator
                name: [
                  (object_pattern)
                  (array_pattern)
                ]
                value: (_) @c
              )
              (member_expression
                object: (_) @c
                !optional_chain
              )
              (subscript_expression
                object: (_) @c
                !optional_chain
              )
              (for_in_statement
                operator: "of"
                right: (_) @c
              )
              (binary_expression
                operator: [
                  "in"
                  "instanceof"
                ]
                right: (_) @c
              )
              (with_statement
                object: (_) @c
              )
            "# => |node, context| {
                check_undefined_short_circuit(
                    node,
                    &|node| {
                        context.report(violation! {
                            message_id => "unsafe_optional_chain",
                            node => node,
                        });
                    },
                    context
                );
            },
            r#"
              (spread_element) @c
            "# => |node, context| {
                if node.parent().unwrap().kind() == Object {
                    return;
                }

                check_undefined_short_circuit(
                    node.first_non_comment_named_child(context),
                    &|node| {
                        context.report(violation! {
                            message_id => "unsafe_optional_chain",
                            node => node,
                        });
                    },
                    context
                );
            },
            r#"
              (binary_expression
                left: (_) @c
                operator: [
                  "+"
                  "-"
                  "/"
                  "*"
                  "%"
                  "**"
                ]
                right: (_) @c
              )
              (unary_expression
                operator: [
                  "+"
                  "-"
                  "/"
                  "*"
                  "%"
                  "**"
                ]
                argument: (_) @c
              )
              (augmented_assignment_expression
                operator: [
                  "+="
                  "-="
                  "/="
                  "*="
                  "%="
                  "**="
                ]
                right: (_) @c
              )
            "# => |node, context| {
                if !self.disallow_arithmetic_operators {
                    return;
                }

                check_undefined_short_circuit(
                    node,
                    &|node| {
                        context.report(violation! {
                            message_id => "unsafe_arithmetic",
                            node => node,
                        });
                    },
                    context
                );
            }
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::MemberExpression;

    use super::*;

    use itertools::Itertools;
    use tree_sitter_lint::{
        rule_tests, serde_json::json, RuleTestExpectedErrorBuilder, RuleTestInvalidBuilder,
        RuleTestValidBuilder, RuleTester,
    };

    #[test]
    fn test_no_unsafe_optional_chaining_rule() {
        RuleTester::run(
            no_unsafe_optional_chaining_rule(),
            rule_tests! {
                valid => [
                    "var foo;",
                    "class Foo {}",
                    "!!obj?.foo",
                    "obj?.foo();",
                    "obj?.foo?.();",
                    "(obj?.foo ?? bar)();",
                    "(obj?.foo)?.()",
                    "(obj?.foo ?? bar?.baz)?.()",
                    "(obj.foo)?.();",
                    "obj?.foo.bar;",
                    "obj?.foo?.bar;",
                    "(obj?.foo)?.bar;",
                    "(obj?.foo)?.bar.baz;",
                    "(obj?.foo)?.().bar",
                    "(obj?.foo ?? bar).baz;",
                    "(obj?.foo ?? val)`template`",
                    "new (obj?.foo ?? val)()",
                    "new bar();",
                    "obj?.foo?.()();",
                    "const {foo} = obj?.baz || {};",
                    "const foo = obj?.bar",
                    "foo = obj?.bar",
                    "foo.bar = obj?.bar",
                    "bar(...obj?.foo ?? []);",
                    "var bar = {...foo?.bar};",
                    "foo?.bar in {};",
                    "foo?.bar < foo?.baz;",
                    "foo?.bar <= foo?.baz;",
                    "foo?.bar > foo?.baz;",
                    "foo?.bar >= foo?.baz;",
                    "[foo = obj?.bar] = [];",
                    "[foo.bar = obj?.bar] = [];",
                    "({foo = obj?.bar} = obj);",
                    "({foo: obj.bar = obj?.baz} = obj);",
                    "(foo?.bar, bar)();",
                    "(foo?.bar ? baz : qux)();",
                    "
                    async function func() {
                      await obj?.foo();
                      await obj?.foo?.();
                      (await obj?.foo)?.();
                      (await obj?.foo)?.bar;
                      await bar?.baz;
                      await (foo ?? obj?.foo.baz);
                      (await bar?.baz ?? bar).baz;
                      (await bar?.baz ?? await bar).baz;
                      await (foo?.bar ? baz : qux);
                    }
                    ",

                    // logical operations
                    "(obj?.foo ?? bar?.baz ?? qux)();",
                    "((obj?.foo ?? bar?.baz) || qux)();",
                    "((obj?.foo || bar?.baz) || qux)();",
                    "((obj?.foo && bar?.baz) || qux)();",

                    // The default value option disallowArithmeticOperators is false
                    "obj?.foo - bar;",
                    "obj?.foo + bar;",
                    "obj?.foo * bar;",
                    "obj?.foo / bar;",
                    "obj?.foo % bar;",
                    "obj?.foo ** bar;",
                    "+obj?.foo;",
                    "-obj?.foo;",
                    "bar += obj?.foo;",
                    "bar -= obj?.foo;",
                    "bar %= obj?.foo;",
                    "bar **= obj?.foo;",
                    "bar *= obj?.boo",
                    "bar /= obj?.boo",
                    "async function func() {
                        await obj?.foo + await obj?.bar;
                        await obj?.foo - await obj?.bar;
                        await obj?.foo * await obj?.bar;
                        +await obj?.foo;
                        -await obj?.foo;
                        bar += await obj?.foo;
                        bar -= await obj?.foo;
                        bar %= await obj?.foo;
                        bar **= await obj?.foo;
                        bar *= await obj?.boo;
                        bar /= await obj?.boo;
                    }
                    ",
                    ...[
                        "obj?.foo | bar",
                        "obj?.foo & bar",
                        "obj?.foo >> obj?.bar;",
                        "obj?.foo << obj?.bar;",
                        "obj?.foo >>> obj?.bar;",
                        "(obj?.foo || baz) + bar;",
                        "(obj?.foo ?? baz) + bar;",
                        "(obj?.foo ?? baz) - bar;",
                        "(obj?.foo ?? baz) * bar;",
                        "(obj?.foo ?? baz) / bar;",
                        "(obj?.foo ?? baz) % bar;",
                        "(obj?.foo ?? baz) ** bar;",
                        "void obj?.foo;",
                        "typeof obj?.foo;",
                        "!obj?.foo",
                        "~obj?.foo",
                        "+(obj?.foo ?? bar)",
                        "-(obj?.foo ?? bar)",
                        "bar |= obj?.foo;",
                        "bar &= obj?.foo;",
                        "bar ^= obj?.foo;",
                        "bar <<= obj?.foo;",
                        "bar >>= obj?.foo;",
                        "bar >>>= obj?.foo;",
                        "bar ||= obj?.foo",
                        "bar &&= obj?.foo",
                        "bar += (obj?.foo ?? baz);",
                        "bar -= (obj?.foo ?? baz)",
                        "bar *= (obj?.foo ?? baz)",
                        "bar /= (obj?.foo ?? baz)",
                        "bar %= (obj?.foo ?? baz);",
                        "bar **= (obj?.foo ?? baz)",

                        r#"async function foo() {
                          (await obj?.foo || baz) + bar;
                          (await obj?.foo ?? baz) + bar;
                          (await obj?.foo ?? baz) - bar;
                          (await obj?.foo ?? baz) * bar;
                          (await obj?.foo ?? baz) / bar;
                          (await obj?.foo ?? baz) % bar;
                          "(await obj?.foo ?? baz) ** bar;",
                          "void await obj?.foo;",
                          "typeof await obj?.foo;",
                          "!await obj?.foo",
                          "~await obj?.foo",
                          "+(await obj?.foo ?? bar)",
                          "-(await obj?.foo ?? bar)",
                          bar |= await obj?.foo;
                          bar &= await obj?.foo;
                          bar ^= await obj?.foo;
                          bar <<= await obj?.foo;
                          bar >>= await obj?.foo;
                          bar >>>= await obj?.foo
                          bar += ((await obj?.foo) ?? baz);
                          bar -= ((await obj?.foo) ?? baz);
                          bar /= ((await obj?.foo) ?? baz);
                          bar %= ((await obj?.foo) ?? baz);
                          bar **= ((await obj?.foo) ?? baz);
                        }"#
                    ].into_iter().map(|code| {
                        RuleTestValidBuilder::default()
                            .code(code)
                            .options(json!({"disallow_arithmetic_operators": true}))
                            .build()
                            .unwrap()
                    }).collect_vec(),
                    {
                        code => "obj?.foo - bar;",
                        options => {}
                    },
                    {
                        code => "obj?.foo - bar;",
                        options => {
                            disallow_arithmetic_operators => false
                        }
                    }
                ],
                invalid => [
                    ...[
                        "(obj?.foo)();",
                        "(obj.foo ?? bar?.baz)();",
                        "(obj.foo || bar?.baz)();",
                        "(obj?.foo && bar)();",
                        "(bar && obj?.foo)();",
                        "(obj?.foo).bar",
                        "(obj?.foo)[1];",
                        "(obj?.foo)`template`",
                        "new (obj?.foo)();",
                        "new (obj?.foo?.() || obj?.bar)()",

                        "async function foo() {
                          (await obj?.foo)();
                        }",
                        "async function foo() {
                          (await obj?.foo).bar;
                        }",
                        "async function foo() {
                          (bar?.baz ?? await obj?.foo)();
                        }",
                        "async function foo() {
                          (bar && await obj?.foo)();
                        }",
                        "async function foo() {
                          (await (bar && obj?.foo))();
                        }",

                        // spread
                        "[...obj?.foo];",
                        "bar(...obj?.foo);",
                        "new Bar(...obj?.foo);",

                        // destructuring
                        "const {foo} = obj?.bar;",
                        "const [foo] = obj?.bar;",
                        "const [foo] = obj?.bar || obj?.foo;",
                        "([foo] = obj?.bar);",
                        "[{ foo } = obj?.bar] = [];",
                        "({bar: [ foo ] = obj?.prop} = {});",
                        "[[ foo ] = obj?.bar] = [];",
                        "async function foo() { const {foo} = await obj?.bar; }",
                        "async function foo() { const [foo] = await obj?.bar || await obj?.foo; }",
                        "async function foo() { ([foo] = await obj?.bar); }",

                        // class declaration
                        "class A extends obj?.foo {}",
                        "async function foo() { class A extends (await obj?.foo) {}}",

                        // class expression
                        "var a = class A extends obj?.foo {}",
                        "async function foo() { var a = class A extends (await obj?.foo) {}}",

                        // relational operations
                        "foo instanceof obj?.prop",
                        "async function foo() { foo instanceof await obj?.prop }",
                        "1 in foo?.bar;",
                        "async function foo() { 1 in await foo?.bar; }",

                        // for...of
                        "for (foo of obj?.bar);",
                        "async function foo() { for (foo of await obj?.bar);}",

                        // sequence expression
                        "(foo, obj?.foo)();",
                        "(foo, obj?.foo)[1];",
                        "async function foo() { (await (foo, obj?.foo))(); }",
                        "async function foo() { ((foo, await obj?.foo))(); }",
                        "async function foo() { (foo, await obj?.foo)[1]; }",
                        "async function foo() { (await (foo, obj?.foo)) [1]; }",

                        // conditional expression
                        "(a ? obj?.foo : b)();",
                        "(a ? b : obj?.foo)();",
                        "(a ? obj?.foo : b)[1];",
                        "(a ? b : obj?.foo).bar;",
                        "async function foo() { (await (a ? obj?.foo : b))(); }",
                        "async function foo() { (a ? await obj?.foo : b)(); }",
                        "async function foo() { (await (a ? b : obj?.foo))(); }",
                        "async function foo() { (await (a ? obj?.foo : b))[1]; }",
                        "async function foo() { (await (a ? b : obj?.foo)).bar; }",
                        "async function foo() { (a ? b : await obj?.foo).bar; }"
                    ].into_iter().map(|code| {
                        RuleTestInvalidBuilder::default()
                            .code(code)
                            .errors(vec![
                                RuleTestExpectedErrorBuilder::default()
                                    .message_id("unsafe_optional_chain")
                                    .type_(MemberExpression)
                                    .build()
                                    .unwrap()
                            ])
                            .build()
                            .unwrap()
                    }).collect_vec(),
                    ...[
                        "new (obj?.foo?.())()",
                        "const {foo} = obj?.bar();",
                        "const {foo: bar} = obj?.bar();",
                        "const [foo] = obj?.bar?.();",
                        "async function foo() { const {foo} = await obj?.bar(); }",
                    ].into_iter().map(|code| {
                        RuleTestInvalidBuilder::default()
                            .code(code)
                            .errors(vec![
                                RuleTestExpectedErrorBuilder::default()
                                    .message_id("unsafe_optional_chain")
                                    .type_(CallExpression)
                                    .build()
                                    .unwrap()
                            ])
                            .build()
                            .unwrap()
                    }).collect_vec(),
                    {
                        code => "(obj?.foo && obj?.baz).bar",
                        errors => [
                            {
                                message_id => "unsafe_optional_chain",
                                type => MemberExpression,
                                line => 1,
                                column => 2
                            },
                            {
                                message_id => "unsafe_optional_chain",
                                type => MemberExpression,
                                line => 1,
                                column => 14
                            }
                        ]
                    },
                    {
                        code => "with (obj?.foo) {};",
                        // parserOptions: {
                        //     sourceType: "script"
                        // },
                        errors => [
                            {
                                message_id => "unsafe_optional_chain",
                                type => MemberExpression,
                                line => 1,
                                column => 7
                            }
                        ]
                    },
                    {
                        code => "async function foo() { with ( await obj?.foo) {}; }",
                        // parserOptions: {
                        //     sourceType: "script"
                        // },
                        errors => [
                            {
                                message_id => "unsafe_optional_chain",
                                type => MemberExpression,
                                line => 1,
                                column => 37
                            }
                        ]
                    },
                    {
                        code => "(foo ? obj?.foo : obj?.bar).bar",
                        errors => [
                            {
                                message_id => "unsafe_optional_chain",
                                type => MemberExpression,
                                line => 1,
                                column => 8
                            },
                            {
                                message_id => "unsafe_optional_chain",
                                type => MemberExpression,
                                line => 1,
                                column => 19
                            }
                        ]
                    },
                    ...[
                        "obj?.foo + bar;",
                        "(foo || obj?.foo) + bar;",
                        "bar + (foo || obj?.foo);",
                        "(a ? obj?.foo : b) + bar",
                        "(a ? b : obj?.foo) + bar",
                        "(foo, bar, baz?.qux) + bar",
                        "obj?.foo - bar;",
                        "obj?.foo * bar;",
                        "obj?.foo / bar;",
                        "obj?.foo % bar;",
                        "obj?.foo ** bar;",
                        "+obj?.foo;",
                        "-obj?.foo;",
                        "+(foo ?? obj?.foo);",
                        "+(foo || obj?.bar);",
                        "+(obj?.bar && foo);",
                        "+(foo ? obj?.foo : bar);",
                        "+(foo ? bar : obj?.foo);",
                        "bar += obj?.foo;",
                        "bar -= obj?.foo;",
                        "bar %= obj?.foo;",
                        "bar **= obj?.foo;",
                        "bar *= obj?.boo",
                        "bar /= obj?.boo",
                        "bar += (foo ?? obj?.foo);",
                        "bar += (foo || obj?.foo);",
                        "bar += (foo && obj?.foo);",
                        "bar += (foo ? obj?.foo : bar);",
                        "bar += (foo ? bar : obj?.foo);",
                        "async function foo() { await obj?.foo + bar; }",
                        "async function foo() { (foo || await obj?.foo) + bar;}",
                        "async function foo() { bar + (foo || await obj?.foo); }"
                    ].into_iter().map(|code| {
                        RuleTestInvalidBuilder::default()
                            .code(code)
                            .options(json!({"disallow_arithmetic_operators": true}))
                            .errors(vec![
                                RuleTestExpectedErrorBuilder::default()
                                    .message_id("unsafe_arithmetic")
                                    .type_(MemberExpression)
                                    .build()
                                    .unwrap()
                            ])
                            .build()
                            .unwrap()
                    }).collect_vec(),
                ]
            },
        )
    }
}
