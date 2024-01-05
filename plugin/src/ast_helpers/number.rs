use std::{hash, ops};

use squalid::{regex, CowStrExt};
use tree_sitter_lint::{tree_sitter::Node, QueryMatchContext};

use crate::{assert_kind, kind};

#[derive(Copy, Clone, Debug)]
pub enum NumberOrBigInt {
    Number(Number),
    BigInt(i64),
}

#[derive(Copy, Clone, Debug)]
pub enum Number {
    NaN,
    Integer(i64),
    Float(f64),
}

impl NumberOrBigInt {
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Number(Number::NaN) => false,
            Self::Number(Number::Integer(value)) => *value != 0,
            Self::Number(Number::Float(value)) => *value != 0.0,
            Self::BigInt(value) => *value != 0,
        }
    }

    pub fn eq_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(Number::NaN), _) | (_, Self::Number(Number::NaN)) => false,
            (Self::BigInt(a), Self::BigInt(b)) => a == b,
            (Self::BigInt(a), Self::Number(Number::Integer(b))) => a == b,
            (Self::BigInt(a), Self::Number(Number::Float(b))) => *a as f64 == *b,
            (Self::Number(Number::Integer(a)), Self::BigInt(b)) => a == b,
            (Self::Number(Number::Float(a)), Self::BigInt(b)) => *a == *b as f64,
            (Self::Number(Number::Integer(a)), Self::Number(Number::Integer(b))) => a == b,
            (Self::Number(Number::Float(a)), Self::Number(Number::Float(b))) => a == b,
            (Self::Number(Number::Integer(a)), Self::Number(Number::Float(b))) => *a as f64 == *b,
            (Self::Number(Number::Float(a)), Self::Number(Number::Integer(b))) => *a == *b as f64,
        }
    }

    pub fn partial_cmp_value(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.partial_cmp(b),
            (Self::BigInt(a), Self::BigInt(b)) => a.partial_cmp(b),
            (Self::Number(Number::Integer(a)), Self::BigInt(b)) => a.partial_cmp(b),
            (Self::Number(Number::Float(a)), Self::BigInt(b)) => a.partial_cmp(&(*b as f64)),
            (Self::BigInt(a), Self::Number(Number::Integer(b))) => a.partial_cmp(b),
            (Self::BigInt(a), Self::Number(Number::Float(b))) => (*a as f64).partial_cmp(b),
            _ => None,
        }
    }
}

impl From<&str> for NumberOrBigInt {
    fn from(value: &str) -> Self {
        let mut value = regex!(r#"_"#).replace_all(value, "");
        let mut is_bigint = false;
        if is_bigint_literal(&value) {
            value = value.sliced(|len| ..len - 1);
            is_bigint = true;
        }
        let to_integer_or_bigint = |parsed: i64| {
            if is_bigint {
                Self::BigInt(parsed)
            } else {
                Self::Number(Number::Integer(parsed))
            }
        };
        if is_hex_literal(&value) {
            i64::from_str_radix(&value[2..], 16)
                .map_or(Self::Number(Number::NaN), to_integer_or_bigint)
        } else if is_octal_literal(&value) {
            i64::from_str_radix(&value[2..], 8)
                .map_or(Self::Number(Number::NaN), to_integer_or_bigint)
        } else if is_binary_literal(&value) {
            i64::from_str_radix(&value[2..], 2)
                .map_or(Self::Number(Number::NaN), to_integer_or_bigint)
        // } else if is_bigint_literal(&value) {
        //     value[..value.len() - 1]
        //         .parse::<i64>()
        //         .map_or(Self::NaN, Self::Integer)
        } else if let Some(value) = value
            .strip_prefix('0')
            .filter(|value| !value.is_empty() && !value.contains('.'))
        {
            i64::from_str_radix(value, 8).map_or(Self::Number(Number::NaN), to_integer_or_bigint)
        } else {
            value
                .parse::<i64>()
                .map(to_integer_or_bigint)
                .unwrap_or_else(|_| {
                    if is_bigint {
                        return Self::Number(Number::NaN);
                    }
                    value
                        .parse::<f64>()
                        .map_or(Self::Number(Number::NaN), |parsed| {
                            Self::Number(Number::Float(parsed))
                        })
                })
        }
    }
}

impl ops::Mul<i32> for NumberOrBigInt {
    type Output = Self;

