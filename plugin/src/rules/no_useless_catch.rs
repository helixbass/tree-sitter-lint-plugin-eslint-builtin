use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_useless_catch_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-useless-catch",
        languages => [Javascript],
        messages => [
            unnecessary_catch_clause => "Unnecessary catch clause.",
            unnecessary_catch => "Unnecessary try/catch wrapper.",
        ],
        listeners => [
            r#"
              (catch_clause
                parameter: (identifier) @catch_param
                body: (statement_block
                  .
                  (comment)*
                  .
                  (throw_statement
                    (identifier) @throw_arg (#eq? @throw_arg @catch_param)
                  )
                )
              ) @catch_clause
            "# => {
                capture_name => "catch_clause",
                callback => |node, context| {
                    if node
                        .parent()
                        .unwrap()
                        .child_by_field_name("finalizer")
                        .is_some()
                    {
                        context.report(violation! {
                            node => node,
                            message_id => "unnecessary_catch_clause",
                        });
                    } else {
                        context.report(violation! {
                            node => node.parent().unwrap(),
                            message_id => "unnecessary_catch",
                        });
                    }
                },
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::{CatchClause, TryStatement};

    #[test]
    fn test_no_useless_catch_rule() {
        RuleTester::run(
            no_useless_catch_rule(),
            rule_tests! {
            valid => [
                r#"
                    try {
                        foo();
                    } catch (err) {
                        console.error(err);
                    }
                "#,
                r#"
                    try {
                        foo();
                    } catch (err) {
                        console.error(err);
                    } finally {
                        bar();
                    }
                "#,
                r#"
                    try {
                        foo();
                    } catch (err) {
                        doSomethingBeforeRethrow();
                        throw err;
                    }
                "#,
                r#"
                    try {
                        foo();
                    } catch (err) {
                        throw err.msg;
                    }
                "#,
                r#"
                    try {
                        foo();
                    } catch (err) {
                        throw new Error("whoops!");
                    }
                "#,
                r#"
                    try {
                        foo();
                    } catch (err) {
                        throw bar;
                    }
                "#,
                r#"
                    try {
                        foo();
                    } catch (err) { }
                "#,
                {
                    code => r#"
                        try {
                            foo();
                        } catch ({ err }) {
                            throw err;
                        }
                    "#,
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        try {
                            foo();
                        } catch ([ err ]) {
                            throw err;
                        }
                    "#,
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        async () => {
                            try {
                                await doSomething();
                            } catch (e) {
                                doSomethingAfterCatch();
                                throw e;
                            }
                        }
                    "#,
                    // parserOptions: { ecmaVersion: 8 }
                },
                {
                    code => r#"
                        try {
                            throw new Error('foo');
                        } catch {
                            throw new Error('foo');
                        }
                    "#,
                    // parserOptions: { ecmaVersion: 2019 }
                }
            ],
            invalid => [
                {
                    code => r#"
                        try {
                            foo();
                        } catch (err) {
                            throw err;
                        }
                    "#,
                    errors => [{
                        message_id => "unnecessary_catch",
                        type => TryStatement
                    }]
                },
                {
                    code => r#"
                        try {
                            foo();
                        } catch (err) {
                            throw err;
                        } finally {
                            foo();
                        }
                    "#,
                    errors => [{
                        message_id => "unnecessary_catch_clause",
                        type => CatchClause
                    }]
                },
                {
                    code => r#"
                        try {
                            foo();
                        } catch (err) {
                            /* some comment */
                            throw err;
                        }
                    "#,
                    errors => [{
                        message_id => "unnecessary_catch",
                        type => TryStatement
                    }]
                },
                {
                    code => r#"
                        try {
                            foo();
                        } catch (err) {
                            /* some comment */
                            throw err;
                        } finally {
                            foo();
                        }
                    "#,
                    errors => [{
                        message_id => "unnecessary_catch_clause",
                        type => CatchClause
                    }]
                },
                {
                    code => r#"
                        async () => {
                            try {
                                await doSomething();
                            } catch (e) {
                                throw e;
                            }
                        }
                    "#,
                    // parserOptions: { ecmaVersion: 8 },
                    errors => [{
                        message_id => "unnecessary_catch",
                        type => TryStatement
                    }]
                }
            ]
            },
        )
    }
}
