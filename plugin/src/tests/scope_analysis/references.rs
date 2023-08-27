#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_let_declaration_on_global_reference_on_global_should_be_resolved() {
    tracing_subscribe();

    let code = "let a = 0;";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_let_declaration_on_global_reference_in_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        let a = 0;
        function foo() {
            let b = a;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_let_declaration_on_global_reference_in_default_parameters_should_be_resolved() {
    tracing_subscribe();

    let code = "
        let a = 0;
        function foo(b = a) {
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_const_declaration_on_global_reference_on_global_should_be_resolved() {
    tracing_subscribe();

    let code = "const a = 0;";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_const_declaration_on_global_reference_in_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        const a = 0;
        function foo() {
            const b = a;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_var_declaration_on_global_reference_on_global_should_not_be_resolved() {
    tracing_subscribe();

    let code = "var a = 0;";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    assert_that(&scope.variables().collect_vec()).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved()).is_none();
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_var_declaration_on_global_reference_in_functions_should_not_be_resolved() {
    tracing_subscribe();

    let code = "
        var a = 0;
        function foo() {
            var b = a;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved()).is_none();
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_function_declaration_on_global_reference_on_global_should_not_be_resolved() {
    tracing_subscribe();

    let code = "
        function a() {}
        a();
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.variables().collect_vec()).has_length(1);
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&reference.resolved()).is_none();
    assert_that!(&reference.write_expr()).is_none();
    assert_that!(&reference.is_write()).is_false();
    assert_that!(&reference.is_read()).is_true();
}

#[test]
fn test_function_declaration_on_global_reference_in_functions_should_not_be_resolved() {
    tracing_subscribe();

    let code = "
        function a() {}
        function foo() {
            let b = a();
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(3);

    let scope = &scopes[2];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved()).is_none();
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_class_declaration_on_global_reference_on_global_should_be_resolved() {
    tracing_subscribe();

    let code = "
        class A {}
        let b = new A();
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("A");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_class_declaration_on_global_reference_in_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        class A {}
        function foo() {
            let b = new A();
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(3);

    let scope = &scopes[2];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("A");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}
