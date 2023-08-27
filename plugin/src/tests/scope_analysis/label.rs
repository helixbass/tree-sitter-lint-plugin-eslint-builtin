#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_should_not_create_variables() {
    tracing_subscribe();

    let code = "function bar() { q: for(;;) { break q; } }";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        Default::default()
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(2);

    let scope = &scopes[0];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Global);
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("bar");
    assert_that(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[1];

    assert_that(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("arguments");
    assert_that(&scope.is_arguments_materialized()).is_false();
    assert_that(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_should_count_child_node_references() {
    tracing_subscribe();

    let code = "
        var foo = 5;

        label: while (true) {
          console.log(foo);
          break;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        Default::default()
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(scopes).has_length(1);

    let scope = &scopes[0];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("foo");
    let through = scope.through().collect_vec();
    assert_that!(&through).has_length(3);
    assert_that(&&*through[2].identifier().text(&scope_manager)).is_equal_to("foo");
    assert_that(&through[2].is_read()).is_true();
}
