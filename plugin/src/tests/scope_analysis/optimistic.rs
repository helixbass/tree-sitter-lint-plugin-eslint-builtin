#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_direct_call_to_eval() {
    tracing_subscribe();

    let code = "
        function outer() {
            eval(str);
            var i = 20;
            function inner() {
                i;
            }
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .optimistic(true)
            .build()
            .unwrap(),
    );

    assert_that(
        &scope_manager
            .scopes()
            .map(|scope| {
                scope
                    .variables()
                    .map(|variable| variable.name().to_owned())
                    .collect_vec()
            })
            .collect_vec(),
    )
    .is_equal_to(vec![
        vec!["outer".to_owned()],
        vec!["arguments".to_owned(), "i".to_owned(), "inner".to_owned()],
        vec!["arguments".to_owned()],
    ]);
}

#[test]
fn test_with_statement() {
    tracing_subscribe();

    let code = "
        function outer() {
            eval(str);
            var i = 20;
            with (obj) {
                i;
            }
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .optimistic(true)
            .build()
            .unwrap(),
    );

    assert_that(
        &scope_manager
            .scopes()
            .map(|scope| {
                scope
                    .variables()
                    .map(|variable| variable.name().to_owned())
                    .collect_vec()
            })
            .collect_vec(),
    )
    .is_equal_to(vec![
        vec!["outer".to_owned()],
        vec!["arguments".to_owned(), "i".to_owned()],
        vec![],
    ]);
}
