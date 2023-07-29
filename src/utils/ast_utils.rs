use const_format::formatcp;
use once_cell::sync::Lazy;
use regex::Regex;
use tree_sitter_lint::tree_sitter::Node;

use crate::kind::{ArrowFunction, Function, FunctionDeclaration};

static any_function_pattern: Lazy<Regex> = Lazy::new(|| {
    Regex::new(formatcp!(
        r#"^(?:{FunctionDeclaration}|{Function}|{ArrowFunction})$"#
    ))
    .unwrap()
});

pub fn is_function(node: Node) -> bool {
    any_function_pattern.is_match(node.kind())
}
