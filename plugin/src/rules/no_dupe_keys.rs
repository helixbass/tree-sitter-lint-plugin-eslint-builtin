use std::{borrow::Cow, collections::HashMap, sync::Arc};

use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, Rule};

use crate::{
    ast_helpers::{get_object_property_kind, ObjectPropertyKind},
    continue_if_none,
    kind::{MethodDefinition, Pair, ShorthandPropertyIdentifier},
    utils::ast_utils,
};

#[derive(Default)]
struct GetOrSet {
    get: bool,
    set: bool,
}

struct ObjectInfo<'a> {
    node: Node<'a>,
    properties: HashMap<Cow<'a, str>, GetOrSet>,
}

impl<'a> ObjectInfo<'a> {
    pub fn new(node: Node<'a>) -> Self {
        Self {
            node,
            properties: Default::default(),
        }
    }

    pub fn is_property_defined(&self, name: &str, kind: ObjectPropertyKind) -> bool {
        self.properties.get(name).matches(|entry| {
            matches!(kind, ObjectPropertyKind::Init | ObjectPropertyKind::Get) && entry.get
                || matches!(kind, ObjectPropertyKind::Init | ObjectPropertyKind::Set) && entry.set
        })
    }

    pub fn define_property(&mut self, name: Cow<'a, str>, kind: ObjectPropertyKind) {
        let entry = self.properties.entry(name).or_default();
        if matches!(kind, ObjectPropertyKind::Init | ObjectPropertyKind::Get) {
            entry.get = true;
        }
        if matches!(kind, ObjectPropertyKind::Init | ObjectPropertyKind::Set) {
            entry.set = true;
        }
    }
}

pub fn no_dupe_keys_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-dupe-keys",
        languages => [Javascript],
        messages => [
            unexpected => "Duplicate key '{{name}}'.",
        ],
        listeners => [
            r#"(
              (object) @c
            )"# => |node, context| {
                let mut info = ObjectInfo::new(node);

                let mut cursor = node.walk();
                for property in node.named_children(&mut cursor).filter(|property| {
                    [Pair, MethodDefinition, ShorthandPropertyIdentifier].contains(&property.kind())
                }) {
                    let name = continue_if_none!(ast_utils::get_static_property_name(property, context));

                    let kind = get_object_property_kind(property, context);

                    if info.is_property_defined(&name, kind) {
                        context.report(violation! {
                            node => info.node,
                            range => property.range(),
                            message_id => "unexpected",
                            data => {
                                name => name.clone().into_owned(),
                            }
                        });
                    }

                    info.define_property(name, kind);
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_dupe_keys_rule() {
        RuleTester::run(
            no_dupe_keys_rule(),
            rule_tests! {
                valid => [
                    "var foo = { __proto__: 1, two: 2};",
                    "var x = { foo: 1, bar: 2 };",
                    "var x = { '': 1, bar: 2 };",
                    "var x = { '': 1, ' ': 2 };",
                    { code => "var x = { '': 1, [null]: 2 };", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var x = { '': 1, [a]: 2 };", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var x = { [a]: 1, [a]: 2 };", /*parserOptions: { ecmaVersion: 6 }*/ },
                    "+{ get a() { }, set a(b) { } };",
                    { code => "var x = { a: b, [a]: b };", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var x = { a: b, ...c }", /*parserOptions: { ecmaVersion: 2018 }*/ },
                    { code => "var x = { get a() {}, set a (value) {} };", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var x = { a: 1, b: { a: 2 } };", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var x = ({ null: 1, [/(?<zero>0)/]: 2 })", /*parserOptions: { ecmaVersion: 2018 }*/ },
                    { code => "var {a, a} = obj", /*parserOptions: { ecmaVersion: 6 }*/ },
                    "var x = { 012: 1, 12: 2 };",
                    { code => "var x = { 1_0: 1, 1: 2 };", /*parserOptions: { ecmaVersion: 2021 }*/ }
                ],
                invalid => [
                    { code => "var x = { a: b, ['a']: b };", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected", data => { name => "a" }, type => "object" }] },
                    { code => "var x = { y: 1, y: 2 };", errors => [{ message_id => "unexpected", data => { name => "y" }, type => "object" }] },
                    { code => "var x = { '': 1, '': 2 };", errors => [{ message_id => "unexpected", data => { name => "" }, type => "object" }] },
                    { code => "var x = { '': 1, [``]: 2 };", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected", data => { name => "" }, type => "object" }] },
                    { code => "var foo = { 0x1: 1, 1: 2};", errors => [{ message_id => "unexpected", data => { name => "1" }, type => "object" }] },
                    { code => "var x = { 012: 1, 10: 2 };", errors => [{ message_id => "unexpected", data => { name => "10" }, type => "object" }] },
                    { code => "var x = { 0b1: 1, 1: 2 };", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected", data => { name => "1" }, type => "object" }] },
                    { code => "var x = { 0o1: 1, 1: 2 };", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected", data => { name => "1" }, type => "object" }] },
                    { code => "var x = { 1n: 1, 1: 2 };", /*parserOptions: { ecmaVersion: 2020 }*/ errors => [{ message_id => "unexpected", data => { name => "1" }, type => "object" }] },
                    { code => "var x = { 1_0: 1, 10: 2 };", /*parserOptions: { ecmaVersion: 2021 }*/ errors => [{ message_id => "unexpected", data => { name => "10" }, type => "object" }] },
                    { code => "var x = { \"z\": 1, z: 2 };", errors => [{ message_id => "unexpected", data => { name => "z" }, type => "object" }] },
                    { code => "var foo = {\n  bar: 1,\n  bar: 1,\n}", errors => [{ message_id => "unexpected", data => { name => "bar" }, line => 3, column => 3 }] },
                    { code => "var x = { a: 1, get a() {} };", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected", data => { name => "a" }, type => "object" }] },
                    { code => "var x = { a: 1, set a(value) {} };", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected", data => { name => "a" }, type => "object" }] },
                    { code => "var x = { a: 1, b: { a: 2 }, get b() {} };", /*parserOptions: { ecmaVersion: 6 }*/ errors => [{ message_id => "unexpected", data => { name => "b" }, type => "object" }] },
                    { code => "var x = ({ '/(?<zero>0)/': 1, [/(?<zero>0)/]: 2 })", /*parserOptions: { ecmaVersion: 2018 }*/ errors => [{ message_id => "unexpected", data => { name => "/(?<zero>0)/" }, type => "object" }] }
                ]
            },
        )
    }
}
