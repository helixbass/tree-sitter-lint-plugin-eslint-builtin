use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_debugger_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-debugger",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected 'debugger' statement.",
        ],
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_debugger_rule() {
        RuleTester::run(
            no_debugger_rule(),
            rule_tests! {
                valid => [
                    "var test = { debugger: 1 }; test.debugger;"
                ],
                invalid => [
                    {
                        code => "if (foo) debugger",
                        output => None,
                        errors => [{ message_id => "unexpected", type => "debugger_statement" }]
                    }
                ]
            },
        )
    }
}
