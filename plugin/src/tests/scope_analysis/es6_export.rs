#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType, SourceType, VariableType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_should_create_variable_bindings() {
    tracing_subscribe();

    let code = "export var v;";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
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
    assert_that!(&variables[0].defs().next().unwrap().type_()).is_equal_to(VariableType::Variable);
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_should_create_function_declaration_bindings() {
    tracing_subscribe();

    let code = "export default function f(){};";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .source_type(SourceType::Module)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(&scopes).has_length(3);

    let scope = &scopes[0];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Module);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("f");
    assert_that!(&variables[0].defs().next().unwrap().type_())
        .is_equal_to(VariableType::FunctionName);
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_should_export_function_expression() {
    tracing_subscribe();

    let code = "export default function(){};";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .source_type(SourceType::Module)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(&scopes).has_length(3);

    let scope = &scopes[0];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Module);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_should_export_literal() {
    tracing_subscribe();

    let code = "export default 42;";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
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
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_should_refer_exported_references_1() {
    tracing_subscribe();

    let code = "const x = 1; export {x};";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
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
    assert_that!(&scope.variables().collect_vec()).has_length(1);
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(2);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("x");
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("x");
}

#[test]
fn test_should_refer_exported_references_2() {
    tracing_subscribe();

    let code = "const v = 1; export {v as x};";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
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
    assert_that!(&scope.variables().collect_vec()).has_length(1);
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(2);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("v");
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("v");
}

#[test]
fn test_should_not_refer_exported_references_from_other_source_1() {
    tracing_subscribe();

    let code = "export {x} from \"mod\";";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
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
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_should_not_refer_exported_references_from_other_source_2() {
    tracing_subscribe();

    let code = "export {v as x} from \"mod\";";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
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
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_should_not_refer_exported_references_from_other_source_3() {
    tracing_subscribe();

    let code = "export * from \"mod\";";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
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
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
}
