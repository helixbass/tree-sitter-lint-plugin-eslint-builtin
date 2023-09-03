use std::collections::HashMap;

use once_cell::sync::Lazy;
use squalid::HashMapExt;

type Globals = HashMap<&'static str, Visibility>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Visibility {
    Readonly,
    Writable,
    #[allow(dead_code)]
    Off,
}

pub static COMMONJS: Lazy<Globals> = Lazy::new(|| {
    [
        ("exports", Visibility::Writable),
        ("global", Visibility::Readonly),
        ("module", Visibility::Readonly),
        ("require", Visibility::Readonly),
    ]
    .into()
});

pub static ES3: Lazy<Globals> = Lazy::new(|| {
    [
        ("Array", Visibility::Readonly),
        ("Boolean", Visibility::Readonly),
        ("constructor", Visibility::Readonly),
        ("Date", Visibility::Readonly),
        ("decodeURI", Visibility::Readonly),
        ("decodeURIComponent", Visibility::Readonly),
        ("encodeURI", Visibility::Readonly),
        ("encodeURIComponent", Visibility::Readonly),
        ("Error", Visibility::Readonly),
        ("escape", Visibility::Readonly),
        ("eval", Visibility::Readonly),
        ("EvalError", Visibility::Readonly),
        ("Function", Visibility::Readonly),
        ("hasOwnProperty", Visibility::Readonly),
        ("Infinity", Visibility::Readonly),
        ("isFinite", Visibility::Readonly),
        ("isNaN", Visibility::Readonly),
        ("isPrototypeOf", Visibility::Readonly),
        ("Math", Visibility::Readonly),
        ("NaN", Visibility::Readonly),
        ("Number", Visibility::Readonly),
        ("Object", Visibility::Readonly),
        ("parseFloat", Visibility::Readonly),
        ("parseInt", Visibility::Readonly),
        ("propertyIsEnumerable", Visibility::Readonly),
        ("RangeError", Visibility::Readonly),
        ("ReferenceError", Visibility::Readonly),
        ("RegExp", Visibility::Readonly),
        ("String", Visibility::Readonly),
        ("SyntaxError", Visibility::Readonly),
        ("toLocaleString", Visibility::Readonly),
        ("toString", Visibility::Readonly),
        ("TypeError", Visibility::Readonly),
        ("undefined", Visibility::Readonly),
        ("unescape", Visibility::Readonly),
        ("URIError", Visibility::Readonly),
        ("valueOf", Visibility::Readonly),
    ]
    .into()
});

pub static ES5: Lazy<Globals> =
    Lazy::new(|| ES3.clone().and_extend([("JSON", Visibility::Readonly)]));

pub static ES2015: Lazy<Globals> = Lazy::new(|| {
    ES5.clone().and_extend([
        ("ArrayBuffer", Visibility::Readonly),
        ("DataView", Visibility::Readonly),
        ("Float32Array", Visibility::Readonly),
        ("Float64Array", Visibility::Readonly),
        ("Int16Array", Visibility::Readonly),
        ("Int32Array", Visibility::Readonly),
        ("Int8Array", Visibility::Readonly),
        ("Map", Visibility::Readonly),
        ("Promise", Visibility::Readonly),
        ("Proxy", Visibility::Readonly),
        ("Reflect", Visibility::Readonly),
        ("Set", Visibility::Readonly),
        ("Symbol", Visibility::Readonly),
        ("Uint16Array", Visibility::Readonly),
        ("Uint32Array", Visibility::Readonly),
        ("Uint8Array", Visibility::Readonly),
        ("Uint8ClampedArray", Visibility::Readonly),
        ("WeakMap", Visibility::Readonly),
        ("WeakSet", Visibility::Readonly),
    ])
});

pub static ES2016: Lazy<Globals> = Lazy::new(|| ES2015.clone());

pub static ES2017: Lazy<Globals> = Lazy::new(|| {
    ES2016.clone().and_extend([
        ("Atomics", Visibility::Readonly),
        ("SharedArrayBuffer", Visibility::Readonly),
    ])
});

pub static ES2018: Lazy<Globals> = Lazy::new(|| ES2017.clone());

pub static ES2019: Lazy<Globals> = Lazy::new(|| ES2018.clone());

pub static ES2020: Lazy<Globals> = Lazy::new(|| {
    ES2019.clone().and_extend([
        ("BigInt", Visibility::Readonly),
        ("BigInt64Array", Visibility::Readonly),
        ("BigUint64Array", Visibility::Readonly),
        ("globalThis", Visibility::Readonly),
    ])
});

pub static ES2021: Lazy<Globals> = Lazy::new(|| {
    ES2020.clone().and_extend([
        ("AggregateError", Visibility::Readonly),
        ("FinalizationRegistry", Visibility::Readonly),
        ("WeakRef", Visibility::Readonly),
    ])
});

pub static ES2022: Lazy<Globals> = Lazy::new(|| ES2021.clone());

pub static ES2023: Lazy<Globals> = Lazy::new(|| ES2022.clone());

pub static ES2024: Lazy<Globals> = Lazy::new(|| ES2023.clone());

pub static BUILTIN: Lazy<Globals> = Lazy::new(|| ES2023.clone());
