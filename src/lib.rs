#![allow(non_upper_case_globals)]

use tree_sitter_lint::Plugin;

mod ast_helpers;
mod kind;
mod rules;

use rules::{
    for_direction_rule, no_async_promise_executor_rule, no_await_in_loop_rule,
    no_compare_neg_zero_rule,
};

pub fn instantiate() -> Plugin {
    Plugin {
        name: "eslint-builtin".to_owned(),
        rules: vec![
            for_direction_rule(),
            no_async_promise_executor_rule(),
            no_await_in_loop_rule(),
            no_compare_neg_zero_rule(),
        ],
    }
}
