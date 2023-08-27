#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    kind::{ClassDeclaration, Program, MethodDefinition, Class, Function},
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_declaration_name_creates_class_scope() {
    tracing_subscribe();

    let code = "
        class Derived extends Base {
            constructor() {
            }
        }
        new Derived();
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
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("Derived");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(2);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("Base");
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("Derived");

    let scope = &scopes[1];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Class);
    assert_that!(&scope.block().kind()).is_equal_to(ClassDeclaration);
    assert_that!(&scope.is_strict()).is_true();
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("Derived");
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[2];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that!(&scope.block().kind()).is_equal_to(MethodDefinition);
    assert_that!(&scope.is_strict()).is_true();

    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("arguments");
    assert_that!(&scope.references().collect_vec()).is_empty();
}

#[test]
fn test_expression_name_creates_class_scope_1() {
    tracing_subscribe();

    let code = "
        (class Derived extends Base {
            constructor() {
            }
        });
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
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("Base");

    let scope = &scopes[1];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Class);
    assert_that!(&scope.block().kind()).is_equal_to(Class);
    assert_that!(&scope.is_strict()).is_true();
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("Derived");
    assert_that!(&scope.references().collect_vec()).is_empty();

    let scope = &scopes[2];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that!(&scope.block().kind()).is_equal_to(MethodDefinition);
}

#[test]
fn test_expression_name_creates_class_scope_2() {
    tracing_subscribe();

    let code = "
        (class extends Base {
            constructor() {
            }
        });
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
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("Base");

    let scope = &scopes[1];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Class);
    assert_that!(&scope.block().kind()).is_equal_to(Class);

    let scope = &scopes[2];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
    assert_that!(&scope.block().kind()).is_equal_to(MethodDefinition);
}

#[test]
fn test_computed_property_key_may_refer_variables() {
    tracing_subscribe();

    let code = "
        (function () {
            var yuyushiki = 42;
            (class {
                [yuyushiki]() {
                }

                [yuyushiki + 40]() {
                }
            });
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

    assert_that!(&scopes).has_length(5);

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
    assert_that!(&references).has_length(1);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("yuyushiki");

    let scope = &scopes[2];

    assert_that!(&scope.type_()).is_equal_to(ScopeType::Class);
    assert_that!(&scope.block().kind()).is_equal_to(Class);
    assert_that!(&scope.is_strict()).is_true();
    assert_that!(&scope.variables().collect_vec()).is_empty();
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(2);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("yuyushiki");
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("yuyushiki");
}

#[test]
fn test_regression_49() {
    tracing_subscribe();

    let code = "
        class Shoe {
            constructor() {
                //Shoe.x = true;
            }
        }
        let shoe = new Shoe();
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
    let variables = scope.variables().collect_vec();
    assert_that!(&variables).has_length(2);
    assert_that!(&variables[0].name()).is_equal_to("Shoe");
    assert_that!(&variables[1].name()).is_equal_to("shoe");
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(2);
    assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("shoe");
    assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("Shoe");
}
