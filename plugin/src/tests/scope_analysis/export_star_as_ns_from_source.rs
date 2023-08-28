#![cfg(test)]

use itertools::Itertools;
use speculoos::prelude::*;
use squalid::VecExt;

use crate::{
    scope::{analyze, ScopeManagerOptionsBuilder, SourceType},
    tests::helpers::{parse, tracing_subscribe},
};

#[test]
fn test_should_not_have_any_references_or_variables() {
    tracing_subscribe();

    let code = "export * as ns from 'source'";
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(11)
            .source_type(SourceType::Module)
            .build()
            .unwrap(),
    );

    let scopes =
        vec![scope_manager.global_scope()].and_extend(scope_manager.global_scope().child_scopes());

    for scope in scopes {
        assert_that!(&scope.references().collect_vec()).is_empty();
        assert_that!(&scope.through().collect_vec()).is_empty();

        assert_that!(&scope.variables().collect_vec()).is_empty();
    }
}
