use std::{borrow::Cow, collections::HashMap, fmt};

use once_cell::sync::Lazy;
use serde::{
    de::{self, Unexpected},
    Deserialize, Deserializer,
};
use squalid::HashMapExt;

pub type Globals = HashMap<Cow<'static, str>, Visibility>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Visibility {
    Readonly,
    Writable,
    #[allow(dead_code)]
    Off,
}

impl<'de> Deserialize<'de> for Visibility {
    fn deserialize<TDeserializer>(deserializer: TDeserializer) -> Result<Self, TDeserializer::Error>
    where
        TDeserializer: Deserializer<'de>,
    {
        deserializer.deserialize_any(VisibilityDeserializeVisitor)
    }
}

struct VisibilityDeserializeVisitor;

impl<'de> de::Visitor<'de> for VisibilityDeserializeVisitor {
    type Value = Visibility;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("true/false/\"readonly\"/\"writable\"/\"readable\"/\"writeable\"")
    }

    fn visit_bool<TError>(self, value: bool) -> Result<Self::Value, TError>
    where
        TError: de::Error,
    {
        Ok(match value {
            true => Visibility::Writable,
            false => Visibility::Readonly,
        })
    }

    fn visit_str<TError>(self, value: &str) -> Result<Self::Value, TError>
    where
        TError: de::Error,
    {
        match value {
            "readonly" | "readable" | "false" => Ok(Visibility::Readonly),
            "writable" | "writeable" | "true" => Ok(Visibility::Writable),
            "off" => Ok(Visibility::Off),
            value => Err(de::Error::invalid_value(Unexpected::Str(value), &self)),
        }
    }

    fn visit_unit<TError>(self) -> Result<Self::Value, TError>
    where
        TError: de::Error,
    {
        Ok(Visibility::Readonly)
    }
}

pub static COMMONJS: Lazy<Globals> = Lazy::new(|| {
    [
        (Cow::Borrowed("exports"), Visibility::Writable),
        (Cow::Borrowed("global"), Visibility::Readonly),
        (Cow::Borrowed("module"), Visibility::Readonly),
        (Cow::Borrowed("require"), Visibility::Readonly),
    ]
    .into()
});

pub static ES3: Lazy<Globals> = Lazy::new(|| {
    [
        (Cow::Borrowed("Array"), Visibility::Readonly),
        (Cow::Borrowed("Boolean"), Visibility::Readonly),
        (Cow::Borrowed("constructor"), Visibility::Readonly),
        (Cow::Borrowed("Date"), Visibility::Readonly),
        (Cow::Borrowed("decodeURI"), Visibility::Readonly),
        (Cow::Borrowed("decodeURIComponent"), Visibility::Readonly),
        (Cow::Borrowed("encodeURI"), Visibility::Readonly),
        (Cow::Borrowed("encodeURIComponent"), Visibility::Readonly),
        (Cow::Borrowed("Error"), Visibility::Readonly),
        (Cow::Borrowed("escape"), Visibility::Readonly),
        (Cow::Borrowed("eval"), Visibility::Readonly),
        (Cow::Borrowed("EvalError"), Visibility::Readonly),
        (Cow::Borrowed("Function"), Visibility::Readonly),
        (Cow::Borrowed("hasOwnProperty"), Visibility::Readonly),
        (Cow::Borrowed("Infinity"), Visibility::Readonly),
        (Cow::Borrowed("isFinite"), Visibility::Readonly),
        (Cow::Borrowed("isNaN"), Visibility::Readonly),
        (Cow::Borrowed("isPrototypeOf"), Visibility::Readonly),
        (Cow::Borrowed("Math"), Visibility::Readonly),
        (Cow::Borrowed("NaN"), Visibility::Readonly),
        (Cow::Borrowed("Number"), Visibility::Readonly),
        (Cow::Borrowed("Object"), Visibility::Readonly),
        (Cow::Borrowed("parseFloat"), Visibility::Readonly),
        (Cow::Borrowed("parseInt"), Visibility::Readonly),
        (Cow::Borrowed("propertyIsEnumerable"), Visibility::Readonly),
        (Cow::Borrowed("RangeError"), Visibility::Readonly),
        (Cow::Borrowed("ReferenceError"), Visibility::Readonly),
        (Cow::Borrowed("RegExp"), Visibility::Readonly),
        (Cow::Borrowed("String"), Visibility::Readonly),
        (Cow::Borrowed("SyntaxError"), Visibility::Readonly),
        (Cow::Borrowed("toLocaleString"), Visibility::Readonly),
        (Cow::Borrowed("toString"), Visibility::Readonly),
        (Cow::Borrowed("TypeError"), Visibility::Readonly),
        (Cow::Borrowed("undefined"), Visibility::Readonly),
        (Cow::Borrowed("unescape"), Visibility::Readonly),
        (Cow::Borrowed("URIError"), Visibility::Readonly),
        (Cow::Borrowed("valueOf"), Visibility::Readonly),
    ]
    .into()
});

