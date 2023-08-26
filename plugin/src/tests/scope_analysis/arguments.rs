#![cfg(test)]

use speculoos::prelude::*;

use crate::{scope::analyze, tests::helpers::parse};

#[test]
fn test_arguments_are_correctly_materialized() {
    let code = "
        (function () {
            arguments;
        }());
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that(&scope_manager.scopes).has_length(2);
}
