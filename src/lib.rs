#![allow(non_upper_case_globals, clippy::into_iter_on_ref)]

use tree_sitter_lint::Plugin;

mod ast_helpers;
mod kind;
mod rules;
mod text;
mod utils;

use rules::{
    for_direction_rule, no_async_promise_executor_rule, no_await_in_loop_rule,
    no_compare_neg_zero_rule, no_cond_assign_rule, no_debugger_rule, no_dupe_class_members_rule,
    no_dupe_else_if_rule,
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
            no_dupe_else_if_rule(),
        ],
    }
}
