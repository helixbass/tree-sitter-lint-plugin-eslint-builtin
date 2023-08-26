#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    kind::Identifier,
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_class_c_f_equals_g() {
    tracing_subscribe();

    let code = "class C { f = g }";
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(13)
            .build()
            .unwrap(),
    );

    let scopes = manager.global_scope().child_scopes().collect_vec();

    assert_that(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that(&class_scope.references().collect_vec()).is_empty();

    let variables = class_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("C");

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let field_initializer_scope = &child_scopes[0];
    assert_that(&field_initializer_scope.type_()).is_equal_to(ScopeType::ClassFieldInitializer);

    assert_that(&field_initializer_scope.block().kind()).is_equal_to(Identifier);
    assert_that(&&*field_initializer_scope.block().text(&manager)).is_equal_to("g");

    assert_that(&field_initializer_scope.variable_scope()).is_equal_to(field_initializer_scope);

    let field_initializer_scope_references = field_initializer_scope.references().collect_vec();
    assert_that(&field_initializer_scope_references).has_length(1);
    assert_that(&&*field_initializer_scope_references[0].identifier().text(&manager)).is_equal_to("g");

    assert_that(&field_initializer_scope.variables().collect_vec()).is_empty();
}

#[test]
fn test_class_c_f() {
    tracing_subscribe();

    let code = "class C { f }";
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(13)
            .build()
            .unwrap(),
    );

    let scopes = manager.global_scope().child_scopes().collect_vec();

    assert_that(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that(&class_scope.references().collect_vec()).is_empty();

    assert_that(&class_scope.child_scopes().collect_vec()).is_empty();

    let variables = class_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("C");
}

#[test]
fn test_class_c_hash_f_equals_g() {
    tracing_subscribe();

    let code = "class C { #f = g }";
    let ast = parse(code);

    let manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(13)
            .build()
            .unwrap(),
    );

    let scopes = manager.global_scope().child_scopes().collect_vec();

    assert_that(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that(&class_scope.references().collect_vec()).is_empty();

    let variables = class_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("C");

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let field_initializer_scope = &child_scopes[0];
    assert_that(&field_initializer_scope.type_()).is_equal_to(ScopeType::ClassFieldInitializer);

    let field_initializer_scope_references = field_initializer_scope.references().collect_vec();
    assert_that(&field_initializer_scope_references).has_length(1);
    assert_that(&&*field_initializer_scope_references[0].identifier().text(&manager)).is_equal_to("g");

    assert_that(&field_initializer_scope.variables().collect_vec()).is_empty();
}
