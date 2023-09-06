use std::sync::Arc;

use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter_grep::SupportedLanguage, violation, NodeExt, Rule};

use crate::kind::{ContinueStatement, EmptyStatement, IfStatement, StatementBlock};

pub fn guard_for_in_rule() -> Arc<dyn Rule> {
    rule! {
        name => "guard-for-in",
        languages => [Javascript],
        messages => [
            wrap => "The body of a for-in should be wrapped in an if statement to filter unwanted properties from the prototype.",
        ],
        listeners => [
            r#"
              (for_in_statement) @c
            "# => |node, context| {
                let body = node.field("body");

                match body.kind() {
                    EmptyStatement | IfStatement => return,
                    StatementBlock => {
                        match body.maybe_first_non_comment_named_child(SupportedLanguage::Javascript) {
                            None => return,
                            Some(first_statement) if first_statement.kind() == IfStatement => {
                                if body.non_comment_named_children(SupportedLanguage::Javascript).nth(1).is_none() {
                                    return;
                                }

                                let consequence = first_statement.field("consequence");
                                match consequence.kind() {
                                    ContinueStatement => return,
                                    StatementBlock => {
                                        let mut statements = consequence.non_comment_named_children(SupportedLanguage::Javascript);
                                        if statements.next().matches(|first_statement| first_statement.kind() == ContinueStatement) &&
                                            statements.next().is_none()
                                        {
                                            return;
                                        }
                                    }
                                    _ => ()
                                }
                            }
                            _ => ()
                        }
                    }
                    _ => ()
                }

                context.report(violation! {
                    node => node,
                    message_id => "wrap",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::ForInStatement;

    #[test]
    fn test_guard_for_in_rule() {
        let error = RuleTestExpectedErrorBuilder::default()
            .message_id("wrap")
            .type_(ForInStatement)
            .build()
            .unwrap();

        RuleTester::run(
            guard_for_in_rule(),
            rule_tests! {
                valid => [
                    "for (var x in o);",
                    "for (var x in o) {}",
                    "for (var x in o) if (x) f();",
                    "for (var x in o) { if (x) { f(); } }",
                    "for (var x in o) { if (x) continue; f(); }",
                    "for (var x in o) { if (x) { continue; } f(); }"
                ],
                invalid => [
                    { code => "for (var x in o) { if (x) { f(); continue; } g(); }", errors => [error] },
                    { code => "for (var x in o) { if (x) { continue; f(); } g(); }", errors => [error] },
                    { code => "for (var x in o) { if (x) { f(); } g(); }", errors => [error] },
                    { code => "for (var x in o) { if (x) f(); g(); }", errors => [error] },
                    { code => "for (var x in o) { foo() }", errors => [error] },
                    { code => "for (var x in o) foo();", errors => [error] }
                ]
            },
        )
    }
}
