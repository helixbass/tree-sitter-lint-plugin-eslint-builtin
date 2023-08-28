#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    kind::Program,
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_materialize_rest_argument_in_scope() {
    tracing_subscribe();

    let code = "
        function foo(...bar) {
            return bar;
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
    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.block().kind()).is_equal_to(Program);
    assert_that!(&scope.is_strict()).is_false();
    assert_that!(&scope.variables().collect_vec()).has_length(1);

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(2);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("bar");
    assert_that(&&*variables[1]
        .defs()
        .next()
        .unwrap()
        .name()
        .text(&scope_manager))
    .is_equal_to("bar");
}
