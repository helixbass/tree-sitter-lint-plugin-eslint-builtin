#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_default_parameter_creates_a_writable_reference_for_its_initialization() {
    tracing_subscribe();

    for (code, num_vars) in [
        ("function foo(a, b = 0) {}", 3),
        ("let foo = function(a, b = 0) {};", 3),
        ("let foo = (a, b = 0) => {};", 2),
    ] {
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

        let scope = &scopes[1];

        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(num_vars);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(1);

        let reference = &references[0];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("b");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(&variables[num_vars - 1]);
        assert_that(&reference.write_expr()).is_some();
        assert_that(&reference.is_write()).is_true();
        assert_that(&reference.is_read()).is_false();
    }
}

#[test]
fn test_default_parameter_creates_a_readable_reference_for_references_in_right() {
    tracing_subscribe();

    for (code, num_vars) in [
        (
            "
            let a;
            function foo(b = a) {}
        ",
            2,
        ),
        (
            "
            let a;
            let foo = function(b = a) {}
        ",
            2,
        ),
        (
            "
            let a;
            let foo = (b = a) => {};
        ",
            1,
        ),
    ] {
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

        let scope = &scopes[1];

        assert_that!(&scope.variables().collect_vec()).has_length(num_vars);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);

        let reference = &references[1];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(scopes[0].variables().next().unwrap());
        assert_that(&reference.write_expr()).is_none();
        assert_that(&reference.is_write()).is_false();
        assert_that(&reference.is_read()).is_true();
    }
}

#[test]
fn test_default_parameter_creates_a_readable_reference_for_references_in_right_for_const() {
    tracing_subscribe();

    for (code, num_vars) in [
        (
            "
            const a = 0;
            function foo(b = a) {}
        ",
            2,
        ),
        (
            "
            const a = 0;
            let foo = function(b = a) {}
        ",
            2,
        ),
        (
            "
            const a = 0;
            let foo = (b = a) => {};
        ",
            1,
        ),
    ] {
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

        let scope = &scopes[1];

        assert_that!(&scope.variables().collect_vec()).has_length(num_vars);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);

        let reference = &references[1];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(scopes[0].variables().next().unwrap());
        assert_that(&reference.write_expr()).is_none();
        assert_that(&reference.is_write()).is_false();
        assert_that(&reference.is_read()).is_true();
    }
}

#[test]
fn test_default_parameter_creates_a_readable_reference_for_references_in_right_partial() {
    tracing_subscribe();

    for (code, num_vars) in [
        (
            "
            let a;
            function foo(b = a.c) {}
        ",
            2,
        ),
        (
            "
            let a;
            let foo = function(b = a.c) {}
        ",
            2,
        ),
        (
            "
            let a;
            let foo = (b = a.c) => {};
        ",
            1,
        ),
    ] {
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

        let scope = &scopes[1];

        assert_that!(&scope.variables().collect_vec()).has_length(num_vars);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);

        let reference = &references[1];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(scopes[0].variables().next().unwrap());
        assert_that(&reference.write_expr()).is_none();
        assert_that(&reference.is_write()).is_false();
        assert_that(&reference.is_read()).is_true();
    }
}

#[test]
fn test_default_parameter_creates_a_readable_reference_for_references_in_rights_nested_scope() {
    tracing_subscribe();

    for code in [
        "
            let a;
            function foo(b = function() { return a; }) {}
        ",
        "
            let a;
            let foo = function(b = function() { return a; }) {}
        ",
        "
            let a;
            let foo = (b = function() { return a; }) => {};
        ",
    ] {
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

        let scope = &scopes[2];

        assert_that!(&scope.variables().collect_vec()).has_length(1);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(1);

        let reference = &references[0];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(scopes[0].variables().next().unwrap());
        assert_that(&reference.write_expr()).is_none();
        assert_that(&reference.is_write()).is_false();
        assert_that(&reference.is_read()).is_true();
    }
}

#[test]
fn test_default_parameter_creates_a_readable_reference_for_references_in_right_resolved_to_outer_scope(
) {
    tracing_subscribe();

    for (code, num_vars) in [
        (
            "
            let a;
            function foo(b = a) { let a; }
        ",
            3,
        ),
        (
            "
            let a;
            let foo = function(b = a) { let a; }
        ",
            3,
        ),
        (
            "
            let a;
            let foo = (b = a) => { let a; };
        ",
            2,
        ),
    ] {
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

        let scope = &scopes[1];

        assert_that!(&scope.variables().collect_vec()).has_length(num_vars);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);

        let reference = &references[1];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(scopes[0].variables().next().unwrap());
        assert_that(&reference.write_expr()).is_none();
        assert_that(&reference.is_write()).is_false();
        assert_that(&reference.is_read()).is_true();
    }
}

#[test]
fn test_default_parameter_creates_a_readable_reference_for_references_in_right_resolved_to_parameter(
) {
    tracing_subscribe();

    for (code, num_vars) in [
        (
            "
            let a;
            function foo(b = a, a) { }
        ",
            3,
        ),
        (
            "
            let a;
            let foo = function(b = a, a) { }
        ",
            3,
        ),
        (
            "
            let a;
            let foo = (b = a, a) => { };
        ",
            2,
        ),
    ] {
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

        let scope = &scopes[1];

        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(num_vars);
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(2);

        let reference = &references[1];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(variables.last().unwrap());
        assert_that(&reference.write_expr()).is_none();
        assert_that(&reference.is_write()).is_false();
        assert_that(&reference.is_read()).is_true();
    }
}

#[test]
fn test_default_parameter_creates_a_readable_reference_for_references_in_right_nested_scope_resolved_to_outer_scope(
) {
    tracing_subscribe();

    for code in [
        "
            let a;
            function foo(b = function(){ a }) { let a; }
        ",
        "
            let a;
            let foo = function(b = function(){ a }) { let a; }
        ",
        "
            let a;
            let foo = (b = function(){ a }) => { let a; };
        ",
    ] {
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

        let scope = &scopes[2];

        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(1);

        let reference = &references[0];

        assert_that!(&reference.from()).is_equal_to(scope);
        assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
        assert_that(&reference.resolved())
            .is_some()
            .is_equal_to(scopes[0].variables().next().unwrap());
        assert_that(&reference.write_expr()).is_none();
        assert_that(&reference.is_write()).is_false();
        assert_that(&reference.is_read()).is_true();
    }
}
