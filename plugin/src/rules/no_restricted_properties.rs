use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::return_if_none, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::{ast_helpers::NodeExtJs, kind::Identifier, utils::ast_utils};

// https://stackoverflow.com/a/73604693/732366
#[derive(Clone, Deserialize)]
#[serde(untagged)]
enum RestrictedCallSpec {
    HasObject {
        object: String,
        property: Option<String>,
        message: Option<String>,
    },
    HasProperty {
        object: Option<String>,
        property: String,
        message: Option<String>,
    },
}

impl RestrictedCallSpec {
    pub fn object(&self) -> Option<&str> {
        match self {
            RestrictedCallSpec::HasObject { object, .. } => Some(object),
            RestrictedCallSpec::HasProperty { object, .. } => object.as_deref(),
        }
    }

    pub fn property(&self) -> Option<&str> {
        match self {
            RestrictedCallSpec::HasObject { property, .. } => property.as_deref(),
            RestrictedCallSpec::HasProperty { property, .. } => Some(property),
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            RestrictedCallSpec::HasObject { message, .. } => message.as_deref(),
            RestrictedCallSpec::HasProperty { message, .. } => message.as_deref(),
        }
    }
}

type RestrictedProperties = HashMap<String, HashMap<String, Option<String>>>;
type GloballyRestrictedObjects = HashMap<String, Option<String>>;
type GloballyRestrictedProperties = HashMap<String, Option<String>>;

fn get_restricted_properties(restricted_calls: &[RestrictedCallSpec]) -> RestrictedProperties {
    restricted_calls
        .into_iter()
        .filter(|restricted_call| {
            restricted_call.object().is_some() && restricted_call.property().is_some()
        })
        .fold(
            Default::default(),
            |mut restricted_properties, restricted_call| {
                restricted_properties
                    .entry(restricted_call.object().unwrap().to_owned())
                    .or_default()
                    .insert(
                        restricted_call.property().unwrap().to_owned(),
                        restricted_call.message().map(ToOwned::to_owned),
                    );
                restricted_properties
            },
        )
}

fn get_globally_restricted_objects(
    restricted_calls: &[RestrictedCallSpec],
) -> GloballyRestrictedObjects {
    restricted_calls
        .into_iter()
        .filter(|restricted_call| {
            restricted_call.object().is_some() && restricted_call.property().is_none()
        })
        .fold(
            Default::default(),
            |mut globally_restricted_objects, restricted_call| {
                globally_restricted_objects.insert(
                    restricted_call.object().unwrap().to_owned(),
                    restricted_call.message().map(ToOwned::to_owned),
                );
                globally_restricted_objects
            },
        )
}

fn get_globally_restricted_properties(
    restricted_calls: &[RestrictedCallSpec],
) -> GloballyRestrictedProperties {
    restricted_calls
        .into_iter()
        .filter(|restricted_call| {
            restricted_call.object().is_none() && restricted_call.property().is_some()
        })
        .fold(
            Default::default(),
            |mut globally_restricted_properties, restricted_call| {
                globally_restricted_properties.insert(
                    restricted_call.property().unwrap().to_owned(),
                    restricted_call.message().map(ToOwned::to_owned),
                );
                globally_restricted_properties
            },
        )
}

fn check_property_access(
    node: Node,
    object_name: Option<&str>,
    property_name: Option<&str>,
    context: &QueryMatchContext,
    restricted_properties: &RestrictedProperties,
    globally_restricted_objects: &GloballyRestrictedObjects,
    globally_restricted_properties: &GloballyRestrictedProperties,
) {
    let property_name = return_if_none!(property_name);

    let matched_object = object_name.and_then(|object_name| restricted_properties.get(object_name));
    let matched_object_property = matched_object
        .and_then(|matched_object| matched_object.get(property_name))
        .or_else(|| {
            object_name.and_then(|object_name| globally_restricted_objects.get(object_name))
        });
    let global_matched_property = globally_restricted_properties.get(property_name);

    if let Some(matched_object_property) = matched_object_property {
        let message = matched_object_property.clone().unwrap_or_default();

        context.report(violation! {
            node => node,
            message_id => "restricted_object_property",
            data => {
                object_name => object_name.unwrap(),
                property_name => property_name,
                message => message,
            }
        });
    } else if let Some(global_matched_property) = global_matched_property {
        let message = global_matched_property.clone().unwrap_or_default();

        context.report(violation! {
            node => node,
            message_id => "restricted_property",
            data => {
                property_name => property_name,
                message => message,
            }
        });
    }
}

