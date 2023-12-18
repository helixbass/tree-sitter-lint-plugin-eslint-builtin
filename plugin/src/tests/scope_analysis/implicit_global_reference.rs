#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    scope::{analyze, VariableType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_assignments_global_scope() {
    tracing_subscribe();

    let code = "
        var x = 20;
        x = 300;
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that!(scope_manager
        .scopes()
        .map(|scope| scope
            .variables()
            .map(|variable| variable.defs().map(|def| def.type_()).collect_vec())
            .collect_vec())
        .collect_vec())
    .is_equal_to(vec![vec![vec![VariableType::Variable]]]);
}

#[test]
fn test_assignments_global_scope_without_definition() {
    tracing_subscribe();

    let code = "
        x = 300;
        x = 300;
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that!(scope_manager
        .scopes()
        .map(|scope| scope
            .variables()
            .map(|variable| variable.defs().map(|def| def.type_()).collect_vec())
            .collect_vec())
        .collect_vec())
    .is_equal_to(vec![vec![]]);

    assert_that!(scope_manager
        .scopes()
        .next()
        .unwrap()
        .implicit()
        .variables
        .iter()
        .map(|variable| variable.name().to_owned())
        .collect_vec())
    .is_equal_to(vec!["x".to_owned()]);
}

#[test]
fn test_assignments_global_scope_without_definition_eval() {
    tracing_subscribe();

    let code = "
        function inner() {
            eval(str);
            x = 300;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that!(scope_manager
        .scopes()
        .map(|scope| scope
            .variables()
            .map(|variable| variable.defs().map(|def| def.type_()).collect_vec())
            .collect_vec())
        .collect_vec())
    .is_equal_to(vec![vec![vec![VariableType::FunctionName]], vec![vec![]]]);
    assert_that!(scope_manager
        .scopes()
        .next()
        .unwrap()
        .implicit()
        .variables
        .iter()
        .map(|variable| variable.name().to_owned())
        .collect_vec())
    .is_equal_to(vec![]);
}

#[test]
fn test_assignment_leaks() {
    tracing_subscribe();

    let code = "
        function outer() {
            x = 20;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that!(scope_manager
        .scopes()
        .map(|scope| scope
            .variables()
            .map(|variable| variable.name().to_owned())
            .collect_vec())
        .collect_vec())
    .is_equal_to(vec![vec!["outer".to_owned()], vec!["arguments".to_owned()]]);
    assert_that!(scope_manager
        .scopes()
        .next()
        .unwrap()
        .implicit()
        .variables
        .iter()
        .map(|variable| variable.name().to_owned())
        .collect_vec())
    .is_equal_to(vec!["x".to_owned()]);
}

#[test]
fn test_assignment_doesnt_leak() {
    tracing_subscribe();

    let code = "
        function outer() {
            function inner() {
                x = 20;
            }
            var x;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that!(scope_manager
        .scopes()
        .map(|scope| scope
            .variables()
            .map(|variable| variable.name().to_owned())
            .collect_vec())
        .collect_vec())
    .is_equal_to(vec![
        vec!["outer".to_owned()],
        vec!["arguments".to_owned(), "inner".to_owned(), "x".to_owned()],
        vec!["arguments".to_owned()],
    ]);
    assert_that!(scope_manager
        .scopes()
        .next()
        .unwrap()
        .implicit()
        .variables
        .iter()
        .map(|variable| variable.name().to_owned())
        .collect_vec())
    .is_equal_to(vec![]);
}

#[test]
fn test_for_in_statement_leaks() {
    tracing_subscribe();

    let code = "
        function outer() {
            for (x in y) { }
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that!(scope_manager
        .scopes()
        .map(|scope| scope
            .variables()
            .map(|variable| variable.name().to_owned())
            .collect_vec())
        .collect_vec())
    .is_equal_to(vec![vec!["outer".to_owned()], vec!["arguments".to_owned()]]);
    assert_that!(scope_manager
        .scopes()
        .next()
        .unwrap()
        .implicit()
        .variables
        .iter()
        .map(|variable| variable.name().to_owned())
        .collect_vec())
    .is_equal_to(vec!["x".to_owned()]);
}

#[test]
fn test_for_in_statement_doesnt_leak() {
    tracing_subscribe();

    let code = "
        function outer() {
            function inner() {
                for (x in y) { }
            }
            var x;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code, Default::default());

    assert_that!(scope_manager
        .scopes()
        .map(|scope| scope
            .variables()
            .map(|variable| variable.name().to_owned())
            .collect_vec())
        .collect_vec())
    .is_equal_to(vec![
        vec!["outer".to_owned()],
        vec!["arguments".to_owned(), "inner".to_owned(), "x".to_owned()],
        vec!["arguments".to_owned()],
    ]);
    assert_that!(scope_manager
        .scopes()
        .next()
        .unwrap()
        .implicit()
        .variables
        .iter()
        .map(|variable| variable.name().to_owned())
        .collect_vec())
    .is_equal_to(vec![]);
}
