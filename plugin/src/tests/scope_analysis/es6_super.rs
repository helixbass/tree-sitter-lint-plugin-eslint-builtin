#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_is_not_handled_as_a_reference() {
    tracing_subscribe();

    let code = "
        class Foo extends Bar {
            constructor() {
                super();
            }

            method() {
                super.method();
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
    assert_that!(&scopes).has_length(4);

    let scope = &scopes[0];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("Foo");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("Bar");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Class);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("Foo");
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[3];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.references().collect_vec()).is_empty();
}
