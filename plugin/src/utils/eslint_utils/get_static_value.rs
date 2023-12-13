use std::{cmp::Ordering, ops};

use tree_sitter_lint::{tree_sitter::Node, NodeExt};

use crate::{
    ast_helpers::{
        get_comma_separated_optional_non_comment_named_children, get_spread_element_argument,
    },
    kind::{Array, AssignmentExpression, BinaryExpression, SpreadElement},
    scope::Scope,
};

pub enum StaticValue {
    Vec(Vec<StaticValue>),
    Undefined,
    Bool(bool),
}

impl StaticValue {
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

impl PartialEq for StaticValue {
    fn eq(&self, other: &Self) -> bool {
        unimplemented!()
    }
}

impl PartialOrd for StaticValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        unimplemented!()
    }
}

impl ops::Add for StaticValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::BitAnd for StaticValue {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::BitOr for StaticValue {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::BitXor for StaticValue {
    type Output = Self;

    fn bitxor(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::Div for StaticValue {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::Mul for StaticValue {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::Shl for StaticValue {
    type Output = Self;

    fn shl(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::Shr for StaticValue {
    type Output = Self;

    fn shr(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl ops::Sub for StaticValue {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        unimplemented!()
    }
}

impl From<Vec<StaticValue>> for StaticValue {
    fn from(value: Vec<StaticValue>) -> Self {
        Self::Vec(value)
    }
}

impl From<bool> for StaticValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<Vec<StaticValue>> for StaticValueWrapper {
    fn from(value: Vec<StaticValue>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl From<bool> for StaticValueWrapper {
    fn from(value: bool) -> Self {
        Self {
            value: value.into(),
        }
    }
}

pub struct StaticValueWrapper {
    pub value: StaticValue,
}

impl From<StaticValue> for StaticValueWrapper {
    fn from(value: StaticValue) -> Self {
        Self { value }
    }
}

fn get_element_values<'a>(
    node_list: impl Iterator<Item = Option<Node<'a>>>,
    initial_scope: Option<Scope<'a, '_>>,
) -> Option<Vec<StaticValue>> {
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
                        let element = get_static_value_r(Some(element_node), initial_scope)?;
                        value_list.push(element.value);
                    }
                }
            }
        }
    }

    Some(value_list)
}

fn get_static_value_r(
    node: Option<Node>,
    initial_scope: Option<Scope>,
) -> Option<StaticValueWrapper> {
    let node = node?;
    match node.kind() {
        Array => get_element_values(
            get_comma_separated_optional_non_comment_named_children(node),
            initial_scope,
        )
        .map(Into::into),
        AssignmentExpression => get_static_value_r(Some(node.field("right")), initial_scope),
        BinaryExpression => {
            let operator = node.field("operator").kind();
            if matches!(operator, "in" | "instanceof") {
                return None;
            }

            let left = get_static_value_r(Some(node.field("left")), initial_scope)?;
            let right = get_static_value_r(Some(node.field("right")), initial_scope)?;
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
        _ => None,
    }
}

pub fn get_static_value(node: Node, initial_scope: Option<Scope>) -> Option<StaticValueWrapper> {
    // try {
    get_static_value_r(Some(node), initial_scope)
    // } catch (_error) {
    //     return null
    // }
}
