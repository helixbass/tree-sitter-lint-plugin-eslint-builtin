use std::{borrow::Cow, cmp::Ordering, ops};

use tree_sitter_lint::{tree_sitter::Node, NodeExt, QueryMatchContext};

use crate::{
    assert_kind,
    ast_helpers::{
        get_call_expression_arguments, get_comma_separated_optional_non_comment_named_children,
        get_spread_element_argument,
    },
    kind::{
        is_literal_kind, Array, AssignmentExpression, BinaryExpression, CallExpression,
        ComputedPropertyName, MemberExpression, Pair, PrivatePropertyIdentifier,
        PropertyIdentifier, SpreadElement, SubscriptExpression,
    },
    scope::Scope,
    utils::ast_utils::get_static_string_value,
};

pub enum StaticValue<'a> {
    Vec(Vec<StaticValue<'a>>),
    Undefined,
    Bool(bool),
    String(Cow<'a, str>),
}

impl<'a> StaticValue<'a> {
    fn is_loose_equal(&self, other: &Self) -> bool {
        unimplemented!()
    }

    fn is_not_loose_equal(&self, other: &Self) -> bool {
        unimplemented!()
    }

    fn pow(&self, other: &Self) -> Self {
        unimplemented!()
    }
}

impl<'a> PartialEq for StaticValue<'a> {
    fn eq(&self, other: &Self) -> bool {
        unimplemented!()
    }
}

impl<'a> PartialOrd for StaticValue<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        unimplemented!()
    }
}

impl<'a> ops::Add for StaticValue<'a> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::BitAnd for StaticValue<'a> {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::BitOr for StaticValue<'a> {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::BitXor for StaticValue<'a> {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::Div for StaticValue<'a> {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::Mul for StaticValue<'a> {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::Shl for StaticValue<'a> {
    type Output = Self;

    fn shl(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::Shr for StaticValue<'a> {
    type Output = Self;

    fn shr(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> ops::Sub for StaticValue<'a> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl<'a> From<Vec<StaticValue<'a>>> for StaticValue<'a> {
    fn from(value: Vec<StaticValue<'a>>) -> Self {
        Self::Vec(value)
    }
}

impl<'a> From<bool> for StaticValue<'a> {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl<'a> From<Cow<'a, str>> for StaticValue<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        Self::String(value)
    }
}

impl<'a> From<Vec<StaticValue<'a>>> for StaticValueWrapper<'a> {
    fn from(value: Vec<StaticValue<'a>>) -> Self {
        Self {
            value: value.into(),
            optional: Default::default(),
        }
    }
}

impl<'a> From<bool> for StaticValueWrapper<'a> {
    fn from(value: bool) -> Self {
        Self {
            value: value.into(),
            optional: Default::default(),
        }
    }
}

impl<'a> From<Cow<'a, str>> for StaticValueWrapper<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        Self {
            value: value.into(),
            optional: Default::default(),
        }
    }
}

pub struct StaticValueWrapper<'a> {
    pub value: StaticValue<'a>,
    pub optional: Option<bool>,
}

impl<'a> From<StaticValue<'a>> for StaticValueWrapper<'a> {
    fn from(value: StaticValue) -> Self {
        Self {
            value,
            optional: Default::default(),
        }
    }
}

fn get_element_values<'a>(
    node_list: impl Iterator<Item = Option<Node<'a>>>,
    initial_scope: Option<&Scope<'a, '_>>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<Vec<StaticValue<'a>>> {
    let mut value_list: Vec<StaticValue> = Default::default();

    for element_node in node_list {
        match element_node {
            None => value_list.push(StaticValue::Undefined),
            Some(element_node) => {
                match element_node.kind() {
                    SpreadElement => {
                        let argument = get_static_value_r(
                            Some(get_spread_element_argument(element_node)),
                            initial_scope,
                            context,
                        )?;
                        value_list.extend(match argument.value {
                            StaticValue::Vec(value) => value,
                            // TODO: this should probably not panic
                            // (if I understand correctly this would trigger
                            // when encountering a spread element value that
                            // evaluates to something other than an array
                            // value)
                            _ => unreachable!(),
                        });
                    }
                    _ => {
                        let element =
                            get_static_value_r(Some(element_node), initial_scope, context)?;
                        value_list.push(element.value);
                    }
                }
            }
        }
    }

    Some(value_list)
}

