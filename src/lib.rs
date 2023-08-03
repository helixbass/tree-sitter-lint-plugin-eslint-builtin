#![allow(non_upper_case_globals, clippy::into_iter_on_ref)]

use tree_sitter_lint::Plugin;

mod ast_helpers;
mod kind;
mod macros;
mod rules;
mod scope;
mod string_utils;
mod text;
mod utils;
mod visit;

use rules::{
    for_direction_rule, max_nested_callbacks_rule, max_params_rule, no_array_constructor_rule,
    no_async_promise_executor_rule, no_await_in_loop_rule, no_compare_neg_zero_rule,
    no_cond_assign_rule, no_debugger_rule, no_dupe_class_members_rule, no_dupe_else_if_rule,
    no_dupe_keys_rule, no_duplicate_case_rule, no_eq_null_rule, no_extra_bind_rule,
    no_extra_label_rule, no_labels_rule, no_lonely_if_rule, no_multi_assign_rule,
    no_negated_condition_rule, no_unneeded_ternary_rule,
};

pub fn instantiate() -> Plugin {
    Plugin {
        name: "eslint-builtin".to_owned(),
        rules: vec![
            for_direction_rule(),
            no_async_promise_executor_rule(),
            no_await_in_loop_rule(),
            no_compare_neg_zero_rule(),
            no_cond_assign_rule(),
            no_debugger_rule(),
            no_dupe_class_members_rule(),
            max_params_rule(),
            max_nested_callbacks_rule(),
            no_dupe_else_if_rule(),
            no_dupe_keys_rule(),
            no_duplicate_case_rule(),
            no_unneeded_ternary_rule(),
            no_array_constructor_rule(),
            no_eq_null_rule(),
            no_extra_bind_rule(),
            no_extra_label_rule(),
            no_labels_rule(),
            no_lonely_if_rule(),
            no_multi_assign_rule(),
            no_negated_condition_rule(),
        ],
    }
}
