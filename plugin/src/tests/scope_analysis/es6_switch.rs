#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use tree_sitter_lint::NodeExt;

use super::util::get_supported_ecma_versions;
use crate::{
    kind::{Program, SwitchStatement},
    scope::{analyze, ScopeManagerOptionsBuilder, ScopeType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_materialize_scope() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(6)).for_each(|ecma_version| {
        let code = "
            switch (ok) {
                case hello:
                    let i = 20;
                    i;
                    break;

                default:
                    let test = 30;
                    test;
            }
        ";
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .build()
                .unwrap(),
        );

        let scopes = scope_manager.scopes().collect_vec();
        assert_that!(&scopes).has_length(2);

        let scope = &scopes[0];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
        assert_that!(&scope.block().kind()).is_equal_to(Program);
        assert_that!(&scope.is_strict()).is_false();
        assert_that!(&scope.variables().collect_vec()).is_empty();
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(1);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("ok");

        let scope = &scopes[1];
        assert_that!(&scope.type_()).is_equal_to(ScopeType::Switch);
        assert_that!(&scope.block().kind()).is_equal_to(SwitchStatement);
        assert_that!(&scope.is_strict()).is_false();
        let variables = scope.variables().collect_vec();
        assert_that!(&variables).has_length(2);
        assert_that!(&variables[0].name()).is_equal_to("i");
        assert_that!(&variables[1].name()).is_equal_to("test");
        let references = scope.references().collect_vec();
        assert_that!(&references).has_length(5);
        assert_that(&&*references[0].identifier().text(&scope_manager)).is_equal_to("hello");
        assert_that(&&*references[1].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that(&&*references[2].identifier().text(&scope_manager)).is_equal_to("i");
        assert_that(&&*references[3].identifier().text(&scope_manager)).is_equal_to("test");
        assert_that(&&*references[4].identifier().text(&scope_manager)).is_equal_to("test");
    });
}
