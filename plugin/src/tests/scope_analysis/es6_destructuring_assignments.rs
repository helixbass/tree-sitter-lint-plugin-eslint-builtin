#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_pattern_in_var_in_for_in_statement() {
    tracing_subscribe();

    let code = "
        (function () {
            for (var [a, b, c] in array);
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(4);
    assert_that(&variables[0].name()).is_equal_to("arguments");
    assert_that(&variables[1].name()).is_equal_to("a");
    assert_that(&variables[2].name()).is_equal_to("b");
    assert_that(&variables[3].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&references[0].is_write()).is_true();
    assert_that(&references[0].partial()).is_true();
    assert_that(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that(&references[1].is_write()).is_true();
    assert_that(&references[1].partial()).is_true();
    assert_that(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that(&references[2].is_write()).is_true();
    assert_that(&references[2].partial()).is_true();
    assert_that(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that(&references[3].is_write()).is_false();
}

#[test]
fn test_pattern_in_let_in_for_in_statement() {
    tracing_subscribe();

    let code = "
        (function () {
            for (let [a, b, c] in array);
        }());
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

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::For);
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(3);
    assert_that(&variables[0].name()).is_equal_to("a");
    assert_that(&variables[1].name()).is_equal_to("b");
    assert_that(&variables[2].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&references[0].is_write()).is_true();
    assert_that(&references[0].partial()).is_true();
    assert_that(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that(&references[1].is_write()).is_true();
    assert_that(&references[1].partial()).is_true();
    assert_that(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that(&references[2].is_write()).is_true();
    assert_that(&references[2].partial()).is_true();
    assert_that(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that(&references[3].is_write()).is_false();
    assert_that(&references[3].resolved()).is_none();
}

#[test]
fn test_pattern_with_default_values_in_var_in_for_in_statement() {
    tracing_subscribe();

    let code = "
        (function () {
            for (var [a, b, c = d] in array);
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(2);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(4);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("a");
    assert_that!(&variables[2].name()).is_equal_to("b");
    assert_that!(&variables[3].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(6);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[0].is_write()).is_true();
    assert_that(&&*references[0].write_expr().unwrap().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[0].partial()).is_false();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[1].is_write()).is_false();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[3].is_write()).is_true();
    assert_that!(&references[3].partial()).is_true();
    assert_that!(&references[3].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[4].is_write()).is_true();
    assert_that(&&*references[4].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[4].partial()).is_true();
    assert_that!(&references[4].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[5].is_write()).is_false();
}

#[test]
fn test_pattern_with_default_values_in_let_in_for_in_statement() {
    tracing_subscribe();

    let code = "
        (function () {
            for (let [a, b, c = d] in array);
        }());
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

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that!(&implicit_left).has_length(2);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&implicit_left[0].from().type_()).is_equal_to(ScopeType::For);
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&implicit_left[1].from().type_()).is_equal_to(ScopeType::For);

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::For);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(3);
    assert_that!(&variables[0].name()).is_equal_to("a");
    assert_that!(&variables[1].name()).is_equal_to("b");
    assert_that!(&variables[2].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(6);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[0].is_write()).is_true();
    assert_that(&&*references[0].write_expr().unwrap().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[0].partial()).is_false();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[1].is_write()).is_false();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[2].is_write()).is_true();
    assert_that(&&*references[2].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[3].is_write()).is_true();
    assert_that!(&references[3].partial()).is_true();
    assert_that!(&references[3].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[4].is_write()).is_true();
    assert_that(&&*references[4].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[4].partial()).is_true();
    assert_that!(&references[4].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[5].is_write()).is_false();
    assert_that!(&references[5].resolved()).is_none();
}

#[test]
fn test_pattern_with_nested_default_values_in_var_in_for_in_statement() {
    tracing_subscribe();

    let code = "
        (function () {
            for (var [a, [b, c = d] = e] in array);
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(3);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("e");
    assert_that(&&*implicit_left[2].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(4);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("a");
    assert_that!(&variables[2].name()).is_equal_to("b");
    assert_that!(&variables[3].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(9);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[0].is_write()).is_true();
    assert_that(&&*references[0].write_expr().unwrap().text(&scope_manager)).is_equal_to("e");
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[1].is_write()).is_true();
    assert_that(&&*references[1].write_expr().unwrap().text(&scope_manager)).is_equal_to("e");
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[2].is_write()).is_true();
    assert_that(&&*references[2].write_expr().unwrap().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[2].partial()).is_false();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[3].is_write()).is_false();
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("e");
    assert_that!(&references[4].is_write()).is_false();
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[5].is_write()).is_true();
    assert_that(&&*references[5].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[5].partial()).is_true();
    assert_that!(&references[5].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[6].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[6].is_write()).is_true();
    assert_that(&&*references[6].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[6].partial()).is_true();
    assert_that!(&references[6].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[7].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[7].is_write()).is_true();
    assert_that(&&*references[7].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[7].partial()).is_true();
    assert_that!(&references[7].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[8].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[8].is_write()).is_false();
}

