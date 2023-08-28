#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use super::util::get_supported_ecma_versions;
use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType, SourceType, VariableType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_should_import_names_from_source() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "import v from \"mod\";";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .source_type(SourceType::Module)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes).has_length(2);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.variables().collect_vec()).is_empty();
        assert_that!(&scope.references().collect_vec()).is_empty();

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Module);
        assert_that!(&scope.is_strict()).is_true();
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(1);
        assert_that!(&variables[0].name()).is_equal_to("v");
        assert_that!(&variables[0].defs().next().unwrap().type_())
            .is_equal_to(VariableType::ImportBinding);
        assert_that!(&scope.references().collect_vec()).is_empty();
    });
}

#[test]
fn test_should_import_namespaces() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "import * as ns from \"mod\";";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .source_type(SourceType::Module)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes).has_length(2);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.variables().collect_vec()).is_empty();
        assert_that!(&scope.references().collect_vec()).is_empty();

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Module);
        assert_that!(&scope.is_strict()).is_true();
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(1);
        assert_that!(&variables[0].name()).is_equal_to("ns");
        assert_that!(&variables[0].defs().next().unwrap().type_())
            .is_equal_to(VariableType::ImportBinding);
        assert_that!(&scope.references().collect_vec()).is_empty();
    });
}

#[test]
fn test_should_import_insided_names_1() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "import {x} from \"mod\";";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .source_type(SourceType::Module)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes).has_length(2);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.variables().collect_vec()).is_empty();
        assert_that!(&scope.references().collect_vec()).is_empty();

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Module);
        assert_that!(&scope.is_strict()).is_true();
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(1);
        assert_that!(&variables[0].name()).is_equal_to("x");
        assert_that!(&variables[0].defs().next().unwrap().type_())
            .is_equal_to(VariableType::ImportBinding);
        assert_that!(&scope.references().collect_vec()).is_empty();
    });
}

#[test]
fn test_should_import_insided_names_2() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "import {x as v} from \"mod\";";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .source_type(SourceType::Module)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes).has_length(2);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.variables().collect_vec()).is_empty();
        assert_that!(&scope.references().collect_vec()).is_empty();

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Module);
        assert_that!(&scope.is_strict()).is_true();
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(1);
        assert_that!(&variables[0].name()).is_equal_to("v");
        assert_that!(&variables[0].defs().next().unwrap().type_())
            .is_equal_to(VariableType::ImportBinding);
        assert_that!(&scope.references().collect_vec()).is_empty();
    });
}
