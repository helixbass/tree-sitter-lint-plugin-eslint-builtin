use std::borrow::Cow;

use tree_sitter_lint::{tree_sitter::Node, NodeExt, QueryMatchContext};

use crate::scope::Scope;

pub fn get_property_name<'a>(
    node: Node<'a>,
    initial_scope: Option<Scope<'a, '_>>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<Cow<'a, str>> {
    None
    // match node.kind() {
    //     SubscriptExpression => get_string_if_constant(node.field("index"), initial_scope, context),
    //     MemberExpression => {
    //         let property = node.field("property");
    //         if property.kind() == PrivatePropertyIdentifier {
    //             return None;
    //         }
    //         Some(property.text(context))
    //     }
    //     Pair => {
    //         let key = node.field("key");
    //         if key.kind() == ComputedPropertyName {
    //             return get_string_if_constant(key, initial_scope, context);
    //         }
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap};

    use speculoos::prelude::*;
    use tree_sitter_lint::{rule, rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_get_property_name() {
        // TODO: is there a nicer pattern for doing this type of "rule that closes over/"emits"
        // some value" type of pattern?
        thread_local! {
            static ACTUALS: RefCell<HashMap<String, Option<String>>> = Default::default();
        }

        let rule = rule! {
            name => "test-get-property-name",
            languages => [Javascript],
            listeners => [
                r#"
                  (pair) @c
                  (field_definition) @c
                  (method_definition) @c
                  (member_expression) @c
                  (subscript_expression) @c
                "# => |node, context| {
                    let actual = get_property_name(node, None, context);
                    ACTUALS.with(|actuals| {
                        actuals.borrow_mut().insert(
                            context.file_run_context.file_contents.into(),
                            actual.map(Cow::into_owned),
                        );
                    });
                },
            ],
        };

        for (code, expected) in [
            ("a.b", Some("b")),
            ("a['b']", Some("b")),
            ("a[`b`]", Some("b")),
            ("a[100]", Some("100")),
            ("a[b]", None),
            ("a['a' + 'b']", Some("ab")),
            ("a[tag`b`]", None),
            ("a[`${b}`]", None),
            ("({b: 1})", Some("b")),
            ("({0x10: 1})", Some("16")),
            ("({'foo': 1})", Some("foo")),
            ("({b() {}})", Some("b")),
            ("({get b() {}})", Some("b")),
            ("({['b']: 1})", Some("b")),
            ("({['b']() {}})", Some("b")),
            ("({[`b`]: 1})", Some("b")),
            ("({[100]: 1})", Some("100")),
            ("({[b]: 1})", None),
            ("({['a' + 'b']: 1})", Some("ab")),
            ("({[tag`b`]: 1})", None),
            ("({[`${b}`]: 1})", None),
            ("(class {b() {}})", Some("b")),
            ("(class {get b() {}})", Some("b")),
            ("(class {['b']() {}})", Some("b")),
            ("(class {[100]() {}})", Some("100")),
            ("(class {[b]() {}})", None),
            ("(class {['a' + 'b']() {}})", Some("ab")),
            ("(class {[tag`b`]() {}})", None),
            ("(class {[`${b}`]() {}})", None),
            ("(class { x })", Some("x")),
            ("(class { static x })", Some("x")),
            ("(class { #x })", None),
            ("(class { get #x() {} })", None),
            ("(class { #x() {} })", None),
            ("(class { static #x })", None),
            ("(class { static get #x() {} })", None),
            ("(class { static #x() {} })", None),
            ("(class { #x; fn() {this.#x} })", None),
            ("(class { #x; fn() {this.x} })", Some("x")),
        ] {
            RuleTester::run(
                rule.clone(),
                rule_tests! {
                    valid => [
                        { code => code }
                    ],
                    invalid => [],
                },
            );
            ACTUALS.with(|actuals| {
                let actuals = actuals.borrow();
                assert_that!(actuals[code]).is_equal_to(expected.map(ToOwned::to_owned));
            });
        }
    }
}
