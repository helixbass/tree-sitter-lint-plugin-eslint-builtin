#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use super::util::get_supported_ecma_versions;
use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType, VariableType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_let_materialize_iteration_scope_for_for_in_statement_1() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "
            (function () {
                let i = 20;
                for (let i in i) {
                    console.log(i);
                }
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
        assert_that!(&scopes).has_length(4);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.variables().collect_vec()).is_empty();

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(2);
        assert_that!(&variables[0].name()).is_equal_to("arguments");
        assert_that!(&variables[1].name()).is_equal_to("i");
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(1);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[0].resolved())
            .is_some()
            .is_equal_to(&variables[1]);

        let scope = &scopes[2];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::For);
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(1);
        assert_that!(&variables[0].name()).is_equal_to("i");
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[0].resolved())
            .is_some()
            .is_equal_to(&variables[0]);
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[1].resolved())
            .is_some()
            .is_equal_to(&variables[0]);

        let iter_scope = &scopes[2];
        let scope = &scopes[3];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Block);
        assert_that!(&scope.variables().collect_vec()).is_empty();
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("console");
        assert_that!(&references[0].resolved()).is_none();
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[1].resolved())
            .is_some()
            .is_equal_to(&iter_scope.variables().next().unwrap());
    });
}

#[test]
fn test_let_materialize_iteration_scope_for_for_in_statement_2() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "
            (function () {
                let i = 20;
                for (let { i, j, k } in i) {
                    console.log(i);
                }
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
        assert_that!(&scopes).has_length(4);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.variables().collect_vec()).is_empty();

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(2);
        assert_that!(&variables[0].name()).is_equal_to("arguments");
        assert_that!(&variables[1].name()).is_equal_to("i");
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(1);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[0].resolved())
            .is_some()
            .is_equal_to(&variables[1]);

        let scope = &scopes[2];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::For);
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(3);
        assert_that!(&variables[0].name()).is_equal_to("i");
        assert_that!(&variables[1].name()).is_equal_to("j");
        assert_that!(&variables[2].name()).is_equal_to("k");
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(4);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[0].resolved())
            .is_some()
            .is_equal_to(&variables[0]);
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("j");
        assert_that!(&references[1].resolved())
            .is_some()
            .is_equal_to(&variables[1]);
        assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("k");
        assert_that!(&references[2].resolved())
            .is_some()
            .is_equal_to(&variables[2]);
        assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[3].resolved())
            .is_some()
            .is_equal_to(&variables[0]);

        let iter_scope = &scopes[2];
        let scope = &scopes[3];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Block);
        assert_that!(&scope.variables().collect_vec()).is_empty();
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("console");
        assert_that!(&references[0].resolved()).is_none();
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[1].resolved())
            .is_some()
            .is_equal_to(&iter_scope.variables().next().unwrap());
    });
}

#[test]
fn test_let_materialize_iteration_scope_for_for_statement_2() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "
            (function () {
                let i = 20;
                let obj = {};
                for (let { i, j, k } = obj; i < okok; ++i) {
                    console.log(i, j, k);
                }
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
        assert_that!(&scopes).has_length(4);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.variables().collect_vec()).is_empty();

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(3);
        assert_that!(&variables[0].name()).is_equal_to("arguments");
        assert_that!(&variables[1].name()).is_equal_to("i");
        assert_that!(&variables[2].name()).is_equal_to("obj");
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[0].resolved())
            .is_some()
            .is_equal_to(&variables[1]);
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("obj");
        assert_that!(&references[1].resolved())
            .is_some()
            .is_equal_to(&variables[2]);

        let function_scope = &scopes[1];
        let scope = &scopes[2];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::For);
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(3);
        assert_that!(&variables[0].name()).is_equal_to("i");
        assert_that!(&variables[0].defs().next().unwrap().type_())
            .is_equal_to(VariableType::Variable);
        assert_that!(&variables[1].name()).is_equal_to("j");
        assert_that!(&variables[1].defs().next().unwrap().type_())
            .is_equal_to(VariableType::Variable);
        assert_that!(&variables[2].name()).is_equal_to("k");
        assert_that!(&variables[2].defs().next().unwrap().type_())
            .is_equal_to(VariableType::Variable);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(7);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[0].resolved())
            .is_some()
            .is_equal_to(&variables[0]);
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("j");
        assert_that!(&references[1].resolved())
            .is_some()
            .is_equal_to(&variables[1]);
        assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("k");
        assert_that!(&references[2].resolved())
            .is_some()
            .is_equal_to(&variables[2]);
        assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("obj");
        assert_that!(&references[3].resolved())
            .is_some()
            .is_equal_to(&function_scope.variables().collect_vec()[2]);
        assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[4].resolved())
            .is_some()
            .is_equal_to(&variables[0]);
        assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("okok");
        assert_that!(&references[5].resolved()).is_none();
        assert_that(&&*references[6].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[6].resolved())
            .is_some()
            .is_equal_to(&variables[0]);

        let iter_scope = &scopes[2];
        let scope = &scopes[3];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Block);
        assert_that!(&scope.variables().collect_vec()).is_empty();
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(4);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("console");
        assert_that!(&references[0].resolved()).is_none();
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that!(&references[1].resolved())
            .is_some()
            .is_equal_to(&iter_scope.variables().next().unwrap());
        assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("j");
        assert_that!(&references[2].resolved())
            .is_some()
            .is_equal_to(&iter_scope.variables().collect_vec()[1]);
        assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("k");
        assert_that!(&references[3].resolved())
            .is_some()
            .is_equal_to(&iter_scope.variables().collect_vec()[2]);
    });
}
