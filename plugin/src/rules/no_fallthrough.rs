use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use regex::Regex;
use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::get_comment_contents,
    directives::directives_pattern,
    kind::{
        BreakStatement, ReturnStatement, StatementBlock, SwitchCase, SwitchDefault, ThrowStatement,
    },
    CodePathAnalyzer, EnterOrExit,
};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    #[serde(with = "serde_regex")]
    comment_pattern: Regex,
    allow_empty_case: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            comment_pattern: Regex::new(r#"(?i)falls?\s?through"#).unwrap(),
            allow_empty_case: Default::default(),
        }
    }
}

fn has_blank_lines_between(node: Node, token: Node) -> bool {
    token.range().start_point.row > node.range().end_point.row + 1
}

fn is_fall_through_comment(comment: &str, fallthrough_comment_pattern: &Regex) -> bool {
    fallthrough_comment_pattern.is_match(comment) && !directives_pattern.is_match(comment.trim())
}

fn has_fallthrough_comment<'a>(
    case_which_falls_through: Node<'a>,
    subsequent_case: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
    fallthrough_comment_pattern: &Regex,
) -> bool {
    let mut cursor = case_which_falls_through.walk();
    let mut body_nodes = case_which_falls_through.children_by_field_name("body", &mut cursor);
    if let Some(block_body_node) = body_nodes
        .next()
        .filter(|body_node| body_node.kind() == StatementBlock && body_nodes.next().is_none())
    {
        let trailing_close_brace =
            context.get_last_token(block_body_node, Option::<fn(Node) -> bool>::None);
        let comment_in_block = context.get_comments_before(trailing_close_brace).next();

        if comment_in_block.matches(|comment_in_block| {
            is_fall_through_comment(
                &get_comment_contents(comment_in_block, context),
                fallthrough_comment_pattern,
            )
        }) {
            return true;
        }
    }

    let comment = context.get_comments_before(subsequent_case).next();

    comment.matches(|comment| {
        is_fall_through_comment(
            &get_comment_contents(comment, context),
            fallthrough_comment_pattern,
        )
    })
}

