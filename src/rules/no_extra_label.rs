use std::sync::Arc;

use squalid::OptionExt;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::return_if_none, violation,
    FromFileRunContextInstanceProviderFactory, NodeExt, QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::{range_between_ends, NodeExtJs},
    kind::LabeledStatement,
    utils::ast_utils,
};

#[derive(Debug)]
struct ScopeInfo<'a> {
    label: Option<Node<'a>>,
    breakable: bool,
    upper: Option<Box<ScopeInfo<'a>>>,
    node: Node<'a>,
}

fn pop_scope_infos(node: Node, scope_info: &mut Option<ScopeInfo>) {
    while scope_info
        .as_ref()
        .matches(|scope_info| !node.is_descendant_of(scope_info.node))
    {
        let scope_info_present = scope_info.take().unwrap();
        *scope_info = scope_info_present.upper.map(|upper| *upper);
    }
}

fn enter_breakable_statement<'a>(node: Node<'a>, scope_info: &mut Option<ScopeInfo<'a>>) {
    let old_scope_info = scope_info.take();
    let parent = node.next_non_parentheses_ancestor();
    *scope_info = Some(ScopeInfo {
        label: if parent.kind() == LabeledStatement {
            Some(parent.field("label"))
        } else {
            None
        },
        breakable: true,
        upper: old_scope_info.map(Box::new),
        node,
    });
}

fn enter_labeled_statement<'a>(node: Node<'a>, scope_info: &mut Option<ScopeInfo<'a>>) {
    if !ast_utils::is_breakable_statement(node.field("body")) {
        let old_scope_info = scope_info.take();
        *scope_info = Some(ScopeInfo {
            label: Some(node.field("label")),
            breakable: false,
            upper: old_scope_info.map(Box::new),
            node,
        });
    }
}

fn report_if_unnecessary<'a>(
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_, impl FromFileRunContextInstanceProviderFactory>,
    mut scope_info: Option<&ScopeInfo>,
) {
    let label_node = return_if_none!(node.child_by_field_name("label"));
    let label_node_name = label_node.text(context);

    while let Some(info) = scope_info {
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
        scope_info = info.upper.as_deref();
    }
}

pub fn no_extra_label_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-extra-label",
        languages => [Javascript],
        messages => [
            unexpected => "This label '{{name}}' is unnecessary.",
        ],
        fixable => true,
        state => {
            [per-file-run]
            scope_info: Option<ScopeInfo<'a>>,
        },
        listeners => [
            r#"
              (while_statement) @c
              (do_statement) @c
              (for_statement) @c
              (for_in_statement) @c
              (switch_statement) @c
            "# => |node, context| {
                pop_scope_infos(node, &mut self.scope_info);
                enter_breakable_statement(node, &mut self.scope_info);
            },
            r#"
              (labeled_statement) @c
            "# => |node, context| {
                pop_scope_infos(node, &mut self.scope_info);
                enter_labeled_statement(node, &mut self.scope_info);
            },
            r#"
              (break_statement) @c
              (continue_statement) @c
            "# => |node, context| {
                pop_scope_infos(node, &mut self.scope_info);
                report_if_unnecessary(node, context, self.scope_info.as_ref());
            },
        ]
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