#[test]
fn test_pattern_with_nested_default_values_in_let_in_for_in_statement() {
    tracing_subscribe();

    let code = "
        (function () {
            for (let [a, [b, c = d] = e] in array);
        }());
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

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(3);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that(&implicit_left[0].from().type_()).is_equal_to(ScopeType::For);
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("e");
    assert_that(&implicit_left[1].from().type_()).is_equal_to(ScopeType::For);
    assert_that(&&*implicit_left[2].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that(&implicit_left[2].from().type_()).is_equal_to(ScopeType::For);

    let scope = &scopes[2];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::For);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(3);
    assert_that!(&variables[0].name()).is_equal_to("a");
    assert_that!(&variables[1].name()).is_equal_to("b");
    assert_that!(&variables[2].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(9);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[0].is_write()).is_true();
    assert_that(&&*references[0].write_expr().unwrap().text(&scope_manager)).is_equal_to("e");
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[1].is_write()).is_true();
    assert_that(&&*references[1].write_expr().unwrap().text(&scope_manager)).is_equal_to("e");
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[2].is_write()).is_true();
    assert_that(&&*references[2].write_expr().unwrap().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[2].partial()).is_false();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[3].is_write()).is_false();
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("e");
    assert_that!(&references[4].is_write()).is_false();
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[5].is_write()).is_true();
    assert_that(&&*references[5].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[5].partial()).is_true();
    assert_that!(&references[5].resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&&*references[6].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[6].is_write()).is_true();
    assert_that(&&*references[6].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[6].partial()).is_true();
    assert_that!(&references[6].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[7].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[7].is_write()).is_true();
    assert_that(&&*references[7].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[7].partial()).is_true();
    assert_that!(&references[7].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[8].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[8].is_write()).is_false();
    assert_that!(&references[8].resolved()).is_none();
}

#[test]
fn test_pattern_with_default_values_in_var_in_for_in_statement_separate_declarations() {
    tracing_subscribe();

    let code = "
        (function () {
            var a, b, c;
            for ([a, b, c = d] in array);
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(2);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(4);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("a");
    assert_that!(&variables[2].name()).is_equal_to("b");
    assert_that!(&variables[3].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(6);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[2].is_write()).is_true();
    assert_that(&&*references[2].write_expr().unwrap().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[2].partial()).is_false();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[3].is_write()).is_true();
    assert_that(&&*references[3].write_expr().unwrap().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[3].partial()).is_true();
    assert_that!(&references[3].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[4].is_write()).is_false();
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[5].is_write()).is_false();
}

