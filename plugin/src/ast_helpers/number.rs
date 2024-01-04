use std::{hash, ops};

use squalid::{regex, CowStrExt};
use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

use crate::{assert_kind, kind};

#[derive(Copy, Clone, Debug)]
pub enum Number {
    NaN,
    Integer(i64),
    Float(f64),
}

impl Number {
    pub fn is_truthy(&self) -> bool {
        match self {
            Number::NaN => false,
            Number::Integer(value) => *value != 0,
            Number::Float(value) => *value != 0.0,
        }
    }
}

impl From<&str> for Number {
    fn from(value: &str) -> Self {
        let mut value = regex!(r#"_"#).replace_all(value, "");
        let mut is_bigint = false;
        if is_bigint_literal(&value) {
            value = value.sliced(|len| ..len - 1);
            is_bigint = true;
        }
        if is_hex_literal(&value) {
            i64::from_str_radix(&value[2..], 16).map_or(Self::NaN, Self::Integer)
        } else if is_octal_literal(&value) {
            i64::from_str_radix(&value[2..], 8).map_or(Self::NaN, Self::Integer)
        } else if is_binary_literal(&value) {
            i64::from_str_radix(&value[2..], 2).map_or(Self::NaN, Self::Integer)
        // } else if is_bigint_literal(&value) {
        //     value[..value.len() - 1]
        //         .parse::<i64>()
        //         .map_or(Self::NaN, Self::Integer)
        } else if let Some(value) = value
            .strip_prefix('0')
            .filter(|value| !value.is_empty() && !value.contains('.'))
        {
            i64::from_str_radix(value, 8).map_or(Self::NaN, Self::Integer)
        } else {
            value.parse::<i64>().map(Self::Integer).unwrap_or_else(|_| {
                if is_bigint {
                    return Self::NaN;
                }
                value.parse::<f64>().map_or(Self::NaN, Self::Float)
            })
        }
    }
}

impl ops::Mul<i32> for Number {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self::Output {
        match self {
            Self::NaN => Self::NaN,
            Self::Integer(value) => Self::Integer(value * rhs as i64),
            Self::Float(value) => Self::Float(value * rhs as f64),
        }
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NaN, _) | (_, Self::NaN) => false,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Integer(a), Self::Float(b)) => *a as f64 == *b,
            (Self::Float(a), Self::Integer(b)) => *a == *b as f64,
        }
    }
}

impl Eq for Number {}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::NaN, _) | (_, Self::NaN) => None,
            (Self::Integer(a), Self::Integer(b)) => a.partial_cmp(b),
            (Self::Integer(a), Self::Float(b)) => (*a as f64).partial_cmp(b),
            (Self::Float(a), Self::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
        }
    }
}

impl hash::Hash for Number {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        match self {
            Number::NaN => "NaN".hash(state),
            Number::Integer(value) => (*value as f64).to_bits().hash(state),
            Number::Float(value) => value.to_bits().hash(state),
        }
    }
}

fn is_bigint_literal(number_node_text: &str) -> bool {
    number_node_text.ends_with('n')
}

fn is_hex_literal(number_node_text: &str) -> bool {
    number_node_text.starts_with("0x") || number_node_text.starts_with("0X")
}

fn is_binary_literal(number_node_text: &str) -> bool {
    number_node_text.starts_with("0b") || number_node_text.starts_with("0B")
}

fn is_octal_literal(number_node_text: &str) -> bool {
    number_node_text.starts_with("0o") || number_node_text.starts_with("0O")
}

pub fn get_number_literal_value(node: Node, context: &QueryMatchContext) -> Number {
    assert_kind!(node, kind::Number);

    Number::from(&*context.get_node_text(node))
}

pub fn get_number_literal_string_value(node: Node, context: &QueryMatchContext) -> String {
    match get_number_literal_value(node, context) {
        Number::NaN => unreachable!("I don't know if this should be possible?"),
        Number::Integer(number) => number.to_string(),
        Number::Float(number) => number.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_values() {
        [
            ("1", Number::Integer(1)),
            ("1.0", Number::Float(1.0)),
            ("0", Number::Integer(0)),
            ("0.0", Number::Float(0.0)),
            ("0x1f", Number::Integer(31)),
            ("1_000", Number::Integer(1000)),
            ("1n", Number::Integer(1)),
            ("-1", Number::Integer(-1)),
            ("-1.0", Number::Float(-1.0)),
            ("0b1001", Number::Integer(9)),
            ("0o12", Number::Integer(10)),
            ("012", Number::Integer(10)),
            ("abc", Number::NaN),
            ("1abc", Number::NaN),
        ]
        .into_iter()
        .for_each(
            |(number_str, expected)| match (Number::from(number_str), expected) {
                (Number::NaN, Number::NaN) => (),
                (actual, expected) => assert_eq!(actual, expected),
            },
        );
    }
}
