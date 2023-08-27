#![cfg(test)]

use std::collections::HashSet;

use itertools::Itertools;
use speculoos::prelude::*;
use squalid::{break_if_none, SliceExtCloneOrd};

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

#[test]
fn test_class_c_f_f() {
    tracing_subscribe();

    let code = "class C { static { function f(){} f(); } }";
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

    assert_that(&global_scope.references().collect_vec()).is_empty();

    assert_that(&global_scope.through().collect_vec()).is_empty();

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that(&class_scope.references().collect_vec()).is_empty();

    assert_that(&class_scope.through().collect_vec()).is_empty();

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    assert_that(&class_static_block_scope.through().collect_vec()).is_empty();

    let variables = class_static_block_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("f");

    let references = class_static_block_scope.references().collect_vec();
    assert_that(&references).has_length(1);
    assert_that(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[0]);

    let f = &variables[0];
    let f_references = f.references().collect_vec();
    assert_that(&f_references).has_length(1);
    assert_that(&f_references[0].from()).is_equal_to(class_static_block_scope);
    assert_that(&f_references[0])
        .is_equal_to(&class_static_block_scope.references().collect_vec()[0]);
}

#[test]
fn test_class_c_a_if_x_a() {
    tracing_subscribe();

    let code = "class C { static { a = 1; if (this.x) { var a; } } }";
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

    assert_that(&global_scope.references().collect_vec()).is_empty();

    assert_that(&global_scope.through().collect_vec()).is_empty();

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that(&class_scope.references().collect_vec()).is_empty();

    assert_that(&class_scope.through().collect_vec()).is_empty();

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    assert_that(&class_static_block_scope.through().collect_vec()).is_empty();

    let variables = class_static_block_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("a");

    let references = class_static_block_scope.references().collect_vec();
    assert_that(&references).has_length(1);
    assert_that(&references[0].resolved())
        .is_some()
        .is_equal_to(&variables[0]);

    let a = &variables[0];
    let a_references = a.references().collect_vec();
    assert_that(&a_references).has_length(1);
    assert_that(&a_references[0].from()).is_equal_to(class_static_block_scope);

    let child_scopes = class_static_block_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let block_scope = &child_scopes[0];
    assert_that(&block_scope.type_()).is_equal_to(ScopeType::Block);

    assert_that(&block_scope.variables().collect_vec()).is_empty();

    assert_that(&block_scope.references().collect_vec()).is_empty();
}

#[test]
fn test_class_c_if_x_a_a() {
    tracing_subscribe();

    let code = "class C { static { if (this.x) { var a; a = 1; } } }";
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

    assert_that(&global_scope.references().collect_vec()).is_empty();

    assert_that(&global_scope.through().collect_vec()).is_empty();

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that(&class_scope.references().collect_vec()).is_empty();

    assert_that(&class_scope.through().collect_vec()).is_empty();

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    assert_that(&class_static_block_scope.through().collect_vec()).is_empty();

    let variables = class_static_block_scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    assert_that(&variables[0].name()).is_equal_to("a");

    assert_that(&class_static_block_scope.references().collect_vec()).is_empty();

    let child_scopes = class_static_block_scope.child_scopes().collect_vec();
    assert_that(&child_scopes).has_length(1);
    let block_scope = &child_scopes[0];
    assert_that(&block_scope.type_()).is_equal_to(ScopeType::Block);

    let a = &variables[0];
    let a_references = a.references().collect_vec();
    assert_that(&a_references).has_length(1);
    assert_that(&a_references[0].from()).is_equal_to(block_scope);

    assert_that(&block_scope.variables().collect_vec()).is_empty();

    let block_scope_references = block_scope.references().collect_vec();
    assert_that(&block_scope_references).has_length(1);
    assert_that(&block_scope_references[0].resolved())
        .is_some()
        .is_equal_to(a);
}

