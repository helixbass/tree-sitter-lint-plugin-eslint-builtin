#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::tree_sitter::{Node, Tree};

use crate::{
    ast_helpers::is_default_import,
    kind::{
        ArrowFunction, CatchClause, Class, ClassDeclaration, ForInStatement, Function,
        FunctionDeclaration, Identifier, ImportSpecifier, ImportStatement, Kind,
        LexicalDeclaration, ObjectPattern, VariableDeclaration, VariableDeclarator, NamespaceImport,
    },
    scope::{analyze, ScopeManager, ScopeManagerOptionsBuilder, SourceType},
    tests::helpers::{parse, tracing_subscribe},
    visit::{walk_tree, TreeEnterLeaveVisitor},
};

struct VerifyEnterLeaveVisitor<'a, 'b> {
    expected_names_list: &'b mut Vec<Vec<&'static str>>,
    scope_manager: ScopeManager<'a>,
    types: Vec<Kind>,
    matcher: Option<&'b dyn Fn(Node) -> bool>,
}

impl<'a, 'b> TreeEnterLeaveVisitor<'a> for VerifyEnterLeaveVisitor<'a, 'b> {
    fn enter_node(&mut self, node: Node<'a>) {
        if self.types.contains(&node.kind())
            && match self.matcher {
                Some(matcher) => matcher(node),
                None => true,
            }
        {
            let expected = self.expected_names_list.remove(0);
            let actual = self.scope_manager.get_declared_variables(node).collect_vec();

            if expected.is_empty() {
                assert_that!(&actual).is_empty();
            } else {
                // println!("actual: {actual:#?}, node: {node:#?}, expected: {expected:#?}");
                assert_that!(&actual).has_length(expected.len());
                for (i, actual_item) in actual.into_iter().enumerate() {
                    assert_that!(&actual_item.name()).is_equal_to(expected[i]);
                }
            }
        }
    }

    fn leave_node(&mut self, _node: Node<'a>) {}
}

fn verify(
    ast: &Tree,
    code: &str,
    types: impl IntoIterator<Item = Kind>,
    expected_names_list: impl IntoIterator<Item = Vec<&'static str>>,
    matcher: Option<&dyn Fn(Node) -> bool>,
) {
    let types = types.into_iter().collect_vec();
    let mut expected_names_list = expected_names_list.into_iter().collect_vec();

    let scope_manager = analyze(
        ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(6)
            .source_type(SourceType::Module)
            .build()
            .unwrap(),
    );

    let mut visitor = VerifyEnterLeaveVisitor {
        expected_names_list: &mut expected_names_list,
        scope_manager,
        types,
        matcher,
    };

    walk_tree(ast, &mut visitor);

    assert_that!(&expected_names_list).is_empty();
}

#[test]
fn test_variable_declaration() {
    tracing_subscribe();

    let code = "
        var {a, x: [b], y: {c = 0}} = foo;
        let {d, x: [e], y: {f = 0}} = foo;
        const {g, x: [h], y: {i = 0}} = foo, {j, k = function() { let l; }} = bar;
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [VariableDeclaration, LexicalDeclaration],
        [
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i", "j", "k"],
            vec!["l"],
        ],
        None,
    );
}

#[test]
fn test_variable_declaration_for_in_of() {
    tracing_subscribe();

    let code = "
        for (var {a, x: [b], y: {c = 0}} in foo) {
            let g;
        }
        for (let {d, x: [e], y: {f = 0}} of foo) {
            let h;
        }
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [VariableDeclaration, LexicalDeclaration, ObjectPattern],
        [
            vec!["a", "b", "c"],
            vec!["g"],
            vec!["d", "e", "f"],
            vec!["h"],
        ],
        Some(&|node| {
            !(node.kind() == ObjectPattern && node.parent().unwrap().kind() != ForInStatement)
        }),
    );
}

#[test]
fn test_variable_declarator() {
    tracing_subscribe();

    let code = "
        var {a, x: [b], y: {c = 0}} = foo;
        let {d, x: [e], y: {f = 0}} = foo;
        const {g, x: [h], y: {i = 0}} = foo, {j, k = function() { let l; }} = bar;
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [VariableDeclarator],
        [
            vec!["a", "b", "c"],
            vec!["d", "e", "f"],
            vec!["g", "h", "i"],
            vec!["j", "k"],
            vec!["l"],
        ],
        None,
    );
}

