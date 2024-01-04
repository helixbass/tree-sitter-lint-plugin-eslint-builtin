use std::sync::Arc;

use itertools::Itertools;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::get_call_expression_arguments,
    kind::{Array, MemberExpression, SpreadElement},
    utils::ast_utils,
};

fn is_variadic_apply_calling(node: Node, context: &QueryMatchContext) -> bool {
    ast_utils::is_specific_member_access(
        node.field("function"),
        Option::<&str>::None,
        Some("apply"),
        context,
    ) && get_call_expression_arguments(node).matches(|arguments| {
        let arguments = arguments.collect_vec();
        arguments.len() == 2 && !matches!(arguments[1].kind(), Array | SpreadElement)
    })
}

fn is_valid_this_arg<'a>(
    expected_this: Option<Node<'a>>,
    this_arg: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> bool {
    let Some(expected_this) = expected_this else {
        return ast_utils::is_null_or_undefined(this_arg);
    };
    ast_utils::equal_tokens(expected_this, this_arg, context)
}

pub fn prefer_spread_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-spread",
        languages => [Javascript],
        messages => [
            prefer_spread => "Use the spread operator instead of '.apply()'.",
        ],
        listeners => [
            r#"
              (call_expression) @c
            "# => |node, context| {
                if !is_variadic_apply_calling(node, context) {
                    return;
                }

                let applied = node.field("function").field("object");
                let expected_this = (applied.kind() == MemberExpression).then(|| {
                    applied.field("object")
                });
                let this_arg = get_call_expression_arguments(node).unwrap().next().unwrap();

                if is_valid_this_arg(expected_this, this_arg, context) {
                    context.report(violation! {
                        node => node,
                        message_id => "prefer_spread",
                    });
                }
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
        let errors = vec![RuleTestExpectedErrorBuilder::default()
            .message_id("prefer_spread")
            .type_(CallExpression)
            .build()
            .unwrap()];

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
