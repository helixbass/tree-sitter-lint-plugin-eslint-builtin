#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use super::util::get_supported_ecma_versions;
use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType, SourceType},
    tests::helpers::{parse, tracing_subscribe}, kind::{Program, FunctionDeclaration},
};

#[test]
fn test_ensures_all_user_scopes_are_strict_if_ecma_version_5() {
    tracing_subscribe();

    let code = r#"
        function foo() {
            function bar() {
                "use strict";
            }
        }
    "#;
    let ast = parse(code);

    get_supported_ecma_versions(Some(5)).for_each(|ecma_version| {
        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .implied_strict(true)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(scopes).has_length(3);

        let scope = &scopes[0];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that(&scope.block().kind()).is_equal_to(Program);
        assert_that(&scope.is_strict()).is_true();

        let scope = &scopes[1];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
        assert_that(&scope.block().kind()).is_equal_to(FunctionDeclaration);
        assert_that(&scope.is_strict()).is_true();

        let scope = &scopes[2];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
        assert_that(&scope.block().kind()).is_equal_to(FunctionDeclaration);
        assert_that(&scope.is_strict()).is_true();
    });
}

#[test]
fn test_ensures_implied_strict_option_is_only_effective_when_ecma_version_5() {
    tracing_subscribe();

    let code = "
        function foo() {}
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(3)
            .implied_strict(true)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(2);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(FunctionDeclaration);
    assert_that(&scope.is_strict()).is_false();
}

#[test]
fn test_omits_a_nodejs_global_scope_when_ensuring_all_user_scopes_are_strict() {
    tracing_subscribe();

    let code = "
        function foo() {}
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(5)
            .nodejs_scope(true)
            .implied_strict(true)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(3);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_true();

    let scope = &scopes[2];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(FunctionDeclaration);
    assert_that(&scope.is_strict()).is_true();
}

#[test]
fn test_omits_a_module_global_scope_when_ensuring_all_user_scopes_are_strict() {
    tracing_subscribe();

    let code = "
        function foo() {}
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .implied_strict(true)
            .source_type(SourceType::Module)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(3);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Module);
    assert_that(&scope.is_strict()).is_true();

    let scope = &scopes[2];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(FunctionDeclaration);
    assert_that(&scope.is_strict()).is_true();
}
