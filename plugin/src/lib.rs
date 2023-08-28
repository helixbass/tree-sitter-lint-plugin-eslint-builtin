#![allow(non_upper_case_globals, clippy::into_iter_on_ref)]

use tree_sitter_lint::{Plugin, FromFileRunContextInstanceProviderFactory, instance_provider_factory};

mod ast_helpers;
mod code_path_analysis;
mod directives;
mod kind;
mod macros;
mod rules;
pub mod scope;
mod string_utils;
#[cfg(test)]
mod tests;
mod utils;
mod visit;

pub use code_path_analysis::{
    CodePath, CodePathAnalyzer, CodePathOrigin, CodePathSegment, EnterOrExit,
};
use rules::{
    array_bracket_newline_rule, array_callback_return_rule, complexity_rule,
    consistent_return_rule, constructor_super_rule, default_case_last_rule, default_case_rule,
    for_direction_rule, getter_return_rule, max_nested_callbacks_rule, max_params_rule,
    no_array_constructor_rule, no_async_promise_executor_rule, no_await_in_loop_rule,
    no_compare_neg_zero_rule, no_cond_assign_rule, no_const_assign_rule,
    no_constructor_return_rule, no_debugger_rule, no_dupe_class_members_rule, no_dupe_else_if_rule,
    no_dupe_keys_rule, no_duplicate_case_rule, no_empty_pattern_rule, no_eq_null_rule,
    no_extra_bind_rule, no_extra_label_rule, no_fallthrough_rule, no_labels_rule,
    no_lonely_if_rule, no_mixed_operators_rule, no_multi_assign_rule, no_multi_str_rule,
    no_negated_condition_rule, no_nested_ternary_rule, no_new_rule, no_new_wrappers_rule,
    no_octal_escape_rule, no_octal_rule, no_plusplus_rule, no_proto_rule,
    no_restricted_properties_rule, no_return_assign_rule, no_script_url_rule, no_self_assign_rule,
    no_sequences_rule, no_ternary_rule, no_this_before_super_rule, no_throw_literal_rule,
    no_unneeded_ternary_rule, no_unreachable_loop_rule, no_unreachable_rule,
    no_unsafe_finally_rule, no_unsafe_negation_rule, no_unsafe_optional_chaining_rule,
    no_unused_labels_rule, no_useless_call_rule, no_useless_catch_rule, no_useless_return_rule,
    require_yield_rule, sort_keys_rule, space_unary_ops_rule, yield_star_spacing_rule,
};
use scope::ScopeManager;
pub use visit::Visit;

pub type ProvidedTypes<'a> = (CodePathAnalyzer<'a>, ScopeManager<'a>);

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
            no_multi_str_rule(),
            no_mixed_operators_rule(),
            no_empty_pattern_rule(),
            no_constructor_return_rule(),
            complexity_rule(),
            consistent_return_rule(),
            getter_return_rule(),
            no_unreachable_rule(),
            no_fallthrough_rule(),
            no_useless_return_rule(),
            no_self_assign_rule(),
            constructor_super_rule(),
            no_unreachable_loop_rule(),
            array_callback_return_rule(),
            no_this_before_super_rule(),
            no_unsafe_finally_rule(),
            no_unsafe_negation_rule(),
            no_unsafe_optional_chaining_rule(),
            yield_star_spacing_rule(),
            array_bracket_newline_rule(),
            space_unary_ops_rule(),
            no_const_assign_rule(),
        ],
    }
}

pub fn get_instance_provider_factory() -> Box<dyn FromFileRunContextInstanceProviderFactory> {
    Box::new(instance_provider_factory!(ProvidedTypes))
}
