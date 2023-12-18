#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    scope::{analyze, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_becomes_read_write() {
    tracing_subscribe();

    let code = "b++";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    let scopes = scope_manager.scopes().collect_vec();

    assert_that!(&scopes).has_length(1);
    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that!(&references[0].is_read_write()).is_true();
}
