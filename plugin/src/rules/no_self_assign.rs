use std::{iter, sync::Arc};

use itertools::Itertools;
use serde::Deserialize;
use squalid::{regex, OptionExt};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{get_comma_separated_optional_non_comment_named_children, NodeExtJs},
    kind::{
        Array, ArrayPattern, Identifier, MemberExpression, Object, ObjectPattern, Pair,
        PairPattern, RestPattern, ShorthandPropertyIdentifier, ShorthandPropertyIdentifierPattern,
        SpreadElement, SubscriptExpression,
    },
    utils::ast_utils,
};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    props: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { props: true }
    }
}

fn report<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
    context.report(violation! {
        node => node,
        message_id => "self_assignment",
        data => {
            name => regex!(r#"\s+"#).replace_all(&node.text(context), ""),
        }
    });
}

fn each_self_assignment<'a>(
    left: Node<'a>,
    right: Node<'a>,
    props: bool,
    context: &QueryMatchContext<'a, '_>,
) {
    let left = left.skip_parentheses();
    let right = right.skip_parentheses();
    if [Identifier, ShorthandPropertyIdentifierPattern].contains(&left.kind())
        && [Identifier, ShorthandPropertyIdentifier].contains(&right.kind())
        && left.text(context) == right.text(context)
    {
        report(right, context);
        return;
    }

    if left.kind() == ArrayPattern && right.kind() == Array {
        let num_right_elements =
            get_comma_separated_optional_non_comment_named_children(right).count();
        for (index, (left_element, right_element)) in iter::zip(
            get_comma_separated_optional_non_comment_named_children(left),
            get_comma_separated_optional_non_comment_named_children(right),
        )
        .enumerate()
        {
            if left_element.matches(|left_element| left_element.kind() == RestPattern)
                && index < num_right_elements - 1
            {
                break;
            }

            if let (Some(left_element), Some(right_element)) = (left_element, right_element) {
                each_self_assignment(left_element, right_element, props, context);
            }

            if right_element.matches(|right_element| right_element.kind() == SpreadElement) {
                break;
            }
        }
        return;
    }

    if left.kind() == RestPattern && right.kind() == SpreadElement {
        each_self_assignment(
            left.first_non_comment_named_child(),
            right.first_non_comment_named_child(),
            props,
            context,
        );
        return;
    }

    if left.kind() == ObjectPattern && right.kind() == Object {
        let right_properties =
            get_comma_separated_optional_non_comment_named_children(right).collect_vec();
        if right_properties.is_empty() {
            return;
        }
        let left_properties =
            get_comma_separated_optional_non_comment_named_children(left).collect_vec();
        let right_spread_index = right_properties
            .iter()
            .enumerate()
            .rev()
            .find(|(_, right_property)| {
                right_property.matches(|right_property| right_property.kind() == SpreadElement)
            })
            .map(|(index, _)| index);
        // TODO: this looks weird that it's doing "combinations" of these two
        // (vs "pair-wise" (?))?
        for left_property in left_properties {
            for &right_property in right_properties.iter().skip(
                right_spread_index.map_or_default(|right_spread_index| right_spread_index + 1),
            ) {
                if let (Some(left_property), Some(right_property)) = (left_property, right_property)
                {
                    each_self_assignment(left_property, right_property, props, context);
                }
            }
        }
        return;
    }

    if left.kind() == PairPattern && right.kind() == Pair {
        let left_name = ast_utils::get_static_property_name(left, context);

        if left_name.matches(|left_name| {
            Some(left_name) == ast_utils::get_static_property_name(right, context)
        }) {
            each_self_assignment(left.field("value"), right.field("value"), props, context);
        }
        return;
    }

    if props
        && [MemberExpression, SubscriptExpression].contains(&left.kind())
        && [MemberExpression, SubscriptExpression].contains(&right.kind())
        && ast_utils::is_same_reference(left, right, None, context)
    {
        report(right, context);
    }
}

