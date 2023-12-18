#![cfg(test)]

use speculoos::prelude::*;

use super::util::get_supported_ecma_versions;
use crate::{
    scope::{analyze, Scope, ScopeManagerOptionsBuilder},
    tests::helpers::{parse, tracing_subscribe},
};

fn assert_is_strict_recursively(scope: Scope, expected: bool) {
    assert_that!(&scope.is_strict()).is_equal_to(expected);

    scope.child_scopes().for_each(|child_scope| {
        assert_is_strict_recursively(child_scope, expected);
    });
}

#[test]
fn test_should_be_ignored_when_ecma_version_3() {
    tracing_subscribe();

    let code = r#"
        "use strict";
        function a() {
            "use strict";
            function b() {
                foo();
            }
        }
    "#;
    let ast = parse(code);

    let scope_manager = analyze(
        &ast,
        code,
        ScopeManagerOptionsBuilder::default()
            .ecma_version(3)
            .build()
            .unwrap(),
    );

    assert_is_strict_recursively(scope_manager.global_scope(), false);
}

#[test]
fn test_at_the_top_level_should_make_all_scopes_strict_when_ecma_version_5() {
    tracing_subscribe();

    get_supported_ecma_versions(Some(5)).for_each(|ecma_version| {
        let code = r#"
            "use strict";
            if (a) {
                foo();
            }
            function b() {
                if (c) {
                    foo();
                }
                function d() {
                    if (e) {
                        foo();
                    }
                }
            }
        "#;
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .build()
                .unwrap(),
        );

        assert_is_strict_recursively(scope_manager.global_scope(), true);
    });
}

#[test]
fn test_at_the_function_level_should_make_functions_scope_and_all_descendants_strict_when_ecma_version_5(
) {
    tracing_subscribe();

    get_supported_ecma_versions(Some(5)).for_each(|ecma_version| {
        let code = r#"
            function a() {
                "use strict";
                if (b) {
                    foo();
                }
                function c() {
                    if (d) {
                        foo();
                    }
                }
            }
            function e() {
                if (f) {
                    foo();
                }
            }
        "#;
        let ast = parse(code);

        let scope_manager = analyze(
            &ast,
            code,
            ScopeManagerOptionsBuilder::default()
                .ecma_version(ecma_version)
                .build()
                .unwrap(),
        );

        let global_scope = scope_manager.global_scope();
        assert_that!(&global_scope.is_strict()).is_false();
        let mut child_scopes = global_scope.child_scopes();
        assert_is_strict_recursively(child_scopes.next().unwrap(), true);
        assert_is_strict_recursively(child_scopes.next().unwrap(), false);
    });
}