#[test]
fn test_class_c_a_foo_bar_b_a_baz_b() {
    tracing_subscribe();

    let code = "class C { static { const { a } = this.foo; if (this.bar) { const b = a + 1; this.baz(b); } } }";
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

    assert_that!(&global_scope.references().collect_vec()).is_empty();

    assert_that!(&global_scope.through().collect_vec()).is_empty();

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that!(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that!(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that!(&class_scope.references().collect_vec()).is_empty();

    assert_that!(&class_scope.through().collect_vec()).is_empty();

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that!(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    let child_scopes = class_static_block_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(1);
    let block_scope = &child_scopes[0];
    assert_that!(&block_scope.type_()).is_equal_to(ScopeType::Block);

    let variables = class_static_block_scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("a");

    let a = &variables[0];
    let a_references = a.references().collect_vec();
    assert_that!(&a_references).has_length(2);
    assert_that!(&a_references[0].is_write_only()).is_true();
    assert_that!(&a_references[0].from()).is_equal_to(class_static_block_scope);
    assert_that!(&a_references[1].is_read_only()).is_true();
    assert_that!(&a_references[1].from()).is_equal_to(block_scope);

    assert_that!(&class_static_block_scope.through().collect_vec()).is_empty();

    let variables = block_scope.variables().collect_vec();
    assert_that!(&variables).has_length(1);
    assert_that!(&variables[0].name()).is_equal_to("b");

    let b = &variables[0];
    let b_references = b.references().collect_vec();
    assert_that!(&b_references).has_length(2);
    assert_that!(&b_references[0].is_write_only()).is_true();
    assert_that!(&b_references[0].from()).is_equal_to(block_scope);
    assert_that!(&b_references[1].is_read_only()).is_true();
    assert_that!(&b_references[1].from()).is_equal_to(block_scope);

    let through = block_scope.through().collect_vec();
    assert_that!(&through).has_length(1);
    assert_that!(&through[0].resolved())
        .is_some()
        .is_equal_to(a);
}

#[test]
fn test_class_c_c_x() {
    tracing_subscribe();

    let code = "class C { static { C.x; } }";
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

    assert_that!(&global_scope.references().collect_vec()).is_empty();

    assert_that!(&global_scope.through().collect_vec()).is_empty();

    assert_that!(&global_scope.set()).contains_key(&"C".into());

    assert_that!(&global_scope.set()["C"].references().collect_vec()).is_empty();

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that!(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that!(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that!(&class_scope.references().collect_vec()).is_empty();

    assert_that!(&class_scope.through().collect_vec()).is_empty();

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that!(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    assert_that!(&class_scope.set()).contains_key(&"C".into());

    let c = &class_scope.set()["C"];
    let references = c.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that!(&references[0].from()).is_equal_to(class_static_block_scope);

    let references = class_static_block_scope.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(c);

    let through = class_static_block_scope.through().collect_vec();
    assert_that!(&through).has_length(1);
    assert_that!(&through[0].resolved())
        .is_some()
        .is_equal_to(c);

    assert_that!(&class_static_block_scope.variables().collect_vec()).is_empty();
}

#[test]
fn test_a_class_c_lbl_b_a() {
    tracing_subscribe();

    let code = "let a; class C { static { lbl: { this.b = a } } }";
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

    assert_that!(&global_scope.references().collect_vec()).is_empty();

    assert_that!(&global_scope.through().collect_vec()).is_empty();

    assert_that!(&global_scope.set()).contains_key(&"a".into());

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that!(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that!(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that!(&class_scope.references().collect_vec()).is_empty();

    let a = &global_scope.set()["a"];
    let through = class_scope.through().collect_vec();
    assert_that!(&through).has_length(1);
    assert_that!(&through[0].resolved())
        .is_some()
        .is_equal_to(a);

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that!(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    assert_that!(&class_static_block_scope.references().collect_vec()).is_empty();

    let through = class_static_block_scope.through().collect_vec();
    assert_that!(&through).has_length(1);
    assert_that!(&through[0].resolved())
        .is_some()
        .is_equal_to(a);

    let child_scopes = class_static_block_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(1);
    let block_scope = &child_scopes[0];
    assert_that!(&block_scope.type_()).is_equal_to(ScopeType::Block);

    let references = block_scope.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that!(&references[0].resolved())
        .is_some()
        .is_equal_to(a);

    let through = block_scope.through().collect_vec();
    assert_that!(&through).has_length(1);
    assert_that!(&through[0].resolved())
        .is_some()
        .is_equal_to(a);
}

#[test]
fn test_class_c_static_a() {
    tracing_subscribe();

    let code = "class C { static { a; } }";
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

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that!(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that!(&class_scope.type_()).is_equal_to(ScopeType::Class);

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(1);
    let class_static_block_scope = &child_scopes[0];
    assert_that!(&class_static_block_scope.type_()).is_equal_to(ScopeType::ClassStaticBlock);

    let references = class_static_block_scope.references().collect_vec();
    assert_that!(&references).has_length(1);
    assert_that!(&references[0].resolved()).is_none();

    let reference = &references[0];
    let mut scope = class_static_block_scope.clone();
    loop {
        assert_that!(scope.through().next())
            .is_some()
            .is_equal_to(reference);
        scope = break_if_none!(scope.maybe_upper());
    }
}

#[test]
fn test_a_class_c_a_a_a_a() {
    tracing_subscribe();

    let code = "let a; class C { static { let a; a; } static { a; let a; } }";
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

    assert_that!(&global_scope.set()).contains_key(&"a".into());

    let a = &global_scope.set()["a"];

    assert_that!(&a.references().collect_vec()).is_empty();

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that!(&scopes).has_length(1);
    let class_scope = &scopes[0];
    assert_that!(&class_scope.type_()).is_equal_to(ScopeType::Class);

    assert_that!(&class_scope.references().collect_vec()).is_empty();

    assert_that!(&class_scope.through().collect_vec()).is_empty();

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(2);
    assert_that!(&child_scopes[0].type_()).is_equal_to(ScopeType::ClassStaticBlock);
    assert_that!(&child_scopes[0].block().kind()).is_equal_to(ClassStaticBlock);
    assert_that!(&child_scopes[1].type_()).is_equal_to(ScopeType::ClassStaticBlock);
    assert_that!(&child_scopes[1].block().kind()).is_equal_to(ClassStaticBlock);
    assert_that!(&child_scopes[0]).is_not_equal_to(&child_scopes[1]);
    assert_that!(&child_scopes[0].block()).is_not_equal_to(child_scopes[1].block());
    assert_that!(&child_scopes[0].upper()).is_equal_to(&child_scopes[1].upper());

    for class_static_block_scope in &child_scopes {
        let variables = class_static_block_scope.variables().collect_vec();
        assert_that!(&variables).has_length(1);

        let variable = &variables[0];

        assert_that!(&variable.scope()).is_equal_to(class_static_block_scope);
        assert_that!(&variable.name()).is_equal_to("a");
        let references = variable.references().collect_vec();
        assert_that!(&references).has_length(1);

        let reference = &references[0];
        let reference_identifier = reference.identifier();
        let static_block_node = class_static_block_scope.block();

        assert_that!(&reference.from()).is_equal_to(class_static_block_scope);
        assert_that!(&static_block_node.range().start_byte)
            .is_less_than_or_equal_to(reference_identifier.range().start_byte);
        assert_that!(&reference_identifier.range().end_byte)
            .is_less_than_or_equal_to(static_block_node.range().end_byte);
    }
}

#[test]
fn test_a_class_c_a_a_a_a_a_a_a() {
    tracing_subscribe();

    let code = "let a; class C { [a]; static { let a; } [a]; static { function a(){} } [a]; static { var a; } [a]; }";
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

    assert_that!(&global_scope.set()).contains_key(&"a".into());

    let a = &global_scope.set()["a"];

    let scopes = global_scope.child_scopes().collect_vec();
    assert_that!(&scopes).has_length(1);
    let class_scope = &scopes[0];

    let a_references = a.references().collect_vec();
    assert_that!(&a_references).has_length(4);
    a_references.iter().for_each(|reference| {
        assert_that!(&reference.from()).is_equal_to(class_scope);
    });

    let child_scopes = class_scope.child_scopes().collect_vec();
    assert_that!(&child_scopes).has_length(3);
    assert_that!(&child_scopes[0].type_()).is_equal_to(ScopeType::ClassStaticBlock);
    assert_that!(&child_scopes[1].type_()).is_equal_to(ScopeType::ClassStaticBlock);
    assert_that!(&child_scopes[2].type_()).is_equal_to(ScopeType::ClassStaticBlock);

    child_scopes.iter().for_each(|class_static_block_scope| {
        let variables = class_static_block_scope.variables().collect_vec();
        assert_that!(&variables).has_length(1);

        let variable = &variables[0];

        assert_that!(&variable.name()).is_equal_to("a");
        assert_that!(&variable.references().collect_vec()).is_empty();
    });
}
