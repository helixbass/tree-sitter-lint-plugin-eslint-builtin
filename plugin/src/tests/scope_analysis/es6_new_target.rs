#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    kind::MethodDefinition,
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_should_not_make_references_of_new_target() {
    tracing_subscribe();

    let code = "
        class A {
            constructor() {
                new.target;
            }
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(&scopes).has_length(3);

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that!(&scope.block().kind()).is_equal_to(MethodDefinition);
    assert_that!(&scope.is_strict()).is_true();
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.references().collect_vec()).is_empty();
}
