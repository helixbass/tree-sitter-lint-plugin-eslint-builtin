use std::{borrow::Cow, sync::Arc};

use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{
    rule, tree_sitter::Node, violation, FromFileRunContextInstanceProviderFactory, NodeExt, Rule,
};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{BreakStatement, SwitchStatement},
    utils::ast_utils,
};

#[derive(Copy, Clone, Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_loop: bool,
    allow_switch: bool,
}

struct ScopeInfo<'a> {
    label: Cow<'a, str>,
    kind: BodyKind,
    node: Node<'a>,
}

fn pop_scope_infos(node: Node, scope_infos: &mut Vec<ScopeInfo>) {
    while !scope_infos.is_empty() && !node.is_descendant_of(scope_infos[scope_infos.len() - 1].node)
    {
        scope_infos.pop().unwrap();
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BodyKind {
    Loop,
    Switch,
    Other,
}

fn get_body_kind(node: Node) -> BodyKind {
    if ast_utils::is_loop(node) {
        BodyKind::Loop
    } else if node.kind() == SwitchStatement {
        BodyKind::Switch
    } else {
        BodyKind::Other
    }
}

fn is_allowed(kind: BodyKind, allow_loop: bool, allow_switch: bool) -> bool {
    match kind {
        BodyKind::Loop => allow_loop,
        BodyKind::Switch => allow_switch,
        BodyKind::Other => false,
    }
}

fn get_kind(label: &str, scope_infos: &[ScopeInfo]) -> BodyKind {
    scope_infos
        .into_iter()
        .rev()
        .find(|info| info.label == label)
        .map_or(BodyKind::Other, |info| info.kind)
}

pub fn no_labels_rule<
    TFromFileRunContextInstanceProviderFactory: FromFileRunContextInstanceProviderFactory,
>() -> Arc<dyn Rule<TFromFileRunContextInstanceProviderFactory>> {
    rule! {
        name => "no-labels",
        languages => [Javascript],
        messages => [
            unexpected_label => "Unexpected labeled statement.",
            unexpected_label_in_break => "Unexpected label in break statement.",
            unexpected_label_in_continue => "Unexpected label in continue statement.",
        ],
        options_type => Options,
        state => {
            [per-run]
            allow_loop: bool = options.allow_loop,
            allow_switch: bool = options.allow_switch,

            [per-file-run]
            scope_infos: Vec<ScopeInfo<'a>>,
        },
        listeners => [
            r#"(
              (labeled_statement) @c
            )"# => |node, context| {
                pop_scope_infos(node, &mut self.scope_infos);
                let body_kind = get_body_kind(node.field("body"));

                self.scope_infos.push(ScopeInfo {
                    label: node.field("label").text(context),
                    kind: body_kind,
                    node,
                });

                if !is_allowed(body_kind, self.allow_loop, self.allow_switch) {
                    context.report(violation! {
                        node => node,
                        message_id => "unexpected_label",
                    });
                }
            },
            r#"
              (break_statement) @c
              (continue_statement) @c
            "# => |node, context| {
                pop_scope_infos(node, &mut self.scope_infos);
                if node.child_by_field_name("label").matches(|label| {
                    !is_allowed(
                        get_kind(&label.text(context), &self.scope_infos),
                        self.allow_loop,
                        self.allow_switch,
                    )
                }) {
                    context.report(violation! {
                        node => node,
                        message_id => if node.kind() == BreakStatement {
                            "unexpected_label_in_break"
                        } else {
                            "unexpected_label_in_continue"
                        },
                    });
                }
            }
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_labels_rule() {
        RuleTester::run(
            no_labels_rule(),
            rule_tests! {
                valid => [
                    "var f = { label: foo ()}",
                    "while (true) {}",
                    "while (true) { break; }",
                    "while (true) { continue; }",

                    // {allow_loop => true} option.
                    { code => "A: while (a) { break A; }", options => { allow_loop => true } },
                    { code => "A: do { if (b) { break A; } } while (a);", options => { allow_loop => true } },
                    { code => "A: for (var a in obj) { for (;;) { switch (a) { case 0: continue A; } } }", options => { allow_loop => true } },

                    // {allow_switch => true} option.
                    { code => "A: switch (a) { case 0: break A; }", options => { allow_switch => true } }
                ],

                invalid => [
                    {
                        code => "label: while(true) {}",
                        errors => [{
                            message_id => "unexpected_label",
                            type => "labeled_statement"
                        }]
                    },
                    {
                        code => "label: while (true) { break label; }",
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "label: while (true) { continue label; }",
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_continue",
                                type => "continue_statement"
                            }
                        ]
                    },

                    {
                        code => "A: var foo = 0;",
                        errors => [{
                            message_id => "unexpected_label",
                            type => "labeled_statement"
                        }]
                    },
                    {
                        code => "A: break A;",
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: { if (foo()) { break A; } bar(); };",
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: if (a) { if (foo()) { break A; } bar(); };",
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: switch (a) { case 0: break A; default: break; };",
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: switch (a) { case 0: B: { break A; } default: break; };",
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },

                    // {allow_loop => true} option.
                    {
                        code => "A: var foo = 0;",
                        options => { allow_loop => true },
                        errors => [{
                            message_id => "unexpected_label",
                            type => "labeled_statement"
                        }]
                    },
                    {
                        code => "A: break A;",
                        options => { allow_loop => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: { if (foo()) { break A; } bar(); };",
                        options => { allow_loop => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: if (a) { if (foo()) { break A; } bar(); };",
                        options => { allow_loop => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: switch (a) { case 0: break A; default: break; };",
                        options => { allow_loop => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            },
                            {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },

                    // {allow_switch => true} option.
                    {
                        code => "A: var foo = 0;",
                        options => { allow_switch => true },
                        errors => [{
                            message_id => "unexpected_label",
                            type => "labeled_statement"
                        }]
                    },
                    {
                        code => "A: break A;",
                        options => { allow_switch => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            }, {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: { if (foo()) { break A; } bar(); };",
                        options => { allow_switch => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            }, {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: if (a) { if (foo()) { break A; } bar(); };",
                        options => { allow_switch => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            }, {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: while (a) { break A; }",
                        options => { allow_switch => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            }, {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: do { if (b) { break A; } } while (a);",
                        options => { allow_switch => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            }, {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    },
                    {
                        code => "A: for (var a in obj) { for (;;) { switch (a) { case 0: break A; } } }",
                        options => { allow_switch => true },
                        errors => [
                            {
                                message_id => "unexpected_label",
                                type => "labeled_statement"
                            }, {
                                message_id => "unexpected_label_in_break",
                                type => "break_statement"
                            }
                        ]
                    }
                ]
            },
        )
    }
}
