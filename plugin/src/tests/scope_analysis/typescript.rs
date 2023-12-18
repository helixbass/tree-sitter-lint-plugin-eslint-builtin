#![cfg(test)]

// use itertools::Itertools;
// use speculoos::prelude::*;

// use crate::{
//     scope::{analyze, ScopeType},
//     tests::helpers::{parse_typescript, tracing_subscribe},
// };

// TODO: enable this once eg Typescript visiting is supported
// #[test]
// fn test_should_create_a_function_scope() {
//     tracing_subscribe();

//     let code = r#"
//         function foo(bar: number): number;
//         function foo(bar: string): string;
//         function foo(bar: string | number): string | number {
//             return bar;
//         }
//     "#;
//     let ast = parse_typescript(code);

//     let scope_manager = analyze(&ast, code, Default::default());

//     let scopes = scope_manager.scopes().collect_vec();

//     assert_that!(&scopes).has_length(2);

//     let scope = &scopes[0];

//     assert_that!(&scope.type_()).is_equal_to(ScopeType::Global);
//     assert_that!(&scope.variables().collect_vec()).has_length(1);
//     assert_that!(&scope.references().collect_vec()).has_length(4);
//     assert_that!(&scope.is_arguments_materialized()).is_true();

//     let scope = &scopes[1];

//     assert_that!(&scope.type_()).is_equal_to(ScopeType::Function);
//     let variables = scope.variables().collect_vec();
//     assert_that!(&variables).has_length(2);
//     assert_that!(&variables[0].name()).is_equal_to("arguments");
//     assert_that!(&scope.is_arguments_materialized()).is_false();
//     assert_that!(&scope.references().collect_vec()).has_length(1);
// }
