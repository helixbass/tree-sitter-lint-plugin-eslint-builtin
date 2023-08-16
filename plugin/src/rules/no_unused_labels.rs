use std::{borrow::Cow, sync::Arc};

use squalid::return_if_none;
use tree_sitter_lint::{
    range_between_starts, rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule,
    SkipOptionsBuilder,
};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{self, ExpressionStatement, LabeledStatement, Program, StatementBlock},
    utils::ast_utils,
};

struct ScopeInfo<'a> {
    label: Cow<'a, str>,
    used: bool,
    node: Node<'a>,
}

fn is_fixable<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
    let label = node.field("label");
    let body = node.field("body");

    if context.get_token_after(
        label,
        Some(
            SkipOptionsBuilder::<fn(Node) -> bool>::default()
                .include_comments(true)
                .build()
                .unwrap(),
        ),
    ) != context.get_token_before(
        body,
        Some(
            SkipOptionsBuilder::<fn(Node) -> bool>::default()
                .include_comments(true)
                .build()
                .unwrap(),
        ),
    ) {
        return false;
    }

    let ancestor = node.next_ancestor_not_of_type(LabeledStatement);

    #[allow(clippy::collapsible_if)]
    if ancestor.kind() == Program
        || ancestor.kind() == StatementBlock && ast_utils::is_function(ancestor.parent().unwrap())
    {
        if body.kind() == ExpressionStatement && {
            let expression = body.first_non_comment_named_child().skip_parentheses();
            expression.kind() == kind::String || ast_utils::is_static_template_literal(expression)
        } {
            return false;
        }
    }

    true
}

pub fn no_unused_labels_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-unused-labels",
        languages => [Javascript],
        messages => [
            unused => "'{{name}}:' is defined but never used.",
        ],
        fixable => true,
        state => {
            [per-file-run]
            scope_infos: Vec<ScopeInfo<'a>>,
        },
        listeners => [
            r#"
              (labeled_statement) @c
            "# => |node, context| {
                self.scope_infos.push(ScopeInfo {
                    node,
                    used: false,
                    label: node.field("label").text(context),
                });
            },
            "labeled_statement:exit" => |node, context| {
                let scope_info = self.scope_infos.pop().unwrap();
                if !scope_info.used {
                    let node = scope_info.node;
                    let label = node.field("label");
                    context.report(violation! {
                        node => label,
                        message_id => "unused",
                        data => {
                            name => label.text(context),
                        },
                        fix => |fixer| {
                            if !is_fixable(node, context) {
                                return;
                            }

                            fixer.remove_range(range_between_starts(node.range(), node.field("body").range()));
                        }
                    });
                }
            },
            r#"
              (break_statement) @c
              (continue_statement) @c
            "# => |node, context| {
                let label = return_if_none!(node.child_by_field_name("label")).text(context);

                for info in self.scope_infos.iter_mut().rev() {
                    if info.label == label {
                        info.used = true;
                        break;
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_unused_labels_rule() {
        RuleTester::run(
            no_unused_labels_rule(),
            rule_tests! {
                valid => [
                    "A: break A;",
                    "A: { foo(); break A; bar(); }",
                    "A: if (a) { foo(); if (b) break A; bar(); }",
                    "A: for (var i = 0; i < 10; ++i) { foo(); if (a) break A; bar(); }",
                    "A: for (var i = 0; i < 10; ++i) { foo(); if (a) continue A; bar(); }",
                    "A: { B: break B; C: for (var i = 0; i < 10; ++i) { foo(); if (a) break A; if (c) continue C; bar(); } }",
                    "A: { var A = 0; console.log(A); break A; console.log(A); }"
                ],
                invalid => [
                    {
                        code => "A: var foo = 0;",
                        output => "var foo = 0;",
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A: { foo(); bar(); }",
                        output => "{ foo(); bar(); }",
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A: if (a) { foo(); bar(); }",
                        output => "if (a) { foo(); bar(); }",
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A: for (var i = 0; i < 10; ++i) { foo(); if (a) break; bar(); }",
                        output => "for (var i = 0; i < 10; ++i) { foo(); if (a) break; bar(); }",
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A: for (var i = 0; i < 10; ++i) { foo(); if (a) continue; bar(); }",
                        output => "for (var i = 0; i < 10; ++i) { foo(); if (a) continue; bar(); }",
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A: for (var i = 0; i < 10; ++i) { B: break A; }",
                        output => "A: for (var i = 0; i < 10; ++i) { break A; }",
                        errors => [{ message_id => "unused", data => { name => "B" } }]
                    },
                    {
                        code => "A: { var A = 0; console.log(A); }",
                        output => "{ var A = 0; console.log(A); }",
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A: /* comment */ foo",
                        output => None,
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A /* comment */: foo",
                        output => None,
                        errors => [{ message_id => "unused" }]
                    },

                    // https://github.com/eslint/eslint/issues/16988
                    {
                        code => r#"A: "use strict""#,
                        output => None,
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => r#""use strict"; foo: "bar""#,
                        output => None,
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => r#"A: ("use strict")"#, // Parentheses may be removed by another rule.
                        output => None,
                        errors => [{ message_id => "unused" }]
                    },
                    {
                        code => "A: `use strict`", // `use strict` may be changed to "use strict" by another rule.
                        output => None,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{ message_id => "unused" }]
                    },
                    // TODO: this is not currently parsing correctly
                    // https://github.com/tree-sitter/tree-sitter-javascript/issues/259
                    // {
                    //     code => "if (foo) { bar: 'baz' }",
                    //     output => "if (foo) { 'baz' }",
                    //     errors => [{ message_id => "unused" }]
                    // },
                    {
                        code => "A: B: 'foo'",
                        output => "B: 'foo'",
                        errors => [{ message_id => "unused" }, { message_id => "unused" }]
                    },
                    {
                        code => "A: B: C: 'foo'",
                        // TODO: even when now only running a single fixing pass
                        // these all appear to be getting applied - maybe we have
                        // a different idea of "conflicting" than ESLint does (ie
                        // it would appear that the ESLint version of this rule is
                        // emitting multiple fixes the "first time" and ESLint is
                        // only applying a subset of them)?
                        // output => "B: C: 'foo'", // Becomes "C: 'foo'" on the second pass.
                        output => "C: 'foo'", // Becomes "C: 'foo'" on the second pass.
                        errors => [{ message_id => "unused" }, { message_id => "unused" }, { message_id => "unused" }]
                    },
                    {
                        code => "A: B: C: D: 'foo'",
                        // output => "B: D: 'foo'", // Becomes "D: 'foo'" on the second pass.
                        output => "D: 'foo'",
                        errors => [
                            { message_id => "unused" },
                            { message_id => "unused" },
                            { message_id => "unused" },
                            { message_id => "unused" }]
                    },
                    {
                        code => "A: B: C: D: E: 'foo'",
                        // output => "B: D: E: 'foo'", // Becomes "E: 'foo'" on the third pass.
                        output => "E: 'foo'",
                        errors => [
                            { message_id => "unused" },
                            { message_id => "unused" },
                            { message_id => "unused" },
                            { message_id => "unused" },
                            { message_id => "unused" }
                        ]
                    },
                    {
                        code => "A: 42",
                        output => "42",
                        errors => [{ message_id => "unused" }]
                    }

                    /*
                     * Below is fatal errors.
                     * "A: break B",
                     * "A: function foo() { break A; }",
                     * "A: class Foo { foo() { break A; } }",
                     * "A: { A: { break A; } }"
                     */
                ]
            },
        )
    }
}
