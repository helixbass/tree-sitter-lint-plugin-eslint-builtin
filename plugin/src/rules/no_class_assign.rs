use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_class_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-class-assign",
        languages => [Javascript],
        messages => [
            class => "'{{name}}' is a class.",
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
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::Identifier;

    #[test]
    fn test_no_class_assign_rule() {
        RuleTester::run(
            no_class_assign_rule(),
            rule_tests! {
                valid => [
                    "class A { } foo(A);",
                    "let A = class A { }; foo(A);",
                    "class A { b(A) { A = 0; } }",
                    "class A { b() { let A; A = 0; } }",
                    "let A = class { b() { A = 0; } }",

                    // ignores non class.
                    "var x = 0; x = 1;",
                    "let x = 0; x = 1;",
                    "const x = 0; x = 1;",
                    "function x() {} x = 1;",
                    "function foo(x) { x = 1; }",
                    "try {} catch (x) { x = 1; }"
                ],
                invalid => [
                    {
                        code => "class A { } A = 0;",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "class A { } ({A} = 0);",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "class A { } ({b: A = 0} = {});",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "A = 0; class A { }",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "class A { b() { A = 0; } }",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "let A = class A { b() { A = 0; } }",
                        errors => [{ message_id => "class", data => { name => "A" }, type => Identifier }]
                    },
                    {
                        code => "class A { } A = 0; A = 1;",
                        errors => [
                            { message_id => "class", data => { name => "A" }, type => Identifier, line => 1, column => 13 },
                            { message_id => "class", data => { name => "A" }, type => Identifier, line => 1, column => 20 }
                        ]
                    }
                ]
            },
        )
    }
}
