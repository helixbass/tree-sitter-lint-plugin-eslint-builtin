#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    scope::{analyze, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_creates_scope() {
    tracing_subscribe();

    let code = "
        (function () {
            with (obj) {
                testing;
            }
        }());
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        Default::default()
    );

    let scopes = scope_manager.scopes().collect_vec();

    assert_that!(&scopes).has_length(3);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.is_arguments_materialized()).is_false();
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);
    assert_that(&references[0].resolved()).is_none();

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::With);
    assert_that(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.is_arguments_materialized()).is_true();
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);
    assert_that(&references[0].resolved()).is_none();
}