pub static ES5: Lazy<Globals> = Lazy::new(|| {
    ES3.clone()
        .and_extend([(Cow::Borrowed("JSON"), Visibility::Readonly)])
});

pub static ES2015: Lazy<Globals> = Lazy::new(|| {
    ES5.clone().and_extend([
        (Cow::Borrowed("ArrayBuffer"), Visibility::Readonly),
        (Cow::Borrowed("DataView"), Visibility::Readonly),
        (Cow::Borrowed("Float32Array"), Visibility::Readonly),
        (Cow::Borrowed("Float64Array"), Visibility::Readonly),
        (Cow::Borrowed("Int16Array"), Visibility::Readonly),
        (Cow::Borrowed("Int32Array"), Visibility::Readonly),
        (Cow::Borrowed("Int8Array"), Visibility::Readonly),
        (Cow::Borrowed("Map"), Visibility::Readonly),
        (Cow::Borrowed("Promise"), Visibility::Readonly),
        (Cow::Borrowed("Proxy"), Visibility::Readonly),
        (Cow::Borrowed("Reflect"), Visibility::Readonly),
        (Cow::Borrowed("Set"), Visibility::Readonly),
        (Cow::Borrowed("Symbol"), Visibility::Readonly),
        (Cow::Borrowed("Uint16Array"), Visibility::Readonly),
        (Cow::Borrowed("Uint32Array"), Visibility::Readonly),
        (Cow::Borrowed("Uint8Array"), Visibility::Readonly),
        (Cow::Borrowed("Uint8ClampedArray"), Visibility::Readonly),
        (Cow::Borrowed("WeakMap"), Visibility::Readonly),
        (Cow::Borrowed("WeakSet"), Visibility::Readonly),
    ])
});

pub static ES2016: Lazy<Globals> = Lazy::new(|| ES2015.clone());

pub static ES2017: Lazy<Globals> = Lazy::new(|| {
    ES2016.clone().and_extend([
        (Cow::Borrowed("Atomics"), Visibility::Readonly),
        (Cow::Borrowed("SharedArrayBuffer"), Visibility::Readonly),
    ])
});

pub static ES2018: Lazy<Globals> = Lazy::new(|| ES2017.clone());

pub static ES2019: Lazy<Globals> = Lazy::new(|| ES2018.clone());

pub static ES2020: Lazy<Globals> = Lazy::new(|| {
    ES2019.clone().and_extend([
        (Cow::Borrowed("BigInt"), Visibility::Readonly),
        (Cow::Borrowed("BigInt64Array"), Visibility::Readonly),
        (Cow::Borrowed("BigUint64Array"), Visibility::Readonly),
        (Cow::Borrowed("globalThis"), Visibility::Readonly),
    ])
});

pub static ES2021: Lazy<Globals> = Lazy::new(|| {
    ES2020.clone().and_extend([
        (Cow::Borrowed("AggregateError"), Visibility::Readonly),
        (Cow::Borrowed("FinalizationRegistry"), Visibility::Readonly),
        (Cow::Borrowed("WeakRef"), Visibility::Readonly),
    ])
});

pub static ES2022: Lazy<Globals> = Lazy::new(|| ES2021.clone());

pub static ES2023: Lazy<Globals> = Lazy::new(|| ES2022.clone());

pub static ES2024: Lazy<Globals> = Lazy::new(|| ES2023.clone());

pub static BUILTIN: Lazy<Globals> = Lazy::new(|| ES2023.clone());

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn test_deserialize_visibility() {
        [
            (r#""off""#, Visibility::Off),
            (r#"true"#, Visibility::Writable),
            (r#""true""#, Visibility::Writable),
            (r#"false"#, Visibility::Readonly),
            (r#""false""#, Visibility::Readonly),
            (r#"null"#, Visibility::Readonly),
            (r#""writeable""#, Visibility::Writable),
            (r#""writable""#, Visibility::Writable),
            (r#""readable""#, Visibility::Readonly),
            (r#""readonly""#, Visibility::Readonly),
            (r#""writable""#, Visibility::Writable)
        ].into_iter().for_each(|(input, output)| {
            let deserialized: Visibility = serde_json::from_str(input).unwrap();
            assert_that!(&deserialized).is_equal_to(output);
        });

        let deserialized: Result<Visibility, _> = serde_json::from_str("something else");
        assert_that!(&deserialized).is_err();
    }
}
