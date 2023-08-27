#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_let_declaration_on_global_reference_on_global_should_be_resolved() {
    tracing_subscribe();

    let code = "let a = 0;";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_let_declaration_on_global_reference_in_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        let a = 0;
        function foo() {
            let b = a;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_let_declaration_on_global_reference_in_default_parameters_should_be_resolved() {
    tracing_subscribe();

    let code = "
        let a = 0;
        function foo(b = a) {
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_const_declaration_on_global_reference_on_global_should_be_resolved() {
    tracing_subscribe();

    let code = "const a = 0;";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_const_declaration_on_global_reference_in_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        const a = 0;
        function foo() {
            const b = a;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_var_declaration_on_global_reference_on_global_should_not_be_resolved() {
    tracing_subscribe();

    let code = "var a = 0;";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    assert_that(&scope.variables().collect_vec()).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved()).is_none();
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_var_declaration_on_global_reference_in_functions_should_not_be_resolved() {
    tracing_subscribe();

    let code = "
        var a = 0;
        function foo() {
            var b = a;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved()).is_none();
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_function_declaration_on_global_reference_on_global_should_not_be_resolved() {
    tracing_subscribe();

    let code = "
        function a() {}
        a();
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that!(&scopes).has_length(2);

    let scope = &scopes[0];

    assert_that!(&scope.variables().collect_vec()).has_length(1);
    let references = scope.references().collect_vec();
    assert_that!(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that!(&reference.resolved()).is_none();
    assert_that!(&reference.write_expr()).is_none();
    assert_that!(&reference.is_write()).is_false();
    assert_that!(&reference.is_read()).is_true();
}

#[test]
fn test_function_declaration_on_global_reference_in_functions_should_not_be_resolved() {
    tracing_subscribe();

    let code = "
        function a() {}
        function foo() {
            let b = a();
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(3);

    let scope = &scopes[2];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved()).is_none();
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_class_declaration_on_global_reference_on_global_should_be_resolved() {
    tracing_subscribe();

    let code = "
        class A {}
        let b = new A();
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("A");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_class_declaration_on_global_reference_in_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        class A {}
        function foo() {
            let b = new A();
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(3);

    let scope = &scopes[2];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("A");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[0].variables().next().unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_let_declaration_in_functions_reference_on_the_function_should_be_resolved() {
    tracing_subscribe();

    let code = "
        function foo() {
            let a = 0;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_let_declaration_in_functions_reference_in_nested_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        function foo() {
            let a = 0;
            function bar() {
                let b = a;
            }
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(3);

    let scope = &scopes[2];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[1].variables().nth(1).unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_var_declaration_in_functions_reference_on_the_function_should_be_resolved() {
    tracing_subscribe();

    let code = "
        function foo() {
            var a = 0;
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(2);

    let scope = &scopes[1];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[1]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_var_declaration_in_functions_reference_in_nested_functions_should_be_resolved() {
    tracing_subscribe();

    let code = "
        function foo() {
            var a = 0;
            function bar() {
                var b = a;
            }
        }
    ";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(3);

    let scope = &scopes[2];

    assert_that(&scope.variables().collect_vec()).has_length(2);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(2);

    let reference = &references[1];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&scopes[1].variables().nth(1).unwrap());
    assert_that(&reference.write_expr()).is_none();
    assert_that(&reference.is_write()).is_false();
    assert_that(&reference.is_read()).is_true();
}

#[test]
fn test_let_a_1_reference_should_be_resolved() {
    tracing_subscribe();

    let code = "let [a] = [1];";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_let_a_a_1_reference_should_be_resolved() {
    tracing_subscribe();

    let code = "let {a} = {a: 1};";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_let_a_a_a_a_1_reference_should_be_resolved() {
    tracing_subscribe();

    let code = "let {a: {a}} = {a: {a: 1}};";
    let ast = parse(code);

    let scope_manager = analyze(&ast, code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .build()
            .unwrap(),
    );

    let scopes = scope_manager.scopes().collect_vec();
    assert_that(&scopes).has_length(1);

    let scope = &scopes[0];

    let variables = scope.variables().collect_vec();
    assert_that(&variables).has_length(1);
    let references = scope.references().collect_vec();
    assert_that(&references).has_length(1);

    let reference = &references[0];

    assert_that!(&reference.from()).is_equal_to(scope);
    assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
    assert_that(&reference.resolved())
        .is_some()
        .is_equal_to(&variables[0]);
    assert_that(&reference.write_expr()).is_some();
    assert_that(&reference.is_write()).is_true();
    assert_that(&reference.is_read()).is_false();
}

#[test]
fn test_reference_init_should_be_boolean_of_whether_initialized_or_not() {
    tracing_subscribe();

    let true_codes = [
        "var a = 0;",
        "let a = 0;",
        "const a = 0;",
        "var [a] = [];",
        "let [a] = [];",
        "const [a] = [];",
        "var [a = 1] = [];",
        "let [a = 1] = [];",
        "const [a = 1] = [];",
        "var {a} = {};",
        "let {a} = {};",
        "const {a} = {};",
        "var {b: a} = {};",
        "let {b: a} = {};",
        "const {b: a} = {};",
        "var {b: a = 0} = {};",
        "let {b: a = 0} = {};",
        "const {b: a = 0} = {};",
        "for (var a in []);",
        "for (let a in []);",
        "for (var [a] in []);",
        "for (let [a] in []);",
        "for (var [a = 0] in []);",
        "for (let [a = 0] in []);",
        "for (var {a} in []);",
        "for (let {a} in []);",
        "for (var {a = 0} in []);",
        "for (let {a = 0} in []);",
        "new function(a = 0) {}",
        "new function([a = 0] = []) {}",
        "new function({b: a = 0} = {}) {}"
    ];

    true_codes.into_iter().for_each(|code| {
        let ast = parse(code);

        let scope_manager = analyze(&ast, code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(6)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes.len()).is_greater_than_or_equal_to(1);

        let scope = scopes.last().unwrap();

        let variables = scope.variables().collect_vec();
        assert_that!(&variables.len()).is_greater_than_or_equal_to(1);
        let references = scope.references().collect_vec();
        assert_that!(&references.len()).is_greater_than_or_equal_to(1);

        references.into_iter().for_each(|reference| {
            assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
            assert_that!(&reference.is_write()).is_true();
            assert_that!(&reference.init()).is_some().is_true();
        });
    });

    let false_codes = [
        "let a; a = 0;",
        "let a; [a] = [];",
        "let a; [a = 1] = [];",
        "let a; ({a} = {});",
        "let a; ({b: a} = {});",
        "let a; ({b: a = 0} = {});",
        "let a; for (a in []);",
        "let a; for ([a] in []);",
        "let a; for ([a = 0] in []);",
        "let a; for ({a} in []);",
        "let a; for ({a = 0} in []);"
    ];

    false_codes.into_iter().for_each(|code| {
        let ast = parse(code);

        let scope_manager = analyze(&ast, code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(6)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes.len()).is_greater_than_or_equal_to(1);

        let scope = scopes.last().unwrap();

        let variables = scope.variables().collect_vec();
        assert_that!(&variables.len()).is_greater_than_or_equal_to(1);
        let references = scope.references().collect_vec();
        assert_that!(&references.len()).is_greater_than_or_equal_to(1);

        references.into_iter().for_each(|reference| {
            assert_that(&&*reference.identifier().text(&scope_manager)).is_equal_to("a");
            assert_that!(&reference.is_write()).is_true();
            assert_that!(&reference.init()).is_some().is_false();
        });
    });

    let false_codes = [
        "let a; let b = a;",
        "let a; let [b] = a;",
        "let a; let [b = a] = [];",
        "let a; for (var b in a);",
        "let a; for (var [b = a] in []);",
        "let a; for (let b in a);",
        "let a; for (let [b = a] in []);",
        "let a,b; b = a;",
        "let a,b; [b] = a;",
        "let a,b; [b = a] = [];",
        "let a,b; for (b in a);",
        "let a,b; for ([b = a] in []);",
        "let a; a.foo = 0;",
        "let a,b; b = a.foo;"
    ];

    false_codes.into_iter().for_each(|code| {
        let ast = parse(code);

        let scope_manager = analyze(&ast, code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(6)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes.len()).is_greater_than_or_equal_to(1);

        let scope = &scopes[0];

        let variables = scope.variables().collect_vec();
        assert_that!(&variables.len()).is_greater_than_or_equal_to(1);
        assert_that!(&variables[0].name()).is_equal_to("a");

        let references = variables[0].references().collect_vec();

        assert_that!(&references.len()).is_greater_than_or_equal_to(1);

        references.into_iter().for_each(|reference| {
            assert_that!(&reference.is_read()).is_true();
            assert_that!(&reference.init()).is_none();
        });
    });
}
