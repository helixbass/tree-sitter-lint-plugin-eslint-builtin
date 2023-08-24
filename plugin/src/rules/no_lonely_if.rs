use std::sync::Arc;

use squalid::{regex, EverythingExt, OptionExt};
use tree_sitter_lint::{
    range_between_start_and_end, rule, tree_sitter::Node, violation, NodeExt, Rule,
};

use crate::kind::StatementBlock;

pub fn no_lonely_if_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-lonely-if",
        languages => [Javascript],
        messages => [
            unexpected_lonely_if => "Unexpected if as the only statement in an else block.",
        ],
        fixable => true,
        listeners => [
            r#"
              (if_statement
                alternative: (else_clause
                  (statement_block
                    ; TODO: it seems unexpected that these anchors don't "work"?
                    .
                    (
                      (comment)*
                      (if_statement) @c
                      (comment)*
                    )
                    .
                  )
                )
              )
            "# => |node, context| {
                if !node.is_only_non_comment_named_sibling(context) {
                    return;
                }
                context.report(violation! {
                    node => node,
                    message_id => "unexpected_lonely_if",
                    fix => |fixer| {
                        let parent = node.parent().unwrap();
                        let opening_else_curly = context.get_first_token(
                            parent,
                            Option::<fn(Node) -> bool>::None,
                        );
                        let closing_else_curly = context.get_last_token(
                            parent,
                            Option::<fn(Node) -> bool>::None,
                        );
                        let else_keyword = context.get_token_before(
                            opening_else_curly,
                            Option::<fn(Node) -> bool>::None,
                        );
                        let token_after_else_block = context.maybe_get_token_after(
                            closing_else_curly,
                            Option::<fn(Node) -> bool>::None,
                        );
                        let consequent = node.field("consequence");
                        let last_if_token = context.get_last_token(
                            consequent,
                            Option::<fn(Node) -> bool>::None,
                        );

                        if !context.get_text_slice(opening_else_curly.end_byte()..node.start_byte()).trim().is_empty() ||
                            node.has_trailing_comments(context) {
                            return;
                        }

                        if consequent.kind() != StatementBlock &&
                            last_if_token.text(context) != ";" &&
                            token_after_else_block.matches(|token_after_else_block| {
                                consequent.start_position().row == token_after_else_block.start_position().row ||
                                    regex!(r#"^[(\[+`-]"#).is_match(&token_after_else_block.text(context)) ||
                                    matches!(
                                        &*last_if_token.text(context),
                                        "++" | "--"
                                    )
                            }) {
                            return;
                        }

                        fixer.replace_text_range(
                            range_between_start_and_end(
                                opening_else_curly.range(),
                                closing_else_curly.range(),
                            ),
                            node.text(context).thrush(|node_text| {
                                if else_keyword.end_byte() == opening_else_curly.start_byte() {
                                    format!(" {node_text}")
                                } else {
                                    node_text.into_owned()
                                }
                            })
                        );
                    }
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    #[test]
    fn test_no_lonely_if_rule() {
        let errors = [RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected_lonely_if")
            .type_("if_statement")
            .build()
            .unwrap()];

        RuleTester::run(
            no_lonely_if_rule(),
            rule_tests! {
                // Examples of code that should not trigger the rule
                valid => [
                    "if (a) {;} else if (b) {;}",
                    "if (a) {;} else { if (b) {;} ; }"
                ],

                // Examples of code that should trigger the rule
                invalid => [
                    {
                        code => "if (a) {;} else { if (b) {;} }",
                        output => "if (a) {;} else if (b) {;}",
                        errors => errors,
                    },
                    {
                        code => r#"
                            if (a) {
                              foo();
                            } else {
                              if (b) {
                                bar();
                              }
                            }
                        "#,
                        output => r#"
                            if (a) {
                              foo();
                            } else if (b) {
                                bar();
                              }
                        "#,
                        errors => errors,
                    },
                    {
                        code => r#"
                            if (a) {
                              foo();
                            } else /* comment */ {
                              if (b) {
                                bar();
                              }
                            }
                        "#,
                        output => r#"
                            if (a) {
                              foo();
                            } else /* comment */ if (b) {
                                bar();
                              }
                        "#,
                        errors => errors,
                    },
                    {
                        code => r#"
                            if (a) {
                              foo();
                            } else {
                              /* otherwise, do the other thing */ if (b) {
                                bar();
                              }
                            }
                        "#,
                        output => None,
                        errors => errors,
                    },
                    {
                        code => r#"
                            if (a) {
                              foo();
                            } else {
                              if /* this comment is ok */ (b) {
                                bar();
                              }
                            }
                        "#,
                        output => r#"
                            if (a) {
                              foo();
                            } else if /* this comment is ok */ (b) {
                                bar();
                              }
                        "#,
                        errors => errors,
                    },
                    {
                        code => r#"
                            if (a) {
                              foo();
                            } else {
                              if (b) {
                                bar();
                              } /* this comment will prevent this test case from being autofixed. */
                            }
                        "#,
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "if (foo) {} else { if (bar) baz(); }",
                        output => "if (foo) {} else if (bar) baz();",
                        errors => errors,
                    },
                    {
                        // Not fixed; removing the braces would cause a SyntaxError.
                        code => "if (foo) {} else { if (bar) baz() } qux();",
                        output => None,
                        errors => errors,
                    },
                    {
                        // This is fixed because there is a semicolon after baz().
                        code => "if (foo) {} else { if (bar) baz(); } qux();",
                        output => "if (foo) {} else if (bar) baz(); qux();",
                        errors => errors,
                    },
                    {
                        // Not fixed; removing the braces would change the semantics due to ASI.
                        code => r#"
                            if (foo) {
                            } else {
                              if (bar) baz()
                            }
                            [1, 2, 3].forEach(foo);
                        "#,
                        output => None,
                        errors => errors,
                    },
                    {
                        // Not fixed; removing the braces would change the semantics due to ASI.
                        code => r#"
                            if (foo) {
                            } else {
                              if (bar) baz++
                            }
                            foo;
                        "#,
                        output => None,
                        errors => errors,
                    },
                    {
                        // This is fixed because there is a semicolon after baz++
                        code => r#"
                            if (foo) {
                            } else {
                              if (bar) baz++;
                            }
                            foo;
                        "#,
                        output => r#"
                            if (foo) {
                            } else if (bar) baz++;
                            foo;
                        "#,
                        errors => errors,
                    },
                    {
                        // Not fixed; bar() would be interpreted as a template literal tag
                        code => r#"
                            if (a) {
                              foo();
                            } else {
                              if (b) bar()
                            }
                            `template literal`;
                        "#,
                        output => None,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => errors,
                    },
                    {
                        code => r#"
                            if (a) {
                              foo();
                            } else {
                              if (b) {
                                bar();
                              } else if (c) {
                                baz();
                              } else {
                                qux();
                              }
                            }
                        "#,
                        output => r#"
                            if (a) {
                              foo();
                            } else if (b) {
                                bar();
                              } else if (c) {
                                baz();
                              } else {
                                qux();
                              }
                        "#,
                        errors => errors,
                    }
                ]
            },
        )
    }
}
