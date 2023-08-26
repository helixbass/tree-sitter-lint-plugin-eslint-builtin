#![cfg(test)]

use std::collections::HashSet;

use itertools::Itertools;
use speculoos::prelude::*;
use squalid::SliceExtCloneOrd;

use crate::{
    kind::ClassStaticBlock,
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_class_c_a_b_c_d_e() {
    tracing_subscribe();

    let code = "class C { static { var a; let b; const c = 1; function d(){} class e {} } }";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(13)
            .build()
            .unwrap(),
    );

    let global_scope = scope_manager.global_scope();

    let variables = global_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("C");

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that(&class_scope.type_()).is_equal_to(ScopeType::Class);

    let variables = class_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("C");

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    let class_static_block_child_scopes = class_static_block_scope.child_scopes().collect_vec();
    assert_that(&class_static_block_child_scopes).has_length(2);
    let function_scope = &class_static_block_child_scopes[0];
    assert_that(&function_scope.type_()).is_equal_to(ScopeType::Function);
    assert_that(&function_scope.upper()).is_equal_to(class_static_block_scope);
    let nested_class_scope = &class_static_block_child_scopes[1];
    assert_that(&nested_class_scope.type_()).is_equal_to(ScopeType::Class);
    assert_that(&nested_class_scope.upper()).is_equal_to(class_static_block_scope);

    assert_that(&class_static_block_scope.upper()).is_equal_to(class_scope);
    assert_that(&class_static_block_scope.variable_scope()).is_equal_to(class_static_block_scope);

    assert_that(&class_static_block_scope.is_strict()).is_true();

    assert_that(&class_static_block_scope.function_expression_scope()).is_false();
    let static_block_node = class_static_block_scope.block();
    assert_that(&static_block_node.kind()).is_equal_to(ClassStaticBlock);

    assert_that(
        &scope_manager
            .acquire(static_block_node, Some(false))
            .as_ref(),
    )
    .is_equal_to(Some(class_static_block_scope));
    assert_that(
        &scope_manager
            .acquire(static_block_node, Some(true))
            .as_ref(),
    )
    .is_equal_to(Some(class_static_block_scope));

    assert_that(&scope_manager.get_declared_variables(static_block_node)).is_none();

    let expected_variable_names = vec!["a", "b", "c", "d", "e"];
    let expected_variable_names_owned = expected_variable_names
        .iter()
        .map(|&value| value.to_owned())
        .collect_vec();
    assert_that(
        &class_static_block_scope
            .variables()
            .map(|variable| variable.name().to_owned())
            .collect_vec(),
    )
    .is_equal_to(&expected_variable_names_owned);
    assert_that(
        &class_static_block_scope
            .set()
            .keys()
            .map(|value| (**value).to_owned())
            .collect_vec()
            .sorted(),
    )
    .is_equal_to(&expected_variable_names_owned);
    assert_that(
        &class_static_block_scope
            .set()
            .into_values()
            .collect::<HashSet<_>>(),
    )
    .is_equal_to(class_static_block_scope.variables().collect::<HashSet<_>>());
    class_static_block_scope.variables().for_each(|variable| {
        assert_that(&variable.scope()).is_equal_to(class_static_block_scope);
    });
}
