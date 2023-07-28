use tree_sitter_lint::Plugin;

mod rules;

use rules::{for_direction_rule, no_async_promise_executor_rule};

pub fn instantiate() -> Plugin {
    Plugin {
        name: "eslint-builtin".to_owned(),
        rules: vec![for_direction_rule(), no_async_promise_executor_rule()],
    }
}
