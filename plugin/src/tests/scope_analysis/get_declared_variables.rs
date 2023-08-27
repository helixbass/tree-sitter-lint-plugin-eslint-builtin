#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::tree_sitter::{Node, Tree};

use crate::{
    kind::{Kind, LexicalDeclaration, VariableDeclaration},
    scope::{analyze, ScopeManager, ScopeManagerOptionsBuilder, SourceType},
    tests::helpers::{parse, tracing_subscribe},
    visit::{walk_tree, TreeEnterLeaveVisitor},
};

struct VerifyEnterLeaveVisitor<'a, 'b> {
    expected_names_list: &'b mut Vec<Vec<&'static str>>,
    scope_manager: ScopeManager<'a>,
    types: Vec<Kind>,
}

impl<'a, 'b> TreeEnterLeaveVisitor<'a> for VerifyEnterLeaveVisitor<'a, 'b> {
    fn enter_node(&mut self, node: Node<'a>) {
        if self.types.contains(&node.kind()) {
            let expected = self.expected_names_list.remove(0);
            let actual = self.scope_manager.get_declared_variables(node);

            if expected.is_empty() {
                assert_that!(&actual).is_none();
            } else {
                // println!("actual: {actual:#?}");
                assert_that!(&actual).is_some().has_length(expected.len());
                let actual = actual.unwrap();
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
    );
}
