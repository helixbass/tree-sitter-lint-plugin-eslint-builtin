#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use super::util::get_supported_ecma_versions;
use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_let_is_materialized_in_es6_block_scope_1() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "
            {
                let i = 20;
                i;
            }
        ";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();

        assert_that(&scopes).has_length(2);

        let scope = &scopes[0];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that(&scope.variables().collect_vec()).has_length(0);

        let scope = &scopes[1];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Block);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(1);
        assert_that(&variables[0].name()).is_equal_to("i");
        let references = scope.references().collect_vec();
        assert_that(&references).has_length(2);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("i");
    });
}

#[test]
fn test_function_declaration_is_materialized_in_es6_block_scope() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "
            {
                function test() {
                }
                test();
            }
        ";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();

        assert_that(&scopes).has_length(3);

        let scope = &scopes[0];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that(&scope.variables().collect_vec()).has_length(0);

        let scope = &scopes[1];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Block);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(1);
        assert_that(&variables[0].name()).is_equal_to("test");
        let references = scope.references().collect_vec();
        assert_that(&references).has_length(1);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("test");

        let scope = &scopes[2];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(1);
        assert_that(&variables[0].name()).is_equal_to("arguments");
        assert_that(&scope.references().collect_vec()).is_empty();
    });
}

#[test]
fn test_let_is_not_hoistable_1() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        // TODO: this `(1)` looks like it's meant to
        // be inside a comment, upstream?
        let code = "
            var i = 42; (1)
            {
                i;  // (2) ReferenceError at runtime.
                let i = 20;  // (2)
                i;  // (2)
            }
        ";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();

        assert_that(&scopes).has_length(2);

        let scope = &scopes[0];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(1);
        assert_that(&variables[0].name()).is_equal_to("i");
        assert_that(&scope.references().collect_vec()).has_length(1);

        let scope = &scopes[1];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Block);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(1);
        let variable = &variables[0];
        assert_that(&variable.name()).is_equal_to("i");
        let references = scope.references().collect_vec();
        assert_that(&references).has_length(3);
        assert_that(&references[0].resolved()).is_some().is_equal_to(variable);
        assert_that(&references[1].resolved()).is_some().is_equal_to(variable);
        assert_that(&references[2].resolved()).is_some().is_equal_to(variable);
    });
}

#[test]
fn test_let_is_not_hoistable_2() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "
            (function () {
                var i = 42; // (1)
                i;  // (1)
                {
                    i;  // (3)
                    {
                        i;  // (2)
                        let i = 20;  // (2)
                        i;  // (2)
                    }
                    let i = 30;  // (3)
                    i;  // (3)
                }
                i;  // (1)
            }());
        ";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();

        assert_that(&scopes).has_length(4);

        let scope = &scopes[0];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that(&scope.variables().collect_vec()).is_empty();
        assert_that(&scope.references().collect_vec()).is_empty();

        let scope = &scopes[1];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(2);
        assert_that(&variables[0].name()).is_equal_to("arguments");
        assert_that(&variables[1].name()).is_equal_to("i");
        let v1 = &variables[1];

        let references = scope.references().collect_vec();
        assert_that(&references).has_length(3);
        assert_that(&references[0].resolved()).is_some().is_equal_to(v1);
        assert_that(&references[1].resolved()).is_some().is_equal_to(v1);
        assert_that(&references[2].resolved()).is_some().is_equal_to(v1);

        let scope = &scopes[2];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Block);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(1);
        assert_that(&variables[0].name()).is_equal_to("i");
        let v3 = &variables[0];

        let references = scope.references().collect_vec();
        assert_that(&references).has_length(3);
        assert_that(&references[0].resolved()).is_some().is_equal_to(v3);
        assert_that(&references[1].resolved()).is_some().is_equal_to(v3);
        assert_that(&references[2].resolved()).is_some().is_equal_to(v3);

        let scope = &scopes[3];

        assert_that(&scope.type_()).is_equal_to(ScopeType::Block);
        let variables = scope.variables().collect_vec();
        assert_that(&variables).has_length(1);
        assert_that(&variables[0].name()).is_equal_to("i");
        let v2 = &variables[0];

        let references = scope.references().collect_vec();
        assert_that(&references).has_length(3);
        assert_that(&references[0].resolved()).is_some().is_equal_to(v2);
        assert_that(&references[1].resolved()).is_some().is_equal_to(v2);
        assert_that(&references[2].resolved()).is_some().is_equal_to(v2);
    });
}
