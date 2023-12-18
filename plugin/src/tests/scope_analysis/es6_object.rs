#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    kind::{Function, MethodDefinition, Program},
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_method_definition() {
    tracing_subscribe();

    let code = "
        ({
            constructor() {
            }
        })
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

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that!(&scope.block().kind()).is_equal_to(MethodDefinition);
    assert_that!(&scope.is_strict()).is_false();
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_computed_property_key_may_refer_to_variable() {
    tracing_subscribe();

    let code = "
        (function () {
            var yuyushiki = 42;
            ({
                [yuyushiki]() {
                },

                [yuyushiki + 40]() {
                }
            })
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
    assert_that!(&scopes).has_length(4);

    let scope = &scopes[0];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
    assert_that!(&scope.block().kind()).is_equal_to(Program);
    assert_that!(&scope.is_strict()).is_false();

    let scope = &scopes[1];
    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that!(&scope.block().kind()).is_equal_to(Function);
    assert_that!(&scope.is_strict()).is_false();
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(2);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&variables[1].name()).is_equal_to("yuyushiki");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(3);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("yuyushiki");
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("yuyushiki");
    assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("yuyushiki");
}
