#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    kind::{Program, ArrowFunction},
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_materialize_scope() {
    tracing_subscribe();

    let code = "
        var arrow = () => {
            let i = 0;
            var j = 20;
            console.log(i);
        }
    ";
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = manager.scopes().collect_vec();

    assert_that(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();
    assert_that(&scope.variables().collect_vec()).has_length(1);

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(ArrowFunction);
    assert_that(&scope.is_strict()).is_false();
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(2);

    assert_that(&variables[0].name()).is_equal_to("i");
    assert_that(&variables[1].name()).is_equal_to("j");
}

#[test]
fn test_generates_bindings_for_parameters() {
    tracing_subscribe();

    let code = "var arrow = (a, b, c, d) => {}";
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = manager.scopes().collect_vec();

    assert_that(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();
    assert_that(&scope.variables().collect_vec()).has_length(1);

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(ArrowFunction);
    assert_that(&scope.is_strict()).is_false();
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(4);

    assert_that(&variables[0].name()).is_equal_to("a");
    assert_that(&variables[1].name()).is_equal_to("b");
    assert_that(&variables[2].name()).is_equal_to("c");
    assert_that(&variables[3].name()).is_equal_to("d");
}

#[test]
fn test_inherits_upper_scope_strictness() {
    tracing_subscribe();

    let code = r#"
        "use strict";
        var arrow = () => {};
    "#;
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = manager.scopes().collect_vec();

    assert_that(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_true();
    assert_that(&scope.variables().collect_vec()).has_length(1);

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(ArrowFunction);
    assert_that(&scope.is_strict()).is_true();
    assert_that(&scope.variables().collect_vec()).is_empty();
}

#[test]
fn test_is_strict_when_a_strictness_directive_is_used() {
    tracing_subscribe();

    let code = r#"
        var arrow = () => {
            "use strict";
        };
    "#;
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = manager.scopes().collect_vec();

    assert_that(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();
    assert_that(&scope.variables().collect_vec()).has_length(1);

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(ArrowFunction);
    assert_that(&scope.is_strict()).is_true();
    assert_that(&scope.variables().collect_vec()).is_empty();
}

#[test]
fn test_works_with_no_body() {
    tracing_subscribe();

    let code = "var arrow = a => a;";
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = manager.scopes().collect_vec();

    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(ArrowFunction);
    assert_that(&scope.is_strict()).is_false();
    assert_that(&scope.variables().collect_vec()).has_length(1);
}
