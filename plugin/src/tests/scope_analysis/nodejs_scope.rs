#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    kind::Program,
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType, SourceType, VariableType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_creates_a_function_scope_following_the_global_scope_immediately_when_nodejscope_true() {
    tracing_subscribe();

    let code = r#"
        "use strict";
        var hello = 20;
    "#;
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .nodejs_scope(true)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(2);
    assert_that!(scope_manager.is_global_return()).is_true();

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();
    assert_that(&scope.variables().collect_vec()).is_empty();

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_true();
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(2);
    assert_that(&variables[0].name()).is_equal_to("arguments");
    assert_that(&variables[1].name()).is_equal_to("hello");
}

#[test]
fn test_creates_a_function_scope_following_the_global_scope_immediately_when_source_type_commonjs()
{
    tracing_subscribe();

    let code = r#"
        "use strict";
        var hello = 20;
    "#;
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .source_type(SourceType::CommonJS)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(2);
    assert_that!(scope_manager.is_global_return()).is_true();

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();
    assert_that(&scope.variables().collect_vec()).is_empty();

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_true();
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(2);
    assert_that(&variables[0].name()).is_equal_to("arguments");
    assert_that(&variables[1].name()).is_equal_to("hello");
}

#[test]
fn test_creates_a_function_scope_following_the_global_scope_immediately_and_creates_module_scope() {
    tracing_subscribe();

    let code = "import {x as v} from 'mod';";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .nodejs_scope(true)
            .source_type(SourceType::Module)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(3);
    assert_that!(scope_manager.is_global_return()).is_true();

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();
    assert_that(&scope.variables().collect_vec()).is_empty();

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&scope.block().kind()).is_equal_to(Program);
    assert_that(&scope.is_strict()).is_false();
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("arguments");

    let scope = &scopes[2];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Module);
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("v");
    assert_that(&variables[0].defs().next().unwrap().type_())
        .is_equal_to(VariableType::ImportBinding);
    assert_that(&scope.references().collect_vec()).is_empty();
}