#[test]
fn test_pattern_with_default_values_in_var_in_for_in_statement_separate_declarations_and_with_member_expression(
) {
    tracing_subscribe();

    let code = "
        (function () {
            var obj;
            for ([obj.a, obj.b, obj.c = d] in array);
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(2);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(2);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("obj");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(5);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that!(&references[0].is_write()).is_false();
    assert_that!(&references[0].is_read()).is_true();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that!(&references[1].is_write()).is_false();
    assert_that!(&references[1].is_read()).is_true();
    assert_that!(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that!(&references[2].is_write()).is_false();
    assert_that!(&references[2].is_read()).is_true();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[3].is_write()).is_false();
    assert_that!(&references[3].is_read()).is_true();
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[4].is_write()).is_false();
    assert_that!(&references[4].is_read()).is_true();
}

#[test]
fn test_array_pattern_in_var() {
    tracing_subscribe();

    let code = "
        (function () {
            var [a, b, c] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(4);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("a");
    assert_that!(&variables[2].name()).is_equal_to("b");
    assert_that!(&variables[3].name()).is_equal_to("c");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[3].is_write()).is_false();
}

#[test]
fn test_spread_element_in_var() {
    tracing_subscribe();

    let code = "
        (function () {
            var [a, b, ...rest] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(4);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("a");
    assert_that!(&variables[2].name()).is_equal_to("b");
    assert_that!(&variables[3].name()).is_equal_to("rest");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("rest");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[3].is_write()).is_false();

    let code = "
        (function () {
            var [a, b, ...[c, d, ...rest]] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(6);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("a");
    assert_that!(&variables[2].name()).is_equal_to("b");
    assert_that!(&variables[3].name()).is_equal_to("c");
    assert_that!(&variables[4].name()).is_equal_to("d");
    assert_that!(&variables[5].name()).is_equal_to("rest");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(6);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[3].is_write()).is_true();
    assert_that!(&references[3].partial()).is_true();
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("rest");
    assert_that!(&references[4].is_write()).is_true();
    assert_that!(&references[4].partial()).is_true();
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[5].is_write()).is_false();
}

#[test]
fn test_object_pattern_in_var() {
    tracing_subscribe();

    let code = "
        (function () {
            var {
                shorthand,
                key: value,
                hello: {
                    world
                }
            } = object;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("object");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(4);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("shorthand");
    assert_that!(&variables[2].name()).is_equal_to("value");
    assert_that!(&variables[3].name()).is_equal_to("world");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("shorthand");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("value");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved())
        .is_some()
        .is_equal_to(&variables[2]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("world");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved())
        .is_some()
        .is_equal_to(&variables[3]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("object");
    assert_that!(&references[3].is_write()).is_false();
}

#[test]
fn test_complex_pattern_in_var() {
    tracing_subscribe();

    let code = "
        (function () {
            var {
                shorthand,
                key: [ a, b, c, d, e ],
                hello: {
                    world
                }
            } = object;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("object");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(8);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("shorthand");
    assert_that!(&variables[2].name()).is_equal_to("a");
    assert_that!(&variables[3].name()).is_equal_to("b");
    assert_that!(&variables[4].name()).is_equal_to("c");
    assert_that!(&variables[5].name()).is_equal_to("d");
    assert_that!(&variables[6].name()).is_equal_to("e");
    assert_that!(&variables[7].name()).is_equal_to("world");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(8);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("shorthand");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[3].is_write()).is_true();
    assert_that!(&references[3].partial()).is_true();
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[4].is_write()).is_true();
    assert_that!(&references[4].partial()).is_true();
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("e");
    assert_that!(&references[5].is_write()).is_true();
    assert_that!(&references[5].partial()).is_true();
    assert_that(&&*references[6].identifier().text(&scope_manager)).is_equal_to("world");
    assert_that!(&references[6].is_write()).is_true();
    assert_that!(&references[6].partial()).is_true();
    assert_that(&&*references[7].identifier().text(&scope_manager)).is_equal_to("object");
    assert_that!(&references[7].is_write()).is_false();
}

#[test]
fn test_array_pattern_in_assignment_expression() {
    tracing_subscribe();

    let code = "
        (function () {
            [a, b, c] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(4);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that(&&*implicit_left[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that(&&*implicit_left[3].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved()).is_none();
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved()).is_none();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved()).is_none();
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[3].is_write()).is_false();
}

#[test]
fn test_array_pattern_with_member_expression_in_assignment_expression() {
    tracing_subscribe();

    let code = "
        (function () {
            var obj;
            [obj.a, obj.b, obj.c] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(1);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(2);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("obj");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that!(&references[0].is_write()).is_false();
    assert_that!(&references[0].is_read()).is_true();
    assert_that!(&references[0].resolved()).is_some().is_equal_to(&variables[1]);
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that!(&references[1].is_write()).is_false();
    assert_that!(&references[1].is_read()).is_true();
    assert_that!(&references[1].resolved()).is_some().is_equal_to(&variables[1]);
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that!(&references[2].is_write()).is_false();
    assert_that!(&references[2].is_read()).is_true();
    assert_that!(&references[2].resolved()).is_some().is_equal_to(&variables[1]);
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[3].is_write()).is_false();
    assert_that!(&references[3].is_read()).is_true();
}

#[test]
fn test_spread_element_in_assignment_expression() {
    tracing_subscribe();

    let code = "
        (function () {
            [a, b, ...rest] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(4);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that(&&*implicit_left[2].identifier().text(&scope_manager)).is_equal_to("rest");
    assert_that(&&*implicit_left[3].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved()).is_none();
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved()).is_none();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("rest");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved()).is_none();
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[3].is_write()).is_false();

    let code = "
        (function () {
            [a, b, ...[c, d, ...rest]] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(6);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that(&&*implicit_left[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that(&&*implicit_left[3].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that(&&*implicit_left[4].identifier().text(&scope_manager)).is_equal_to("rest");
    assert_that(&&*implicit_left[5].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(6);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved()).is_none();
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved()).is_none();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("c");
    assert_that!(&references[2].is_write()).is_true();
    assert_that!(&references[2].partial()).is_true();
    assert_that!(&references[2].resolved()).is_none();
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("d");
    assert_that!(&references[3].is_write()).is_true();
    assert_that!(&references[3].partial()).is_true();
    assert_that!(&references[3].resolved()).is_none();
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("rest");
    assert_that!(&references[4].is_write()).is_true();
    assert_that!(&references[4].partial()).is_true();
    assert_that!(&references[4].resolved()).is_none();
    assert_that(&&*references[5].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[5].is_write()).is_false();
}

#[test]
fn test_spread_element_with_member_expression_in_assignment_expression() {
    tracing_subscribe();

    let code = "
        (function () {
            [a, b, ...obj.rest] = array;
        }());
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

    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();
    let implicit_left = scope.implicit().left;
    assert_that(&implicit_left).has_length(4);
    assert_that(&&*implicit_left[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&&*implicit_left[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that(&&*implicit_left[2].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that(&&*implicit_left[3].identifier().text(&scope_manager)).is_equal_to("array");

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(4);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&references[0].is_write()).is_true();
    assert_that!(&references[0].partial()).is_true();
    assert_that!(&references[0].resolved()).is_none();
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("b");
    assert_that!(&references[1].is_write()).is_true();
    assert_that!(&references[1].partial()).is_true();
    assert_that!(&references[1].resolved()).is_none();
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("obj");
    assert_that!(&references[2].is_write()).is_false();
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("array");
    assert_that!(&references[3].is_write()).is_false();
}