    fn mul(self, rhs: i32) -> Self::Output {
        match self {
            Self::Number(Number::NaN) => Self::Number(Number::NaN),
            Self::Number(Number::Integer(value)) => {
                Self::Number(Number::Integer(value * rhs as i64))
            }
            Self::Number(Number::Float(value)) => Self::Number(Number::Float(value * rhs as f64)),
            Self::BigInt(value) => Self::BigInt(value * rhs as i64),
        }
    }
}

impl PartialEq for NumberOrBigInt {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BigInt(a), Self::BigInt(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            _ => false,
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

impl Eq for NumberOrBigInt {}

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

// impl hash::Hash for NumberOrBigInt {
//     fn hash<H: hash::Hasher>(&self, state: &mut H) {
//         match self {
//             Self::Number(Number::NaN) => "NaN".hash(state),
//             Self::Number(Number::Integer(value)) => (*value as f64).to_bits().hash(state),
//             Self::Number(Number::Float(value)) => value.to_bits().hash(state),
//             // This will make BigInt's not hash to the same bucket as the corresponding
//             // Number which I don't know if that's "good" (but it's not "bad" because
//             // this has to agree with PartialEq/Eq which do not ever compare BigInt <-> Number
//             // as equal)?
//             Self::BigInt(value) => value.hash(state),
//         }
//     }
// }

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

pub fn get_number_literal_value(node: Node, context: &QueryMatchContext) -> NumberOrBigInt {
    assert_kind!(node, kind::Number);

    NumberOrBigInt::from(&*context.get_node_text(node))
}

pub fn get_number_literal_string_value(node: Node, context: &QueryMatchContext) -> String {
    match get_number_literal_value(node, context) {
        NumberOrBigInt::Number(Number::NaN) => {
            unreachable!("I don't know if this should be possible?")
        }
        NumberOrBigInt::Number(Number::Integer(number)) => number.to_string(),
        NumberOrBigInt::Number(Number::Float(number)) => number.to_string(),
        NumberOrBigInt::BigInt(number) => number.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parsed_values() {
        [
            ("1", NumberOrBigInt::Number(Number::Integer(1))),
            ("1.0", NumberOrBigInt::Number(Number::Float(1.0))),
            ("0", NumberOrBigInt::Number(Number::Integer(0))),
            ("0.0", NumberOrBigInt::Number(Number::Float(0.0))),
            ("0x1f", NumberOrBigInt::Number(Number::Integer(31))),
            ("1_000", NumberOrBigInt::Number(Number::Integer(1000))),
            ("1n", NumberOrBigInt::BigInt(1)),
            ("-1", NumberOrBigInt::Number(Number::Integer(-1))),
            ("-1.0", NumberOrBigInt::Number(Number::Float(-1.0))),
            ("0b1001", NumberOrBigInt::Number(Number::Integer(9))),
            ("0o12", NumberOrBigInt::Number(Number::Integer(10))),
            ("012", NumberOrBigInt::Number(Number::Integer(10))),
            ("abc", NumberOrBigInt::Number(Number::NaN)),
            ("1abc", NumberOrBigInt::Number(Number::NaN)),
        ]
        .into_iter()
        .for_each(|(number_str, expected)| {
            match (NumberOrBigInt::from(number_str), expected) {
                (NumberOrBigInt::Number(Number::NaN), NumberOrBigInt::Number(Number::NaN)) => (),
                (actual, expected) => assert_eq!(actual, expected),
            }
        });
    }
}
