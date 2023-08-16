use std::sync::Arc;

use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, Rule};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{ForStatement, SequenceExpression},
};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_for_loop_afterthoughts: bool,
}

fn is_for_statement_update(node: Node) -> bool {
    let parent = node.next_non_parentheses_ancestor();

    parent.kind() == ForStatement
        && parent
            .child_by_field_name("increment")
            .map(|increment| increment.skip_parentheses())
            .matches(|increment| increment == node)
}

fn is_for_loop_afterthought(node: Node) -> bool {
    let parent = node.next_non_parentheses_ancestor();

    if parent.kind() == SequenceExpression {
        return is_for_loop_afterthought(parent);
    }

    is_for_statement_update(node)
}

pub fn no_plusplus_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-plusplus",
        languages => [Javascript],
        messages => [
            unexpected_unary_op => "Unary operator '{{operator}}' used.",
        ],
        options_type => Options,
        state => {
            [per-run]
            allow_for_loop_afterthoughts: bool = options.allow_for_loop_afterthoughts,
        },
        listeners => [
            r#"
              (update_expression) @c
            "# => |node, context| {
                if self.allow_for_loop_afterthoughts && is_for_loop_afterthought(node) {
                    return;
                }

                context.report(violation! {
                    node => node,
                    message_id => "unexpected_unary_op",
                    data => {
                        operator => node.field("operator").text(context)
                    }
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::UpdateExpression;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_plusplus_rule() {
        RuleTester::run(
            no_plusplus_rule(),
            rule_tests! {
                valid => [
                    "var foo = 0; foo=+1;",

                    // With "allow_for_loop_afterthoughts" allowed
                    { code => "var foo = 0; foo=+1;", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (i = 0; i < l; i++) { console.log(i); }", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (var i = 0, j = i + 1; j < example.length; i++, j++) {}", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (;; i--, foo());", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (;; foo(), --i);", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (;; foo(), ++i, bar);", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (;; i++, (++j, k--));", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (;; foo(), (bar(), i++), baz());", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (;; (--i, j += 2), bar = j + 1);", options => { allow_for_loop_afterthoughts => true } },
                    { code => "for (;; a, (i--, (b, ++j, c)), d);", options => { allow_for_loop_afterthoughts => true } }
                ],

                invalid => [
                    {
                        code => "var foo = 0; foo++;",
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "var foo = 0; foo--;",
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "--"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (i = 0; i < l; i++) { console.log(i); }",
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (i = 0; i < l; foo, i++) { console.log(i); }",
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },

                    // With "allow_for_loop_afterthoughts" allowed
                    {
                        code => "var foo = 0; foo++;",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (i = 0; i < l; i++) { v++; }",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (i++;;);",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (;--i;);",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "--"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (;;) ++i;",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (;; i = j++);",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (;; i++, f(--j));",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "--"
                            },
                            type => UpdateExpression
                        }]
                    },
                    {
                        code => "for (;; foo + (i++, bar));",
                        options => { allow_for_loop_afterthoughts => true },
                        errors => [{
                            message_id => "unexpected_unary_op",
                            data => {
                                operator => "++"
                            },
                            type => UpdateExpression
                        }]
                    }
                ]
            },
        )
    }
}
