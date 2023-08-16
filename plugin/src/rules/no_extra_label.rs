use std::sync::Arc;

use squalid::OptionExt;
use tree_sitter_lint::{
    range_between_ends, rule, tree_sitter::Node, tree_sitter_grep::return_if_none, violation,
    NodeExt, QueryMatchContext, Rule,
};

use crate::{ast_helpers::NodeExtJs, kind::LabeledStatement, utils::ast_utils};

#[derive(Debug)]
struct ScopeInfo<'a> {
    label: Option<Node<'a>>,
    breakable: bool,
}

fn report_if_unnecessary<'a>(
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
    scope_infos: &[ScopeInfo],
) {
    let label_node = return_if_none!(node.child_by_field_name("label"));
    let label_node_name = label_node.text(context);

    for info in scope_infos.into_iter().rev() {
        if info.breakable
            || info
                .label
                .matches(|label| label.text(context) == label_node_name)
        {
            if info.breakable
                && info
                    .label
                    .matches(|label| label.text(context) == label_node_name)
            {
                context.report(violation! {
                    node => label_node,
                    message_id => "unexpected",
                    data => {
                        name => label_node_name,
                    },
                    fix => |fixer| {
                        let break_or_continue_token = context.get_first_token(node, Option::<fn(Node) -> bool>::None);

                        if context.comments_exist_between(break_or_continue_token, label_node) {
                            return;
                        }

                        fixer.remove_range(
                            range_between_ends(
                                break_or_continue_token.range(),
                                label_node.range(),
                            )
                        );
                    }
                });
            }
            return;
        }
    }
}

pub fn no_extra_label_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-extra-label",
        languages => [Javascript],
        messages => [
            unexpected => "This label '{{name}}' is unnecessary.",
        ],
        fixable => true,
        state => {
            [per-file-run]
            scope_infos: Vec<ScopeInfo<'a>>,
        },
        listeners => [
            r#"
              (while_statement) @c
              (do_statement) @c
              (for_statement) @c
              (for_in_statement) @c
              (switch_statement) @c
            "# => |node, context| {
                let parent = node.next_non_parentheses_ancestor();
                self.scope_infos.push(ScopeInfo {
                    label: if parent.kind() == LabeledStatement {
                        Some(parent.field("label"))
                    } else {
                        None
                    },
                    breakable: true,
                });
            },
            r#"
              while_statement:exit,
              do_statement:exit,
              for_statement:exit,
              for_in_statement:exit,
              switch_statement:exit
            "# => |node, context| {
                self.scope_infos.pop().unwrap();
            },
            LabeledStatement => |node, context| {
                if !ast_utils::is_breakable_statement(node.field("body")) {
                    self.scope_infos.push(ScopeInfo {
                        label: Some(node.field("label")),
                        breakable: false,
                    });
                }
            },
            "labeled_statement:exit" => |node, context| {
                if !ast_utils::is_breakable_statement(node.field("body")) {
                    self.scope_infos.pop().unwrap();
                }
            },
            r#"
              (break_statement) @c
              (continue_statement) @c
            "# => |node, context| {
                report_if_unnecessary(node, context, &self.scope_infos);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_extra_label_rule() {
        RuleTester::run(
            no_extra_label_rule(),
            rule_tests! {
                valid => [
                    "A: break A;",
                    "A: { if (a) break A; }",
                    "A: { while (b) { break A; } }",
                    "A: { switch (b) { case 0: break A; } }",
                    "A: while (a) { while (b) { break; } break; }",
                    "A: while (a) { while (b) { break A; } }",
                    "A: while (a) { while (b) { continue A; } }",
                    "A: while (a) { switch (b) { case 0: break A; } }",
                    "A: while (a) { switch (b) { case 0: continue A; } }",
                    "A: switch (a) { case 0: while (b) { break A; } }",
                    "A: switch (a) { case 0: switch (b) { case 0: break A; } }",
                    "A: for (;;) { while (b) { break A; } }",
                    "A: do { switch (b) { case 0: break A; break; } } while (a);",
                    "A: for (a in obj) { while (b) { break A; } }",
                    { code => "A: for (a of ary) { switch (b) { case 0: break A; } }", /*parserOptions: { ecmaVersion: 6 }*/ }
                ],
                invalid => [
                    {
                        code => "A: while (a) break A;",
                        output => "A: while (a) break;",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while (a) { B: { continue A; } }",
                        output => "A: while (a) { B: { continue; } }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "X: while (x) { A: while (a) { B: { break A; break B; continue X; } } }",
                        output => "X: while (x) { A: while (a) { B: { break; break B; continue X; } } }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: do { break A; } while (a);",
                        output => "A: do { break; } while (a);",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: for (;;) { break A; }",
                        output => "A: for (;;) { break; }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: for (a in obj) { break A; }",
                        output => "A: for (a in obj) { break; }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: for (a of ary) { break A; }",
                        output => "A: for (a of ary) { break; }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: switch (a) { case 0: break A; }",
                        output => "A: switch (a) { case 0: break; }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "X: while (x) { A: switch (a) { case 0: break A; } }",
                        output => "X: while (x) { A: switch (a) { case 0: break; } }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "X: switch (a) { case 0: A: while (b) break A; }",
                        output => "X: switch (a) { case 0: A: while (b) break; }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => r#"
                            A: while (true) {
                                break A;
                                while (true) {
                                    break A;
                                }
                            }
                        "#,
                        output => r#"
                            A: while (true) {
                                break;
                                while (true) {
                                    break A;
                                }
                            }
                        "#,
                        errors => [{ message_id => "unexpected", data => { name => "A" }, type => "statement_identifier", line => 3 }]
                    },

                    // Should not autofix if it would remove comments
                    {
                        code => "A: while(true) { /*comment*/break A; }",
                        output => "A: while(true) { /*comment*/break; }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while(true) { break/**/ A; }",
                        output => None,
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while(true) { continue /**/ A; }",
                        output => None,
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while(true) { break /**/A; }",
                        output => None,
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while(true) { continue/**/A; }",
                        output => None,
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while(true) { continue A/*comment*/; }",
                        output => "A: while(true) { continue/*comment*/; }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while(true) { break A//comment\n }",
                        output => "A: while(true) { break//comment\n }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    },
                    {
                        code => "A: while(true) { break A/*comment*/\nfoo() }",
                        output => "A: while(true) { break/*comment*/\nfoo() }",
                        errors => [{ message_id => "unexpected", data => { name => "A" } }]
                    }
                ]
            },
        )
    }
}