pub fn no_fallthrough_rule() -> Arc<dyn Rule> {
    type NodeId = usize;

    rule! {
        name => "no-fallthrough",
        languages => [Javascript],
        messages => [
            case => "Expected a 'break' statement before 'case'.",
            default => "Expected a 'break' statement before 'default'.",
        ],
        options_type => Options,
        state => {
            [per-run]
            comment_pattern: Regex = options.comment_pattern.clone(),
            allow_empty_case: bool = options.allow_empty_case,

            [per-file-run]
            potential_fallthrough_cases: HashMap<NodeId, Node<'a>>,
        },
        listeners => [
            r#"
              (switch_case) @c
            "# => |node, context| {
                if node.is_last_non_comment_named_child(context) {
                    return;
                }
                if node.child_by_field_name("body").is_none()
                    && (self.allow_empty_case
                        || !has_blank_lines_between(
                            node,
                            context.get_token_after(node, Option::<fn(Node) -> bool>::None),
                        ))
                {
                    return;
                }

                let mut cursor = node.walk();
                if node
                    .children_by_field_name("body", &mut cursor)
                    .last()
                    .matches(|last_statement| {
                        matches!(
                            last_statement.kind(),
                            BreakStatement | ReturnStatement | ThrowStatement,
                        )
                    })
                {
                    return;
                }

                self.potential_fallthrough_cases.insert(node.id(), node);
            },
            "program:exit" => |node, context| {
                if self.potential_fallthrough_cases.is_empty() {
                    return;
                }

                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                type NodeId = usize;
                let mut reachable_nodes: HashSet<NodeId> = Default::default();
                for &code_path in &code_path_analyzer.code_paths {
                    code_path_analyzer.code_path_arena[code_path].traverse_all_segments(
                        &code_path_analyzer.code_path_segment_arena,
                        None,
                        |_, segment, _| {
                            code_path_analyzer.code_path_segment_arena[segment]
                                .nodes
                                .iter()
                                .filter(|(enter_or_exit, node)| {
                                    matches!(enter_or_exit, EnterOrExit::Exit,)
                                        && matches!(node.kind(), SwitchCase | SwitchDefault)
                                })
                                .for_each(|(_, node)| {
                                    if code_path_analyzer.code_path_segment_arena[segment].reachable
                                    {
                                        reachable_nodes.insert(node.id());
                                    }
                                });
                        },
                    );
                }
                for candidate_switch_case_node in reachable_nodes
                    .into_iter()
                    .filter_map(|node_id| self.potential_fallthrough_cases.get(&node_id).copied())
                {
                    let next_case_node = candidate_switch_case_node
                        .next_named_sibling_of_kinds(&[SwitchCase, SwitchDefault]);
                    if has_fallthrough_comment(
                        candidate_switch_case_node,
                        next_case_node,
                        context,
                        &self.comment_pattern,
                    ) {
                        continue;
                    }
                    context.report(violation! {
                        message_id => match next_case_node.kind() {
                            SwitchCase => "case",
                            SwitchDefault => "default",
                            _ => unreachable!(),
                        },
                        node => next_case_node,
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::{kind::SwitchCase, CodePathAnalyzerInstanceProviderFactory};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    #[test]
    fn test_no_fallthrough_rule() {
        let errors_default = [RuleTestExpectedErrorBuilder::default()
            .message_id("default")
            .type_(SwitchDefault)
            .build()
            .unwrap()];

        RuleTester::run_with_from_file_run_context_instance_provider(
            no_fallthrough_rule(),
            rule_tests! {
                valid => [
                    "switch(foo) { case 0: a(); /* falls through */ case 1: b(); }",
                    "switch(foo) { case 0: a()\n /* falls through */ case 1: b(); }",
                    "switch(foo) { case 0: a(); /* fall through */ case 1: b(); }",
                    "switch(foo) { case 0: a(); /* fallthrough */ case 1: b(); }",
                    "switch(foo) { case 0: a(); /* FALLS THROUGH */ case 1: b(); }",
                    "switch(foo) { case 0: { a(); /* falls through */ } case 1: b(); }",
                    "switch(foo) { case 0: { a()\n /* falls through */ } case 1: b(); }",
                    "switch(foo) { case 0: { a(); /* fall through */ } case 1: b(); }",
                    "switch(foo) { case 0: { a(); /* fallthrough */ } case 1: b(); }",
                    "switch(foo) { case 0: { a(); /* FALLS THROUGH */ } case 1: b(); }",
                    "switch(foo) { case 0: { a(); } /* falls through */ case 1: b(); }",
                    "switch(foo) { case 0: { a(); /* falls through */ } /* comment */ case 1: b(); }",
                    "switch(foo) { case 0: { /* falls through */ } case 1: b(); }",
                    "function foo() { switch(foo) { case 0: a(); return; case 1: b(); }; }",
                    "switch(foo) { case 0: a(); throw 'foo'; case 1: b(); }",
                    "while (a) { switch(foo) { case 0: a(); continue; case 1: b(); } }",
                    "switch(foo) { case 0: a(); break; case 1: b(); }",
                    "switch(foo) { case 0: case 1: a(); break; case 2: b(); }",
                    "switch(foo) { case 0: case 1: break; case 2: b(); }",
                    "switch(foo) { case 0: case 1: break; default: b(); }",
                    "switch(foo) { case 0: case 1: a(); }",
                    "switch(foo) { case 0: case 1: a(); break; }",
                    "switch(foo) { case 0: case 1: break; }",
                    "switch(foo) { case 0:\n case 1: break; }",
                    "switch(foo) { case 0: // comment\n case 1: break; }",
                    "function foo() { switch(foo) { case 0: case 1: return; } }",
                    "function foo() { switch(foo) { case 0: {return;}\n case 1: {return;} } }",
                    "switch(foo) { case 0: case 1: {break;} }",
                    "switch(foo) { }",
                    "switch(foo) { case 0: switch(bar) { case 2: break; } /* falls through */ case 1: break; }",
                    "function foo() { switch(foo) { case 1: return a; a++; }}",
                    "switch (foo) { case 0: a(); /* falls through */ default:  b(); /* comment */ }",
                    "switch (foo) { case 0: a(); /* falls through */ default: /* comment */ b(); }",
                    "switch (foo) { case 0: if (a) { break; } else { throw 0; } default: b(); }",
                    "switch (foo) { case 0: try { break; } finally {} default: b(); }",
                    "switch (foo) { case 0: try {} finally { break; } default: b(); }",
                    "switch (foo) { case 0: try { throw 0; } catch (err) { break; } default: b(); }",
                    "switch (foo) { case 0: do { throw 0; } while(a); default: b(); }",
                    // TODO: I believe this is testing behavior of disabling-comments
                    // (vs testing the rule itself so to speak)? In which case if I
                    // support those then this can be uncommented?
                    // "switch (foo) { case 0: a(); \n// eslint-disable-next-line no-fallthrough\n case 1: }",
                    {
                        code => "switch(foo) { case 0: a(); /* no break */ case 1: b(); }",
                        options => {
                            comment_pattern => "no break"
                        }
                    },
                    {
                        code => "switch(foo) { case 0: a(); /* no break: need to execute b() */ case 1: b(); }",
                        options => {
                            comment_pattern => "no break:\\s?\\w+"
                        }
                    },
                    {
                        code => "switch(foo) { case 0: a();\n// need to execute b(), so\n// falling through\n case 1: b(); }",
                        options => {
                            comment_pattern => "falling through"
                        }
                    },
                    {
                        code => "switch(foo) { case 0: a(); /* break omitted */ default:  b(); /* comment */ }",
                        options => {
                            comment_pattern => "break omitted"
                        }
                    },
                    {
                        code => "switch(foo) { case 0: a(); /* caution: break is omitted intentionally */ case 1: b(); /* break omitted */ default: c(); }",
                        options => {
                            comment_pattern => "break[\\s\\w]+omitted"
                        }
                    },
                    {
                        code => "switch(foo) { case 0: \n\n\n case 1: b(); }",
                        options => { allow_empty_case => true }
                    },
                    {
                        code => "switch(foo) { case 0: \n /* with comments */  \n case 1: b(); }",
                        options => { allow_empty_case => true }
                    },
                    {
                        code => "switch (a) {\n case 1: ; break; \n case 3: }",
                        options => { allow_empty_case => true }
                    },
                    {
                        code => "switch (a) {\n case 1: ; break; \n case 3: }",
                        options => { allow_empty_case => false }
                    }
                ],
                invalid => [
                    {
                        code => "switch(foo) { case 0: a();\ncase 1: b() }",
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch(foo) { case 0: a();\ndefault: b() }",
                        errors => [
                            {
                                message_id => "default",
                                type => SwitchDefault,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch(foo) { case 0: a(); default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: if (a) { break; } default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: try { throw 0; } catch (err) {} default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: while (a) { break; } default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: do { break; } while (a); default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0:\n\n default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: {} default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: a(); { /* falls through */ } default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: { /* falls through */ } a(); default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: if (a) { /* falls through */ } default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: { { /* falls through */ } } default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: { /* comment */ } default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0:\n // comment\n default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: a(); /* falling through */ default: b() }",
                        errors => errors_default
                    },
                    {
                        code => "switch(foo) { case 0: a();\n/* no break */\ncase 1: b(); }",
                        options => {
                            comment_pattern => "break omitted"
                        },
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch(foo) { case 0: a();\n/* no break */\n/* todo: fix readability */\ndefault: b() }",
                        options => {
                            comment_pattern => "no break"
                        },
                        errors => [
                            {
                                message_id => "default",
                                type => SwitchDefault,
                                line => 4,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch(foo) { case 0: { a();\n/* no break */\n/* todo: fix readability */ }\ndefault: b() }",
                        options => {
                            comment_pattern => "no break"
                        },
                        errors => [
                            {
                                message_id => "default",
                                type => SwitchDefault,
                                line => 4,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch(foo) { case 0: \n /* with comments */  \ncase 1: b(); }",
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch(foo) { case 0:\n\ncase 1: b(); }",
                        options => {
                            allow_empty_case => false
                        },
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch(foo) { case 0:\n\ncase 1: b(); }",
                        options => {},
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "switch (a) { case 1: \n ; case 2:  }",
                        options => { allow_empty_case => false },
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 2,
                                column => 4
                            }
                        ]
                    },
                    {
                        code => "switch (a) { case 1: ; case 2: ; case 3: }",
                        options => { allow_empty_case => true },
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 1,
                                column => 24
                            },
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 1,
                                column => 34
                            }
                        ]
                    },
                    {
                        code => "switch (foo) { case 0: a(); \n// eslint-enable no-fallthrough\n case 1: }",
                        options => {},
                        errors => [
                            {
                                message_id => "case",
                                type => SwitchCase,
                                line => 3,
                                column => 2
                            }
                        ]
                    }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