fn get_static_value_r<'a>(
    node: Option<Node<'a>>,
    initial_scope: Option<&Scope<'a, '_>>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<StaticValueWrapper<'a>> {
    let node = node?;
    match node.kind() {
        Array => get_element_values(
            get_comma_separated_optional_non_comment_named_children(node),
            initial_scope,
            context,
        )
        .map(Into::into),
        AssignmentExpression => get_static_value_r(Some(node.field("right")), initial_scope, context),
        BinaryExpression => {
            let operator = node.field("operator").kind();
            if matches!(operator, "in" | "instanceof") {
                return None;
            }

            let left = get_static_value_r(Some(node.field("left")), initial_scope, context)?;
            let right = get_static_value_r(Some(node.field("right")), initial_scope, context)?;
            Some(match operator {
                "==" => left.value.is_loose_equal(&right.value).into(),
                "!=" => left.value.is_not_loose_equal(&right.value).into(),
                "===" => (left.value == right.value).into(),
                "!==" => (left.value != right.value).into(),
                "<" => (left.value < right.value).into(),
                "<=" => (left.value <= right.value).into(),
                ">" => (left.value > right.value).into(),
                ">=" => (left.value >= right.value).into(),
                "<<" => (left.value << right.value).into(),
                ">>" => (left.value >> right.value).into(),
                // TODO: no idea on >> vs >>>
                ">>>" => (left.value >> right.value).into(),
                "+" => (left.value + right.value).into(),
                "*" => (left.value * right.value).into(),
                "/" => (left.value / right.value).into(),
                "**" => left.value.pow(&right.value).into(),
                "|" => (left.value | right.value).into(),
                "^" => (left.value ^ right.value).into(),
                "&" => (left.value & right.value).into(),
                _ => return None,
            })
        }
        CallExpression => {
            let callee_node = node.field("function");
            let args = get_element_values(
                get_call_expression_arguments(node)?.map(Some),
                initial_scope,
                context,
            )?;
            match callee_node.kind() {
                MemberExpression | SubscriptExpression => {
                    if callee_node.kind() == MemberExpression
                        && callee_node.field("property").kind() == PrivatePropertyIdentifier
                    {
                        return None;
                    }
                    let object =
                        get_static_value_r(Some(callee_node.field("object")), initial_scope, context)?;
                    if matches!(object.value, StaticValue::Undefined)
                        && (object.optional == Some(true)
                            || node.child_by_field_name("optional_chain").is_some())
                    {
                        return Some(StaticValueWrapper {
                            value: StaticValue::Undefined,
                            optional: Some(true),
                        });
                    }
                    let property =
                        get_static_property_name_value(callee_node, initial_scope, context)?;

                    let receiver = &object.value;
                    let method_name = &property.value;
                    unimplemented!()
                }
                _ => {
                    let callee = get_static_value_r(Some(callee_node), initial_scope, context)?;
                    if matches!(callee.value, StaticValue::Undefined)
                        && node.child_by_field_name("optional_chain").is_some()
                    {
                        return Some(StaticValueWrapper {
                            value: StaticValue::Undefined,
                            optional: Some(true),
                        });
                    }
                    let func = &callee.value;
                    unimplemented!()
                }
            }
        }
        _ => None,
    }
}

pub fn get_static_property_name_value<'a>(
    node: Node<'a>,
    initial_scope: Option<&Scope<'a, '_>>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<StaticValueWrapper<'a>> {
    assert_kind!(node, MemberExpression | SubscriptExpression | Pair);

    let name_node = match node.kind() {
        Pair => node.field("key"),
        MemberExpression => node.field("property"),
        SubscriptExpression => node.field("index"),
    };

    if node.kind() == SubscriptExpression
        || node.kind() == Pair && name_node.kind() == ComputedPropertyName
    {
        return get_static_value_r(Some(name_node), initial_scope, context);
    }

    if name_node.kind() == PropertyIdentifier {
        return Some(name_node.text(context).into());
    }

    if is_literal_kind(name_node.kind()) {
        return Some(get_static_string_value(name_node, context).unwrap().into());
    }

    None
}

pub fn get_static_value<'a>(
    node: Node<'a>,
    initial_scope: Option<&Scope<'a, '_>>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<StaticValueWrapper<'a>> {
    // try {
    get_static_value_r(Some(node), initial_scope, context)
    // } catch (_error) {
    //     return null
    // }
}
