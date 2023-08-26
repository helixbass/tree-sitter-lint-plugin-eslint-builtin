#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    scope::{analyze, ScopeType},
    tests::helpers::parse,
};

#[test]
fn test_arguments_are_correctly_materialized() {
    let code = "
        (function () {
            arguments;
        }());
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);
    let global_scope = &scopes[0];

    assert_that(&global_scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&global_scope.variables().collect_vec()).is_empty();
    assert_that(&global_scope.references().collect_vec()).is_empty();
}