pub fn no_restricted_properties_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-restricted-properties",
        languages => [Javascript],
        messages => [
            restricted_object_property => "'{{object_name}}.{{property_name}}' is restricted from being used.{{message}}",
            restricted_property => "'{{property_name}}' is restricted from being used.{{message}}",
        ],
        options_type => Vec<RestrictedCallSpec>,
        state => {
            [per-run]
            is_configuration_empty: bool = options.is_empty(),
            restricted_properties: RestrictedProperties =
                get_restricted_properties(&options),
            globally_restricted_objects: GloballyRestrictedObjects =
                get_globally_restricted_objects(&options),
            globally_restricted_properties: GloballyRestrictedProperties =
                get_globally_restricted_properties(&options),
        },
        listeners => [
            r#"
              (member_expression) @c
              (subscript_expression) @c
            "# => |node, context| {
                if self.is_configuration_empty {
                    return;
                }

                check_property_access(
                    node,
                    node.child_by_field_name("object")
                        .filter(|object| object.kind() == Identifier)
                        .map(|object| {
                            object.text(context)
                        })
                        .as_deref(),
                    ast_utils::get_static_property_name(node, context).as_deref(),
                    context,
                    &self.restricted_properties,
                    &self.globally_restricted_objects,
                    &self.globally_restricted_properties,
                );
            },
            r#"
              (variable_declarator
                name: (object_pattern)
                value: (identifier)
              ) @c
            "# => |node, context| {
                if self.is_configuration_empty {
                    return;
                }

                let object_name = node.field("value").text(context);

                let node_name = node.field("name");
                node_name.non_comment_named_children().for_each(|property| {
                    check_property_access(
                        node_name,
                        Some(&object_name),
                        ast_utils::get_static_property_name(property, context).as_deref(),
                        context,
                        &self.restricted_properties,
                        &self.globally_restricted_objects,
                        &self.globally_restricted_properties,
                    );
                });
            },
            r#"
              (assignment_expression
                left: (object_pattern)
                right: (identifier)
              ) @c
              (assignment_pattern
                left: (object_pattern)
                right: (identifier)
              ) @c
            "# => |node, context| {
                if self.is_configuration_empty {
                    return;
                }

                let object_name = node.field("right").text(context);

                let node_left = node.field("left");
                node_left.non_comment_named_children().for_each(|property| {
                    check_property_access(
                        node_left,
                        Some(&object_name),
                        ast_utils::get_static_property_name(property, context).as_deref(),
                        context,
                        &self.restricted_properties,
                        &self.globally_restricted_objects,
                        &self.globally_restricted_properties,
                    );
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    use crate::kind::{MemberExpression, ObjectPattern, SubscriptExpression};

    #[test]
    fn test_no_restricted_properties_rule() {
        RuleTester::run(
            no_restricted_properties_rule(),
            rule_tests! {
                valid => [
                    {
                        code => "someObject.someProperty",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty"
                        }]
                    }, {
                        code => "anotherObject.disallowedProperty",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty"
                        }]
                    }, {
                        code => "someObject.someProperty()",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty"
                        }]
                    }, {
                        code => "anotherObject.disallowedProperty()",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty"
                        }]
                    }, {
                        code => "anotherObject.disallowedProperty()",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty",
                            message => "Please use someObject.allowedProperty instead."
                        }]
                    }, {
                        code => "anotherObject['disallowedProperty']()",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty"
                        }]
                    }, {
                        code => "obj.toString",
                        options => [{
                            object => "obj",
                            property => "__proto__"
                        }]
                    }, {
                        code => "toString.toString",
                        options => [{
                            object => "obj",
                            property => "foo"
                        }]
                    }, {
                        code => "obj.toString",
                        options => [{
                            object => "obj",
                            property => "foo"
                        }]
                    }, {
                        code => "foo.bar",
                        options => [{
                            property => "baz"
                        }]
                    }, {
                        code => "foo.bar",
                        options => [{
                            object => "baz"
                        }]
                    }, {
                        code => "foo()",
                        options => [{
                            object => "foo"
                        }]
                    }, {
                        code => "foo;",
                        options => [{
                            object => "foo"
                        }]
                    }, {
                        code => "foo[/(?<zero>0)/]",
                        options => [{
                            property => "null"
                        }],
                        // parserOptions: { ecmaVersion: 2018 }
                    }, {
                        code => "let bar = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let {baz: bar} = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let {unrelated} = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let {baz: {bar: qux}} = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let {bar} = foo.baz;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let {baz: bar} = foo;",
                        options => [{ property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let baz; ({baz: bar} = foo)",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let bar;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let bar; ([bar = 5] = foo);",
                        options => [{ object => "foo", property => "1" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "function qux({baz: bar} = foo) {}",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let [bar, baz] = foo;",
                        options => [{ object => "foo", property => "1" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let [, bar] = foo;",
                        options => [{ object => "foo", property => "0" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let [, bar = 5] = foo;",
                        options => [{ object => "foo", property => "1" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "let bar; ([bar = 5] = foo);",
                        options => [{ object => "foo", property => "0" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "function qux([bar] = foo) {}",
                        options => [{ object => "foo", property => "0" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "function qux([, bar] = foo) {}",
                        options => [{ object => "foo", property => "0" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "function qux([, bar] = foo) {}",
                        options => [{ object => "foo", property => "1" }],
                        // parserOptions: { ecmaVersion: 6 }
                    }, {
                        code => "class C { #foo; foo() { this.#foo; } }",
                        options => [{ property => "#foo" }],
                        // parserOptions: { ecmaVersion: 2022 }
                    }
                ],
                invalid => [
                    {
                        code => "someObject.disallowedProperty",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty"
                        }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "someObject",
                                property_name => "disallowedProperty",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "someObject.disallowedProperty",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty",
                            message => "Please use someObject.allowedProperty instead."
                        }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "someObject",
                                property_name => "disallowedProperty",
                                message => "Please use someObject.allowedProperty instead."
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "someObject.disallowedProperty; anotherObject.anotherDisallowedProperty()",
                        options => [{
                            object => "someObject",
                            property => "disallowedProperty"
                        }, {
                            object => "anotherObject",
                            property => "anotherDisallowedProperty"
                        }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "someObject",
                                property_name => "disallowedProperty",
                                message => ""
                            },
                            type => MemberExpression
                        }, {
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "anotherObject",
                                property_name => "anotherDisallowedProperty",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo.__proto__",
                        options => [{
                            property => "__proto__",
                            message => "Please use Object.getPrototypeOf instead."
                        }],
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "__proto__",
                                message => "Please use Object.getPrototypeOf instead."
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo['__proto__']",
                        options => [{
                            property => "__proto__",
                            message => "Please use Object.getPrototypeOf instead."
                        }],
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "__proto__",
                                message => "Please use Object.getPrototypeOf instead."
                            },
                            type => SubscriptExpression
                        }]
                    }, {
                        code => "foo.bar.baz;",
                        options => [{ object => "foo" }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo.bar();",
                        options => [{ object => "foo" }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo.bar.baz();",
                        options => [{ object => "foo" }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo.bar.baz;",
                        options => [{ property => "bar" }],
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "bar",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo.bar();",
                        options => [{ property => "bar" }],
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "bar",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo.bar.baz();",
                        options => [{ property => "bar" }],
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "bar",
                                message => ""
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "foo[/(?<zero>0)/]",
                        options => [{ property => "/(?<zero>0)/" }],
                        // parserOptions: { ecmaVersion: 2018 },
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "/(?<zero>0)/",
                                message => ""
                            },
                            type => SubscriptExpression
                        }]
                    }, {
                        code => "require.call({}, 'foo')",
                        options => [{
                            object => "require",
                            message => "Please call require() directly."
                        }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "require",
                                property_name => "call",
                                message => "Please call require() directly."
                            },
                            type => MemberExpression
                        }]
                    }, {
                        code => "require['resolve']",
                        options => [{
                            object => "require"
                        }],
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "require",
                                property_name => "resolve",
                                message => ""
                            },
                            type => SubscriptExpression
                        }]
                    }, {
                        code => "let {bar} = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let {bar: baz} = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let {'bar': baz} = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let {bar: {baz: qux}} = foo;",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let {bar} = foo;",
                        options => [{ object => "foo" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let {bar: baz} = foo;",
                        options => [{ object => "foo" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let {bar} = foo;",
                        options => [{ property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let bar; ({bar} = foo);",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "let bar; ({bar: baz = 1} = foo);",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "function qux({bar} = foo) {}",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "function qux({bar: baz} = foo) {}",
                        options => [{ object => "foo", property => "bar" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "foo",
                                property_name => "bar",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "var {['foo']: qux, bar} = baz",
                        options => [{ object => "baz", property => "foo" }],
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "restricted_object_property",
                            data => {
                                object_name => "baz",
                                property_name => "foo",
                                message => ""
                            },
                            type => ObjectPattern
                        }]
                    }, {
                        code => "obj['#foo']",
                        options => [{ property => "#foo" }],
                        errors => [{
                            message_id => "restricted_property",
                            data => {
                                property_name => "#foo",
                                message => ""
                            },
                            type => SubscriptExpression
                        }]
                    }
                ]
            },
        )
    }
}
