use std::collections::HashMap;
use std::sync::Arc;

use tree_sitter_lint::{rule, violation, FromFileRunContextInstanceProviderFactory, Rule};

use crate::{
    ast_helpers::{get_method_definition_kind, is_class_member_static, MethodDefinitionKind},
    kind::MethodDefinition,
    utils::ast_utils,
};

macro_rules! continue_if_none {
    ($expr:expr) => {
        match $expr {
            Some(value) => value,
            None => continue,
        }
    };
}

#[derive(Default)]
struct Seen {
    init: bool,
    get: bool,
    set: bool,
}

#[derive(Default)]
struct StaticAndNonStaticSeen {
    non_static: Seen,
    static_: Seen,
}

type StateMap = HashMap<String, StaticAndNonStaticSeen>;

fn get_state(state_map: &mut StateMap, name: String, is_static: bool) -> &mut Seen {
    let entry = state_map.entry(name).or_default();
    if is_static {
        &mut entry.static_
    } else {
        &mut entry.non_static
    }
}

pub fn no_dupe_class_members_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-dupe-class-members",
        languages => [Javascript],
        messages => [
            unexpected => "Duplicate name '{{name}}'.",
        ],
        listeners => [
            r#"(
              (class_body
                member: ((_) @member (comment)* ";"? (comment)*)+
              ) @class_body
            )"# => |captures, context| {
                let mut state_map: StateMap = Default::default();

                for node in captures.get_all("member") {
                    let name = continue_if_none!(ast_utils::get_static_property_name(node, context));
                    let kind = (node.kind() == MethodDefinition).then(|| get_method_definition_kind(node, context));

                    if kind == Some(MethodDefinitionKind::Constructor) {
                        continue;
                    }

                    let state = get_state(
                        &mut state_map,
                        name.clone().into_owned(),
                        is_class_member_static(node, context)
                    );
                    let is_duplicate;

                    match kind {
                        Some(MethodDefinitionKind::Get) => {
                            is_duplicate = state.init || state.get;
                            state.get = true;
                        }
                        Some(MethodDefinitionKind::Set) => {
                            is_duplicate = state.init || state.set;
                            state.set = true;
                        }
                        _ => {
                            is_duplicate = state.init || state.get || state.set;
                            state.init = true;
                        }
                    }

                    if is_duplicate {
                        context.report(violation! {
                            node => node,
                            message_id => "unexpected",
                            data => {
                                name => name,
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
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_dupe_class_members_rule() {
        RuleTester::run(
            no_dupe_class_members_rule(),
            rule_tests! {
                valid => [
                    "class A { foo() {} bar() {} }",
                    "class A { static foo() {} foo() {} }",
                    "class A { get foo() {} set foo(value) {} }",
                    "class A { static foo() {} get foo() {} set foo(value) {} }",
                    "class A { foo() { } } class B { foo() { } }",
                    "class A { [foo]() {} foo() {} }",
                    "class A { 'foo'() {} 'bar'() {} baz() {} }",
                    "class A { *'foo'() {} *'bar'() {} *baz() {} }",
                    "class A { get 'foo'() {} get 'bar'() {} get baz() {} }",
                    "class A { 1() {} 2() {} }",
                    "class A { ['foo']() {} ['bar']() {} }",
                    "class A { [`foo`]() {} [`bar`]() {} }",
                    "class A { [12]() {} [123]() {} }",
                    "class A { [1.0]() {} ['1.0']() {} }",
                    "class A { [0x1]() {} [`0x1`]() {} }",
                    "class A { [null]() {} ['']() {} }",
                    "class A { get ['foo']() {} set ['foo'](value) {} }",
                    "class A { ['foo']() {} static ['foo']() {} }",

                    // computed "constructor" key doesn't create constructor
                    "class A { ['constructor']() {} constructor() {} }",
                    "class A { 'constructor'() {} [`constructor`]() {} }",
                    "class A { constructor() {} get [`constructor`]() {} }",
                    "class A { 'constructor'() {} set ['constructor'](value) {} }",

                    // not assumed to be statically-known values
                    "class A { ['foo' + '']() {} ['foo']() {} }",
                    "class A { [`foo${''}`]() {} [`foo`]() {} }",
                    "class A { [-1]() {} ['-1']() {} }",

                    // not supported by this rule
                    "class A { [foo]() {} [foo]() {} }",

                    // private and public
                    "class A { foo; static foo; }",
                    "class A { foo; #foo; }",
                    "class A { '#foo'; #foo; }"
                ],
                invalid => [
                    {
                        code => "class A { foo() {} foo() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 20, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "!class A { foo() {} foo() {} };",
                        errors => [
                            { type => "method_definition", line => 1, column => 21, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { 'foo'() {} 'foo'() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 22, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { 10() {} 1e1() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 19, message_id => "unexpected", data => { name => "10" } }
                        ]
                    },
                    {
                        code => "class A { ['foo']() {} ['foo']() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 24, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { static ['foo']() {} static foo() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 31, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { set 'foo'(value) {} set ['foo'](val) {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 31, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { ''() {} ['']() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 19, message_id => "unexpected", data => { name => "" } }
                        ]
                    },
                    {
                        code => "class A { [`foo`]() {} [`foo`]() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 24, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { static get [`foo`]() {} static get ['foo']() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 35, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { foo() {} [`foo`]() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 20, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { get [`foo`]() {} 'foo'() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 28, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { static 'foo'() {} static [`foo`]() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 29, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { ['constructor']() {} ['constructor']() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 32, message_id => "unexpected", data => { name => "constructor" } }
                        ]
                    },
                    {
                        code => "class A { static [`constructor`]() {} static constructor() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 39, message_id => "unexpected", data => { name => "constructor" } }
                        ]
                    },
                    {
                        code => "class A { static constructor() {} static 'constructor'() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 35, message_id => "unexpected", data => { name => "constructor" } }
                        ]
                    },
                    {
                        code => "class A { [123]() {} [123]() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 22, message_id => "unexpected", data => { name => "123" } }
                        ]
                    },
                    {
                        code => "class A { [0x10]() {} 16() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 23, message_id => "unexpected", data => { name => "16" } }
                        ]
                    },
                    {
                        code => "class A { [100]() {} [1e2]() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 22, message_id => "unexpected", data => { name => "100" } }
                        ]
                    },
                    {
                        code => "class A { [123.00]() {} [`123`]() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 25, message_id => "unexpected", data => { name => "123" } }
                        ]
                    },
                    {
                        code => "class A { static '65'() {} static [0o101]() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 28, message_id => "unexpected", data => { name => "65" } }
                        ]
                    },
                    {
                        code => "class A { [123n]() {} 123() {} }",
                        /*parserOptions => { ecmaVersion => 2020 }*/
                        errors => [
                            { type => "method_definition", line => 1, column => 23, message_id => "unexpected", data => { name => "123" } }
                        ]
                    },
                    {
                        code => "class A { [null]() {} 'null'() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 23, message_id => "unexpected", data => { name => "null" } }
                        ]
                    },
                    {
                        code => "class A { foo() {} foo() {} foo() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 20, message_id => "unexpected", data => { name => "foo" } },
                            { type => "method_definition", line => 1, column => 29, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { static foo() {} static foo() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 27, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { foo() {} get foo() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 20, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { set foo(value) {} foo() {} }",
                        errors => [
                            { type => "method_definition", line => 1, column => 29, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    },
                    {
                        code => "class A { foo; foo; }",
                        errors => [
                            { type => "field_definition", line => 1, column => 16, message_id => "unexpected", data => { name => "foo" } }
                        ]
                    }
                ]
            },
        )
    }
}