#[test]
fn test_function_declaration() {
    tracing_subscribe();

    let code = "
        function foo({a, x: [b], y: {c = 0}}, [d, e]) {
            let z;
        }
        function bar({f, x: [g], y: {h = 0}}, [i, j = function(q) { let w; }]) {
            let z;
        }
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [FunctionDeclaration],
        [
            vec!["foo", "a", "b", "c", "d", "e"],
            vec!["bar", "f", "g", "h", "i", "j"],
        ],
        None,
    );
}

#[test]
fn test_function_expression() {
    tracing_subscribe();

    let code = "
        (function foo({a, x: [b], y: {c = 0}}, [d, e]) {
            let z;
        });
        (function bar({f, x: [g], y: {h = 0}}, [i, j = function(q) { let w; }]) {
            let z;
        });
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [Function],
        [
            vec!["foo", "a", "b", "c", "d", "e"],
            vec!["bar", "f", "g", "h", "i", "j"],
            vec!["q"],
        ],
        Some(&|node| {
            // https://github.com/tree-sitter/tree-sitter-javascript/issues/268
            node.is_named()
        }),
    );
}

#[test]
fn test_arrow_function() {
    tracing_subscribe();

    let code = "
        (({a, x: [b], y: {c = 0}}, [d, e]) => {
            let z;
        });
        (({f, x: [g], y: {h = 0}}, [i, j]) => {
            let z;
        });
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [ArrowFunction],
        [vec!["a", "b", "c", "d", "e"], vec!["f", "g", "h", "i", "j"]],
        None,
    );
}

#[test]
fn test_class_declaration() {
    tracing_subscribe();

    let code = "
        class A { foo(x) { let y; } }
        class B { foo(x) { let y; } }
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [ClassDeclaration],
        [vec!["A", "A"], vec!["B", "B"]],
        None,
    );
}

#[test]
fn test_class_expression() {
    tracing_subscribe();

    let code = "
        (class A { foo(x) { let y; } });
        (class B { foo(x) { let y; } });
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [Class],
        [vec!["A"], vec!["B"]],
        Some(&|node| {
            // https://github.com/tree-sitter/tree-sitter-javascript/issues/268
            node.is_named()
        }),
    );
}

#[test]
fn test_catch_clause() {
    tracing_subscribe();

    let code = "
        try {} catch ({a, b}) {
            let x;
            try {} catch ({c, d}) {
                let y;
            }
        }
    ";
    let ast = parse(code);

    verify(
        &ast,
        code,
        [CatchClause],
        [vec!["a", "b"], vec!["c", "d"]],
        None,
    );
}

#[test]
fn test_import_statement() {
    tracing_subscribe();

    let code = r#"
        import "aaa";
        import * as a from "bbb";
        import b, {c, x as d} from "ccc";
    "#;
    let ast = parse(code);

    verify(
        &ast,
        code,
        [ImportStatement],
        [vec![], vec!["a"], vec!["b", "c", "d"]],
        None,
    );
}

#[test]
fn test_import_specifier() {
    tracing_subscribe();

    let code = r#"
        import "aaa";
        import * as a from "bbb";
        import b, {c, x as d} from "ccc";
    "#;
    let ast = parse(code);

    verify(&ast, code, [ImportSpecifier], [vec!["c"], vec!["d"]], None);
}

#[test]
fn test_import_default() {
    tracing_subscribe();

    let code = r#"
        import "aaa";
        import * as a from "bbb";
        import b, {c, x as d} from "ccc";
    "#;
    let ast = parse(code);

    verify(
        &ast,
        code,
        [Identifier],
        [vec!["b"]],
        Some(&is_default_import),
    );
}

#[test]
fn test_namespace_import() {
    tracing_subscribe();

    let code = r#"
        import "aaa";
        import * as a from "bbb";
        import b, {c, x as d} from "ccc";
    "#;
    let ast = parse(code);

    verify(
        &ast,
        code,
        [NamespaceImport],
        [vec!["a"]],
        None,
    );
}

#[test]
fn test_duplicate() {
    tracing_subscribe();

    let code = r#"
        var a = 0, a = 1;
    "#;
    let ast = parse(code);

    verify(
        &ast,
        code,
        [VariableDeclaration],
        [vec!["a"]],
        None,
    );
}
