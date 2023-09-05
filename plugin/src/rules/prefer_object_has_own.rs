use std::sync::Arc;

use squalid::OptionExt;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule, SkipOptionsBuilder,
};

use crate::{
    kind::{Identifier, MemberExpression, Object},
    scope::{ScopeManager, ScopeType},
    utils::ast_utils,
};

fn has_left_hand_object(node: Node, context: &QueryMatchContext) -> bool {
    let object = node.field("object");
    if object.kind() == Object
        && !object.has_non_comment_named_children(SupportedLanguage::Javascript)
    {
        return true;
    }

    let object_node_to_check = if object.kind() == MemberExpression
        && ast_utils::get_static_property_name(object, context).as_deref() == Some("prototype")
    {
        object.field("object")
    } else {
        object
    };

    if object_node_to_check.kind() == Identifier && object_node_to_check.text(context) == "Object" {
        return true;
    }

    false
}

pub fn prefer_object_has_own_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-object-has-own",
        languages => [Javascript],
        messages => [
            use_has_own => "Use 'Object.hasOwn()' instead of 'Object.prototype.hasOwnProperty.call()'.",
        ],
        fixable => true,
        listeners => [
            r#"
              (call_expression
                function: (member_expression
                  object: (member_expression)
                )
              ) @c
            "# => |node, context| {
                let callee = node.field("function");
                let callee_property_name = ast_utils::get_static_property_name(callee, context);
                let callee_object = callee.field("object");
                let object_property_name = ast_utils::get_static_property_name(callee_object, context);
                let is_object = has_left_hand_object(callee_object, context);

                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let scope = scope_manager.get_scope(node);
                let variable = ast_utils::get_variable_by_name(scope, "Object");

                if callee_property_name.as_deref() == Some("call") &&
                    object_property_name.as_deref() == Some("hasOwnProperty") &&
                    is_object && variable.matches(|variable| variable.scope().type_() == ScopeType::Global)
                {
                    context.report(violation! {
                        node => node,
                        message_id => "use_has_own",
                        fix => |fixer| {
                            if context.get_comments_inside(callee).count() > 0 {
                                return;
                            }

                            if context.maybe_get_token_before(
                                callee,
                                Some(SkipOptionsBuilder::<fn(Node) -> bool>::default()
                                    .include_comments(true)
                                    .build().unwrap())
                            ).matches(|token_just_before_node| {
                                token_just_before_node.range().end_byte == callee.range().start_byte &&
                                    !ast_utils::can_tokens_be_adjacent(token_just_before_node, "Object.hasOwn", context)
                            }) {
                                fixer.replace_text(callee, " Object.hasOwn");
                            } else {
                                fixer.replace_text(callee, "Object.hasOwn");
                            }
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use squalid::json_object;
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::get_instance_provider_factory;

    #[test]
    fn test_prefer_object_has_own_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            prefer_object_has_own_rule(),
            rule_tests! {
                valid => [
                    "Object",
                    "Object(obj, prop)",
                    "Object.hasOwnProperty",
                    "Object.hasOwnProperty(prop)",
                    "hasOwnProperty(obj, prop)",
                    "foo.hasOwnProperty(prop)",
                    "foo.hasOwnProperty(obj, prop)",
                    "Object.hasOwnProperty.call",
                    "foo.Object.hasOwnProperty.call(obj, prop)",
                    "foo.hasOwnProperty.call(obj, prop)",
                    "foo.call(Object.prototype.hasOwnProperty, Object.prototype.hasOwnProperty.call)",
                    "Object.foo.call(obj, prop)",
                    "Object.hasOwnProperty.foo(obj, prop)",
                    "Object.hasOwnProperty.call.foo(obj, prop)",
                    "Object[hasOwnProperty].call(obj, prop)",
                    "Object.hasOwnProperty[call](obj, prop)",
                    "class C { #hasOwnProperty; foo() { Object.#hasOwnProperty.call(obj, prop) } }",
                    "class C { #call; foo() { Object.hasOwnProperty.#call(obj, prop) } }",
                    "(Object) => Object.hasOwnProperty.call(obj, prop)", // not global Object
                    "Object.prototype",
                    "Object.prototype(obj, prop)",
                    "Object.prototype.hasOwnProperty",
                    "Object.prototype.hasOwnProperty(obj, prop)",
                    "Object.prototype.hasOwnProperty.call",
                    "foo.Object.prototype.hasOwnProperty.call(obj, prop)",
                    "foo.prototype.hasOwnProperty.call(obj, prop)",
                    "Object.foo.hasOwnProperty.call(obj, prop)",
                    "Object.prototype.foo.call(obj, prop)",
                    "Object.prototype.hasOwnProperty.foo(obj, prop)",
                    "Object.prototype.hasOwnProperty.call.foo(obj, prop)",
                    "Object.prototype.prototype.hasOwnProperty.call(a, b);",
                    "Object.hasOwnProperty.prototype.hasOwnProperty.call(a, b);",
                    "Object.prototype[hasOwnProperty].call(obj, prop)",
                    "Object.prototype.hasOwnProperty[call](obj, prop)",
                    "class C { #hasOwnProperty; foo() { Object.prototype.#hasOwnProperty.call(obj, prop) } }",
                    "class C { #call; foo() { Object.prototype.hasOwnProperty.#call(obj, prop) } }",
                    "Object[prototype].hasOwnProperty.call(obj, prop)",
                    "class C { #prototype; foo() { Object.#prototype.hasOwnProperty.call(obj, prop) } }",
                    "(Object) => Object.prototype.hasOwnProperty.call(obj, prop)", // not global Object
                    "({})",
                    "({}(obj, prop))",
                    "({}.hasOwnProperty)",
                    "({}.hasOwnProperty(prop))",
                    "({}.hasOwnProperty(obj, prop))",
                    "({}.hasOwnProperty.call)",
                    "({}).prototype.hasOwnProperty.call(a, b);",
                    "({}.foo.call(obj, prop))",
                    "({}.hasOwnProperty.foo(obj, prop))",
                    "({}[hasOwnProperty].call(obj, prop))",
                    "({}.hasOwnProperty[call](obj, prop))",
                    "({}).hasOwnProperty[call](object, property)",
                    "({})[hasOwnProperty].call(object, property)",
                    "class C { #hasOwnProperty; foo() { ({}.#hasOwnProperty.call(obj, prop)) } }",
                    "class C { #call; foo() { ({}.hasOwnProperty.#call(obj, prop)) } }",
                    "({ foo }.hasOwnProperty.call(obj, prop))", // object literal should be empty
                    "(Object) => ({}).hasOwnProperty.call(obj, prop)", // Object is shadowed, so Object.hasOwn cannot be used here
                    r#"
                    let obj = {};
                    Object.hasOwn(obj,"");
                    "#,
                    "const hasProperty = Object.hasOwn(object, property);",
                    "/* global Object: off */
                    ({}).hasOwnProperty.call(a, b);"
                ],
                invalid => [
                    {
                        code => "Object.hasOwnProperty.call(obj, 'foo')",
                        output => "Object.hasOwn(obj, 'foo')",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 39
                        }]
                    },
                    {
                        code => "Object.hasOwnProperty.call(obj, property)",
                        output => "Object.hasOwn(obj, property)",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 42
                        }]
                    },
                    {
                        code => "Object.prototype.hasOwnProperty.call(obj, 'foo')",
                        output => "Object.hasOwn(obj, 'foo')",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 49
                        }]
                    },
                    {
                        code => "({}).hasOwnProperty.call(obj, 'foo')",
                        output => "Object.hasOwn(obj, 'foo')",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 37
                        }]
                    },

                    //  prevent autofixing if there are any comments
                    {
                        code => "Object/* comment */.prototype.hasOwnProperty.call(a, b);",
                        output => None,
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 56
                        }]
                    },
                    {
                        code => "const hasProperty = Object.prototype.hasOwnProperty.call(object, property);",
                        output => "const hasProperty = Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 75
                        }]
                    },
                    {
                        code => "const hasProperty = (( Object.prototype.hasOwnProperty.call(object, property) ));",
                        output => "const hasProperty = (( Object.hasOwn(object, property) ));",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 24,
                            end_line => 1,
                            end_column => 78
                        }]
                    },
                    {
                        code => "const hasProperty = (( Object.prototype.hasOwnProperty.call ))(object, property);",
                        output => "const hasProperty = (( Object.hasOwn ))(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 81
                        }]
                    },
                    {
                        code => "const hasProperty = (( Object.prototype.hasOwnProperty )).call(object, property);",
                        output => "const hasProperty = Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 81
                        }]
                    },
                    {
                        code => "const hasProperty = (( Object.prototype )).hasOwnProperty.call(object, property);",
                        output => "const hasProperty = Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 81
                        }]
                    },
                    {
                        code => "const hasProperty = (( Object )).prototype.hasOwnProperty.call(object, property);",
                        output => "const hasProperty = Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 81
                        }]
                    },
                    {
                        code => "const hasProperty = {}.hasOwnProperty.call(object, property);",
                        output => "const hasProperty = Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 61
                        }]
                    },
                    {
                        code => "const hasProperty={}.hasOwnProperty.call(object, property);",
                        output => "const hasProperty=Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 19,
                            end_line => 1,
                            end_column => 59
                        }]
                    },
                    {
                        code => "const hasProperty = (( {}.hasOwnProperty.call(object, property) ));",
                        output => "const hasProperty = (( Object.hasOwn(object, property) ));",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 24,
                            end_line => 1,
                            end_column => 64
                        }]
                    },
                    {
                        code => "const hasProperty = (( {}.hasOwnProperty.call ))(object, property);",
                        output => "const hasProperty = (( Object.hasOwn ))(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 67
                        }]
                    },
                    {
                        code => "const hasProperty = (( {}.hasOwnProperty )).call(object, property);",
                        output => "const hasProperty = Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 67
                        }]
                    },
                    {
                        code => "const hasProperty = (( {} )).hasOwnProperty.call(object, property);",
                        output => "const hasProperty = Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 67
                        }]
                    },
                    {
                        code => "function foo(){return {}.hasOwnProperty.call(object, property)}",
                        output => "function foo(){return Object.hasOwn(object, property)}",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 23,
                            end_line => 1,
                            end_column => 63
                        }]
                    },

                    // https://github.com/eslint/eslint/pull/15346#issuecomment-991417335
                    {
                        code => "function foo(){return{}.hasOwnProperty.call(object, property)}",
                        output => "function foo(){return Object.hasOwn(object, property)}",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 22,
                            end_line => 1,
                            end_column => 62
                        }]
                    },
                    {
                        code => "function foo(){return/*comment*/{}.hasOwnProperty.call(object, property)}",
                        output => "function foo(){return/*comment*/Object.hasOwn(object, property)}",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 33,
                            end_line => 1,
                            end_column => 73
                        }]
                    },
                    {
                        code => "async function foo(){return await{}.hasOwnProperty.call(object, property)}",
                        output => "async function foo(){return await Object.hasOwn(object, property)}",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 34,
                            end_line => 1,
                            end_column => 74
                        }]
                    },
                    {
                        code => "async function foo(){return await/*comment*/{}.hasOwnProperty.call(object, property)}",
                        output => "async function foo(){return await/*comment*/Object.hasOwn(object, property)}",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 45,
                            end_line => 1,
                            end_column => 85
                        }]
                    },
                    {
                        code => "for (const x of{}.hasOwnProperty.call(object, property).toString());",
                        output => "for (const x of Object.hasOwn(object, property).toString());",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 16,
                            end_line => 1,
                            end_column => 56
                        }]
                    },
                    {
                        code => "for (const x of/*comment*/{}.hasOwnProperty.call(object, property).toString());",
                        output => "for (const x of/*comment*/Object.hasOwn(object, property).toString());",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 27,
                            end_line => 1,
                            end_column => 67
                        }]
                    },
                    {
                        code => "for (const x in{}.hasOwnProperty.call(object, property).toString());",
                        output => "for (const x in Object.hasOwn(object, property).toString());",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 16,
                            end_line => 1,
                            end_column => 56
                        }]
                    },
                    {
                        code => "for (const x in/*comment*/{}.hasOwnProperty.call(object, property).toString());",
                        output => "for (const x in/*comment*/Object.hasOwn(object, property).toString());",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 27,
                            end_line => 1,
                            end_column => 67
                        }]
                    },
                    {
                        code => "function foo(){return({}.hasOwnProperty.call)(object, property)}",
                        output => "function foo(){return(Object.hasOwn)(object, property)}",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 22,
                            end_line => 1,
                            end_column => 64
                        }]
                    },
                    {
                        code => "Object['prototype']['hasOwnProperty']['call'](object, property);",
                        output => "Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 64
                        }]
                    },
                    {
                        code => "Object[`prototype`][`hasOwnProperty`][`call`](object, property);",
                        output => "Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 64
                        }]
                    },
                    {
                        code => "Object['hasOwnProperty']['call'](object, property);",
                        output => "Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 51
                        }]
                    },
                    {
                        code => "Object[`hasOwnProperty`][`call`](object, property);",
                        output => "Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 51
                        }]
                    },
                    {
                        code => "({})['hasOwnProperty']['call'](object, property);",
                        output => "Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 49
                        }]
                    },
                    {
                        code => "({})[`hasOwnProperty`][`call`](object, property);",
                        output => "Object.hasOwn(object, property);",
                        errors => [{
                            message_id => "use_has_own",
                            line => 1,
                            column => 1,
                            end_line => 1,
                            end_column => 49
                        }]
                    }
                ]
            },
            get_instance_provider_factory(),
            json_object!({
                "ecma_version": 2022,
            }),
        )
    }
}
