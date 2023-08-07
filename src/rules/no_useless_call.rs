use std::sync::Arc;

use squalid::return_default_if_none;
use tree_sitter_lint::{
    rule, tree_sitter::Node, violation, FromFileRunContextInstanceProviderFactory, NodeExt,
    QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::{get_call_expression_arguments, get_num_call_expression_arguments, NodeExtJs},
    kind::{Array, MemberExpression},
    utils::ast_utils,
};

fn is_call_or_non_variadic_apply(
    node: Node,
    context: &QueryMatchContext<impl FromFileRunContextInstanceProviderFactory>,
) -> bool {
    match &*node
        .field("function")
        .skip_parentheses()
        .field("property")
        .text(context)
    {
        "call" => return_default_if_none!(get_num_call_expression_arguments(node)) >= 1,
        "apply" => {
            let arguments =
                return_default_if_none!(get_call_expression_arguments(node)).collect::<Vec<_>>();
            arguments.len() == 2 && arguments[1].kind() == Array
        }
        _ => unreachable!(),
    }
}

fn is_valid_this_arg<'a>(
    expected_this: Option<Node<'a>>,
    this_arg: Node<'a>,
    context: &QueryMatchContext<'a, '_, impl FromFileRunContextInstanceProviderFactory>,
) -> bool {
    match expected_this {
        None => ast_utils::is_null_or_undefined(this_arg, context),
        Some(expected_this) => ast_utils::equal_tokens(expected_this, this_arg, context),
    }
}

pub fn no_useless_call_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-useless-call",
        languages => [Javascript],
        messages => [
            unnecessary_call => "Unnecessary '.{{name}}()'.",
        ],
        listeners => [
            r#"[
              (call_expression
                function: (member_expression
                  property: (property_identifier) @callee_property (#match? @callee_property "^(?:call|apply)$")
                )
              )
              (call_expression
                function: (parenthesized_expression
                  (member_expression
                    property: (property_identifier) @callee_property (#match? @callee_property "^(?:call|apply)$")
                  )
                )
              )
            ] @call_expression
            "# => {
                capture_name => "call_expression",
                callback => |node, context| {
                    if !is_call_or_non_variadic_apply(node, context) {
                        return;
                    }

                    let callee = node.field("function").skip_parentheses();
                    let applied = callee.field("object").skip_parentheses();
                    let expected_this = (applied.kind() == MemberExpression).then(|| {
                        applied.field("object").skip_parentheses()
                    });
                    let this_arg = get_call_expression_arguments(node).unwrap().next().unwrap();

                    if is_valid_this_arg(expected_this, this_arg, context) {
                        context.report(violation! {
                            node => node,
                            message_id => "unnecessary_call",
                            data => {
                                name => callee.field("property").text(context),
                            }
                        });
                    }
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::CallExpression;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_useless_call_rule() {
        RuleTester::run(
            no_useless_call_rule(),
            rule_tests! {
                valid => [
                    // `this` binding is different.
                    "foo.apply(obj, 1, 2);",
                    "obj.foo.apply(null, 1, 2);",
                    "obj.foo.apply(otherObj, 1, 2);",
                    "a.b(x, y).c.foo.apply(a.b(x, z).c, 1, 2);",
                    "foo.apply(obj, [1, 2]);",
                    "obj.foo.apply(null, [1, 2]);",
                    "obj.foo.apply(otherObj, [1, 2]);",
                    "a.b(x, y).c.foo.apply(a.b(x, z).c, [1, 2]);",
                    "a.b.foo.apply(a.b.c, [1, 2]);",

                    // ignores variadic.
                    "foo.apply(null, args);",
                    "obj.foo.apply(obj, args);",

                    // ignores computed property.
                    "var call; foo[call](null, 1, 2);",
                    "var apply; foo[apply](null, [1, 2]);",

                    // ignores incomplete things.
                    "foo.call();",
                    "obj.foo.call();",
                    "foo.apply();",
                    "obj.foo.apply();",

                    // Optional chaining
                    {
                        code => "obj?.foo.bar.call(obj.foo, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 }
                    },

                    // Private members
                    {
                        code => "class C { #call; wrap(foo) { foo.#call(undefined, 1, 2); } }",
                        // parserOptions: { ecmaVersion: 2022 }
                    }
                ],
                invalid => [
                    // call.
                    {
                        code => "foo.call(undefined, 1, 2);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "foo.call(void 0, 1, 2);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "foo.call(null, 1, 2);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "obj.foo.call(obj, 1, 2);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "a.b.c.foo.call(a.b.c, 1, 2);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "a.b(x, y).c.foo.call(a.b(x, y).c, 1, 2);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },

                    // apply.
                    {
                        code => "foo.apply(undefined, [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "foo.apply(void 0, [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "foo.apply(null, [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "obj.foo.apply(obj, [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "a.b.c.foo.apply(a.b.c, [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "a.b(x, y).c.foo.apply(a.b(x, y).c, [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "[].concat.apply([ ], [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "[].concat.apply([\n/*empty*/\n], [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "abc.get(\"foo\", 0).concat.apply(abc . get(\"foo\",  0 ), [1, 2]);",
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "apply" },
                            type => CallExpression
                        }]
                    },

                    // Optional chaining
                    {
                        code => "foo.call?.(undefined, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unnecessary_call", data => { name => "call" } }]
                    },
                    {
                        code => "foo?.call(undefined, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unnecessary_call", data => { name => "call" } }]
                    },
                    {
                        code => "(foo?.call)(undefined, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unnecessary_call", data => { name => "call" } }]
                    },
                    {
                        code => "obj.foo.call?.(obj, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "obj?.foo.call(obj, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "(obj?.foo).call(obj, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "(obj?.foo.call)(obj, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "obj?.foo.bar.call(obj?.foo, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "(obj?.foo).bar.call(obj?.foo, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    },
                    {
                        code => "obj.foo?.bar.call(obj.foo, 1, 2);",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{
                            message_id => "unnecessary_call",
                            data => { name => "call" },
                            type => CallExpression
                        }]
                    }
                ]
            },
        )
    }
}
