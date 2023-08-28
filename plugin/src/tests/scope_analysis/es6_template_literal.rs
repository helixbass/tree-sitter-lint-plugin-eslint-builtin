#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe}, kind::{Program, Function},
};

#[test]
fn test_refer_variables() {
    tracing_subscribe();

    let code = "
        (function () {
            let i, j, k;
            function testing() { }
            let template = testing`testing ${i} and ${j}`
            return template;
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
    assert_that!(&scope.block().kind()).is_equal_to(Program);
    assert_that!(&scope.is_strict()).is_false();
    assert_that!(&scope.variables().collect_vec()).is_empty();

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that!(&scope.block().kind()).is_equal_to(Function);
    assert_that!(&scope.is_strict()).is_false();
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(6);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("i");
    assert_that!(&variables[2].name()).is_equal_to("j");
    assert_that!(&variables[3].name()).is_equal_to("k");
    assert_that!(&variables[4].name()).is_equal_to("testing");
    assert_that!(&variables[5].name()).is_equal_to("template");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(5);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("template");
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("testing");
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("i");
    assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("j");
    assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("template");
}
