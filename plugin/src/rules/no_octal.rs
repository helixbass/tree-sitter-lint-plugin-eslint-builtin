use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_octal_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-octal",
        languages => [Javascript],
        messages => [
            no_octal => "Octal literals should not be used.",
        ],
        listeners => [
            r#"(
              (number) @number (#match? @number "^0[0-9]")
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "no_octal",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::Number;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_octal_rule() {
        RuleTester::run(
            no_octal_rule(),
            rule_tests! {
            valid => [
                "var a = 'hello world';",
                "0x1234",
                "0X5;",
                "a = 0;",
                "0.1",
                "0.5e1"
            ],
            invalid => [
                {
                    code => "var a = 01234;",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "a = 1 + 01234;",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "00",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "08",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "09.1",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "09e1",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "09.1e1",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "018",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "019.1",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "019e1",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                },
                {
                    code => "019.1e1",
                    errors => [{
                        message_id => "no_octal",
                        type => Number
                    }]
                }
            ]
            },
        )
    }
}
