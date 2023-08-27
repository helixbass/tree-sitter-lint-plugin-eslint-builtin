#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    kind::{CatchClause, Program, StatementBlock},
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_takes_binding_pattern() {
    tracing_subscribe();

    let code = "
        try {
        } catch ({ a, b, c, d }) {
            let e = 20;
            a;
            b;
            c;
            d;
        }
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

    assert_that!(&scopes).has_length(4);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.block().kind()).is_equal_to(Program);
    assert_that!(&scope.is_strict()).is_false();
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[1];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Block);
    assert_that!(&scope.block().kind()).is_equal_to(StatementBlock);
    assert_that!(&scope.is_strict()).is_false();
    assert_that!(&scope.variables().collect_vec()).is_empty();
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[2];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Catch);
    assert_that!(&scope.block().kind()).is_equal_to(CatchClause);
    assert_that!(&scope.is_strict()).is_false();

    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(4);
    assert_that!(&variables[0].name()).is_equal_to("a");
    assert_that!(&variables[1].name()).is_equal_to("b");
    assert_that!(&variables[2].name()).is_equal_to("c");
    assert_that!(&variables[3].name()).is_equal_to("d");
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[3];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Block);
    assert_that!(&scope.block().kind()).is_equal_to(StatementBlock);
    assert_that!(&scope.is_strict()).is_false();

    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("e");
    assert_that!(scope
        .references()
        .map(|ref_| ref_.identifier().text(&scope_manager).into_owned())
        .collect_vec())
    .is_equal_to(vec![
        "e".to_owned(),
        "a".to_owned(),
        "b".to_owned(),
        "c".to_owned(),
        "d".to_owned(),
    ]);
}
