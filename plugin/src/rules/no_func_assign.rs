use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_func_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-func-assign",
        languages => [Javascript],
        messages => [
            is_a_function => "'{{name}}' is a function.",
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
    fn test_no_func_assign_rule() {
        RuleTester::run(
            no_func_assign_rule(),
            rule_tests! {
                valid => [
                    "function foo() { var foo = bar; }",
                    "function foo(foo) { foo = bar; }",
                    "function foo() { var foo; foo = bar; }",
                    { code => "var foo = () => {}; foo = bar;", environment => { ecma_version => 6 } },
                    "var foo = function() {}; foo = bar;",
                    "var foo = function() { foo = bar; };",
                    { code => "import bar from 'bar'; function foo() { var foo = bar; }", environment => { ecma_version => 6, source_type => "module" } }
                ],
                invalid => [
                    {
                        code => "function foo() {}; foo = bar;",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "function foo() { foo = bar; }",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "foo = bar; function foo() { };",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "[foo] = bar; function foo() { };",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "({x: foo = 0} = bar); function foo() { };",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "function foo() { [foo] = bar; }",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "(function() { ({x: foo = 0} = bar); function foo() { }; })();",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    },
                    {
                        code => "var a = function foo() { foo = 123; };",
                        errors => [{
                            message_id => "is_a_function",
                            data => { name => "foo" },
                            type => Identifier
                        }]
                    }
                ]
            },
        )
    }
}