pub fn no_self_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-self-assign",
        languages => [Javascript],
        messages => [
            self_assignment => "'{{name}}' is assigned to itself.",
        ],
        options_type => Options,
        state => {
            [per-run]
            props: bool = options.props,
        },
        listeners => [
            r#"
              (assignment_expression) @c
              (augmented_assignment_expression
                operator: [
                  "&&="
                  "||="
                  "??="
                ]
              ) @c
            "# => |node, context| {
                each_self_assignment(
                    node.field("left"),
                    node.field("right"),
                    self.props,
                    context,
                )
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_self_assign_rule() {
        RuleTester::run(
            no_self_assign_rule(),
            rule_tests! {
                valid => [
                    "var a = a",
                    "a = b",
                    "a += a",
                    "a = +a",
                    "a = [a]",
                    "a &= a",
                    "a |= a",
                    { code => "let a = a", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "const a = a", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[a] = a", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[a = 1] = [a]", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[a, b] = [b, a]", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[a,, b] = [, b, a]", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[x, a] = [...x, a]", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[...a] = [...a, 1]", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[a, ...b] = [0, ...b, 1]", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "[a, b] = {a, b}", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({a} = a)", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({a = 1} = {a})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({a: b} = {a})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({a} = {a: b})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({a} = {a() {}})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({a} = {[a]: a})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({[a]: b} = {[a]: b})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({'foo': a, 1: a} = {'bar': a, 2: a})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "({a, ...b} = {a, ...b})", /*parserOptions: { ecmaVersion: 2018 }*/ },
                    { code => "a.b = a.c", options => { props => true } },
                    { code => "a.b = c.b", options => { props => true } },
                    { code => "a.b = a[b]", options => { props => true } },
                    { code => "a[b] = a.b", options => { props => true } },
                    { code => "a.b().c = a.b().c", options => { props => true } },
                    { code => "b().c = b().c", options => { props => true } },
                    { code => "a.null = a[/(?<zero>0)/]", options => { props => true }, /*parserOptions: { ecmaVersion: 2018 }*/ },
                    { code => "a[b + 1] = a[b + 1]", options => { props => true } }, // it ignores non-simple computed properties.
                    {
                        code => "a.b = a.b",
                        options => { props => false }
                    },
                    {
                        code => "a.b.c = a.b.c",
                        options => { props => false }
                    },
                    {
                        code => "a[b] = a[b]",
                        options => { props => false }
                    },
                    {
                        code => "a['b'] = a['b']",
                        options => { props => false }
                    },
                    {
                        code => "a[\n    'b'\n] = a[\n    'b'\n]",
                        options => { props => false }
                    },
                    {
                        code => "this.x = this.y",
                        options => { props => true }
                    },
                    {
                        code => "this.x = this.x",
                        options => { props => false }
                    },
                    {
                        code => "class C { #field; foo() { this['#field'] = this.#field; } }",
                        // parserOptions: { ecmaVersion: 2022 }
                    },
                    {
                        code => "class C { #field; foo() { this.#field = this['#field']; } }",
                        // parserOptions: { ecmaVersion: 2022 }
                    }
                ],
                invalid => [
                    { code => "a = a", errors => [{ message_id => "self_assignment", data => { name => "a" } }] },
                    { code => "[a] = [a]", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }] },
                    { code => "[a, b] = [a, b]", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }, { message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "[a, b] = [a, c]", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }] },
                    { code => "[a, b] = [, b]", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "[a, ...b] = [a, ...b]", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }, { message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "[[a], {b}] = [[a], {b}]", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }, { message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({a} = {a})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }] },
                    { code => "({a: b} = {a: b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({'a': b} = {'a': b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({a: b} = {'a': b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({'a': b} = {a: b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({1: b} = {1: b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({1: b} = {'1': b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({'1': b} = {1: b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({['a']: b} = {a: b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({'a': b} = {[`a`]: b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({1: b} = {[1]: b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({a, b} = {a, b})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }, { message_id => "self_assignment", data => { name => "b" } }] },
                    { code => "({a, b} = {b, a})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }, { message_id => "self_assignment", data => { name => "a" } }] },
                    { code => "({a, b} = {c, a})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "a" } }] },
                    { code => "({a: {b}, c: [d]} = {a: {b}, c: [d]})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }, { message_id => "self_assignment", data => { name => "d" } }] },
                    { code => "({a, b} = {a, ...x, b})", /*parserOptions: { ecmaVersion: 2018 },*/ errors => [{ message_id => "self_assignment", data => { name => "b" } }] },
                    {
                        code => "a.b = a.b",
                        errors => [{ message_id => "self_assignment", data => { name => "a.b" } }]
                    },
                    {
                        code => "a.b.c = a.b.c",
                        errors => [{ message_id => "self_assignment", data => { name => "a.b.c" } }]
                    },
                    {
                        code => "a[b] = a[b]",
                        errors => [{ message_id => "self_assignment", data => { name => "a[b]" } }]
                    },
                    {
                        code => "a['b'] = a['b']",
                        errors => [{ message_id => "self_assignment", data => { name => "a['b']" } }]
                    },
                    {
                        code => "a[\n    'b'\n] = a[\n    'b'\n]",
                        errors => [{ message_id => "self_assignment", data => { name => "a['b']" } }]
                    },
                    { code => "a.b = a.b", options => { props => true }, errors => [{ message_id => "self_assignment", data => { name => "a.b" } }] },
                    { code => "a.b.c = a.b.c", options => { props => true }, errors => [{ message_id => "self_assignment", data => { name => "a.b.c" } }] },
                    { code => "a[b] = a[b]", options => { props => true }, errors => [{ message_id => "self_assignment", data => { name => "a[b]" } }] },
                    { code => "a['b'] = a['b']", options => { props => true }, errors => [{ message_id => "self_assignment", data => { name => "a['b']" } }] },
                    { code => "a[\n    'b'\n] = a[\n    'b'\n]", options => { props => true }, errors => [{ message_id => "self_assignment", data => { name => "a['b']" } }] },
                    {
                        code => "this.x = this.x",
                        options => { props => true },
                        errors => [{ message_id => "self_assignment", data => { name => "this.x" } }]
                    },
                    { code => "a['/(?<zero>0)/'] = a[/(?<zero>0)/]", options => { props => true }, /*parserOptions: { ecmaVersion: 2018 },*/ errors => [{ message_id => "self_assignment", data => { name => "a[/(?<zero>0)/]" } }] },

                    // Optional chaining
                    {
                        code => "(a?.b).c = (a?.b).c",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "self_assignment", data => { name => "(a?.b).c" } }]
                    },
                    {
                        code => "a.b = a?.b",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "self_assignment", data => { name => "a?.b" } }]
                    },

                    // Private members
                    {
                        code => "class C { #field; foo() { this.#field = this.#field; } }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [{ message_id => "self_assignment", data => { name => "this.#field" } }]
                    },
                    {
                        code => "class C { #field; foo() { [this.#field] = [this.#field]; } }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [{ message_id => "self_assignment", data => { name => "this.#field" } }]
                    },

                    // logical assignment
                    {
                        code => "a &&= a",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "self_assignment", data => { name => "a" } }]
                    },
                    {
                        code => "a ||= a",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "self_assignment", data => { name => "a" } }]
                    },
                    {
                        code => "a ??= a",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "self_assignment", data => { name => "a" } }]
                    }
                ]
            },
        )
    }
}
