#![allow(non_upper_case_globals, clippy::into_iter_on_ref)]

use tree_sitter_lint::Plugin;

mod ast_helpers;
mod code_path_analysis;
mod kind;
mod macros;
mod rules;
mod scope;
mod string_utils;
mod text;
mod utils;
mod visit;

use rules::{
    default_case_last_rule, default_case_rule, for_direction_rule, max_nested_callbacks_rule,
    max_params_rule, no_array_constructor_rule, no_async_promise_executor_rule,
    no_await_in_loop_rule, no_compare_neg_zero_rule, no_cond_assign_rule, no_debugger_rule,
    no_dupe_class_members_rule, no_dupe_else_if_rule, no_dupe_keys_rule, no_duplicate_case_rule,
    no_eq_null_rule, no_extra_bind_rule, no_extra_label_rule, no_labels_rule, no_lonely_if_rule,
    no_multi_assign_rule, no_negated_condition_rule, no_nested_ternary_rule, no_new_rule,
    no_new_wrappers_rule, no_octal_escape_rule, no_octal_rule, no_plusplus_rule, no_proto_rule,
    no_restricted_properties_rule, no_return_assign_rule, no_script_url_rule, no_sequences_rule,
    no_ternary_rule, no_throw_literal_rule, no_unneeded_ternary_rule, no_unused_labels_rule,
    no_useless_call_rule, no_useless_catch_rule, require_yield_rule, sort_keys_rule,
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
            no_nested_ternary_rule(),
            no_new_rule(),
            no_new_wrappers_rule(),
            no_octal_rule(),
            no_octal_escape_rule(),
            no_plusplus_rule(),
            no_proto_rule(),
            no_restricted_properties_rule(),
            no_return_assign_rule(),
            no_script_url_rule(),
            no_sequences_rule(),
            no_ternary_rule(),
            no_throw_literal_rule(),
            no_unused_labels_rule(),
            no_useless_call_rule(),
            no_useless_catch_rule(),
            sort_keys_rule(),
            default_case_rule(),
            default_case_last_rule(),
            require_yield_rule(),
        ],
    }
}
