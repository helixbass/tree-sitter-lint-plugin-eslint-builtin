use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use id_arena::Id;
use once_cell::sync::Lazy;
use serde::Deserialize;
use squalid::{continue_if_none, return_default_if_none};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, Rule};

use crate::{
    kind::{ContinueStatement, DoStatement, ForInStatement, ForStatement, Kind, WhileStatement},
    CodePath, CodePathAnalyzer, CodePathSegment, EnterOrExit,
};

#[allow(clippy::enum_variant_names)]
#[derive(Deserialize)]
enum LoopType {
    WhileStatement,
    DoWhileStatement,
    ForStatement,
    ForInStatement,
    ForOfStatement,
}

impl LoopType {
    pub fn kind(&self) -> Kind {
        match self {
            LoopType::WhileStatement => WhileStatement,
            LoopType::DoWhileStatement => DoStatement,
            LoopType::ForStatement => ForStatement,
            LoopType::ForInStatement => ForInStatement,
            LoopType::ForOfStatement => ForInStatement,
        }
    }
}

static ALL_LOOP_TYPES: Lazy<HashSet<Kind>> =
    Lazy::new(|| [WhileStatement, DoStatement, ForStatement, ForInStatement].into());

fn is_looping_target(node: Node, target_loop_kinds: &HashSet<Kind>) -> bool {
    let parent = return_default_if_none!(node.parent());

    if !target_loop_kinds.contains(parent.kind()) {
        return false;
    }

    match parent.kind() {
        WhileStatement => node == parent.field("condition"),
        DoStatement => node == parent.field("body"),
        ForStatement => {
            Some(node) == parent.child_by_field_name("increment")
                || node == parent.field("condition")
                || node == parent.field("body")
        }
        ForInStatement => node == parent.field("left"),
        _ => unreachable!(),
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    ignore: Vec<LoopType>,
}

fn look_for_loops<'a>(
    code_path: Id<CodePath<'a>>,
    code_path_analyzer: &CodePathAnalyzer<'a>,
    target_loop_kinds: &HashSet<Kind>,
) -> HashMap<Id<CodePathSegment<'a>>, Node<'a>> {
    let mut loops_by_target_segments: HashMap<Id<CodePathSegment<'a>>, Node<'a>> =
        Default::default();
    code_path_analyzer.code_path_arena[code_path].traverse_segments_in_any_order(
        &code_path_analyzer.code_path_segment_arena,
        None,
        |_, segment, _| {
            if !code_path_analyzer.code_path_segment_arena[segment].reachable {
                return;
            }
            if let Some((_, node)) = code_path_analyzer.code_path_segment_arena[segment]
                .nodes
                .get(0)
                .filter(|(enter_or_exit, node)| {
                    *enter_or_exit == EnterOrExit::Enter
                        && is_looping_target(*node, target_loop_kinds)
                })
            {
                loops_by_target_segments.insert(segment, node.parent().unwrap());
            }
        },
    );

    loops_by_target_segments
}

fn get_difference(a: &HashSet<Kind>, b: &[LoopType]) -> HashSet<Kind> {
    let mut ret = a.clone();
    b.into_iter()
        .map(|loop_type| loop_type.kind())
        .for_each(|kind| {
            ret.remove(&kind);
        });
    ret
}

pub fn no_unreachable_loop_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-unreachable-loop",
        languages => [Javascript],
        messages => [
            invalid => "Invalid loop. Its body allows only one iteration.",
        ],
        options_type => Options,
        state => {
            [per-run]
            target_loop_kinds: HashSet<Kind> = get_difference(&ALL_LOOP_TYPES, &options.ignore),

            [per-file-run]
            loops_to_report: HashSet<Node<'a>>,
        },
        listeners => [
            "
              (while_statement) @c
              (do_statement) @c
              (for_statement) @c
              (for_in_statement) @c
            " => |node, context| {
                if !self.target_loop_kinds.contains(&node.kind()) {
                    return;
                }

                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                if !code_path_analyzer
                    .get_segments_that_include_node_enter(node)
                    .into_iter()
                    .any(|segment| {
                        code_path_analyzer
                            .code_path_segment_arena[segment]
                            .reachable
                    })
                {
                    return;
                }

                self.loops_to_report.insert(node);
            },
            "program:exit" => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                for &code_path in &code_path_analyzer
                    .code_paths {
                    let loops_by_target_segments = look_for_loops(
                        code_path,
                        code_path_analyzer,
                        &self.target_loop_kinds,
                    );

                    for (_, to_segment, node) in code_path_analyzer
                        .code_path_arena[code_path]
                        .state
                        .looped_segments
                        .iter()
                        .filter(|(from_segment, _, _)| {
                            code_path_analyzer
                                .code_path_segment_arena[*from_segment]
                                .reachable
                        }) {
                        let loop_ = continue_if_none!(loops_by_target_segments.get(to_segment).copied());

                        if loop_ == *node || node.kind() == ContinueStatement {
                            self.loops_to_report.remove(&loop_);
                        }
                    }
                }

                for &node in &self.loops_to_report {
                    context.report(violation! {
                        node => node,
                        message_id => "invalid",
                    });
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use once_cell::sync::Lazy;
    use squalid::regex;
    use std::collections::HashMap;
    use tree_sitter_lint::{
        rule_tests, RuleTestExpectedErrorBuilder, RuleTestInvalid, RuleTestInvalidBuilder,
        RuleTester,
    };

    use crate::{
        kind::{DoStatement, ForInStatement, ForStatement, WhileStatement},
        CodePathAnalyzerInstanceProviderFactory,
    };

    static LOOP_TEMPLATES: Lazy<HashMap<&'static str, Vec<&'static str>>> = Lazy::new(|| {
        [
            (
                "WhileStatement",
                vec!["while (a) <body>", "while (a && b) <body>"],
            ),
            (
                "DoWhileStatement",
                vec!["do <body> while (a)", "do <body> while (a && b)"],
            ),
            (
                "ForStatement",
                vec![
                    "for (a; b; c) <body>",
                    "for (var i = 0; i < a.length; i++) <body>",
                    "for (; b; c) <body>",
                    "for (; b < foo; c++) <body>",
                    "for (a; ; c) <body>",
                    "for (a = 0; ; c++) <body>",
                    "for (a; b;) <body>",
                    "for (a = 0; b < foo; ) <body>",
                    "for (; ; c) <body>",
                    "for (; ; c++) <body>",
                    "for (; b;) <body>",
                    "for (; b < foo; ) <body>",
                    "for (a; ;) <body>",
                    "for (a = 0; ;) <body>",
                    "for (;;) <body>",
                ],
            ),
            (
                "ForInStatement",
                vec![
                    "for (a in b) <body>",
                    "for (a in f(b)) <body>",
                    "for (var a in b) <body>",
                    "for (let a in f(b)) <body>",
                ],
            ),
            (
                "ForOfStatement",
                vec![
                    "for (a of b) <body>",
                    "for (a of f(b)) <body>",
                    "for ({ a, b } of c) <body>",
                    "for (var a of f(b)) <body>",
                    "async function foo() { for await (const a of b) <body> }",
                ],
            ),
        ]
        .into()
    });

    static VALID_LOOP_BODIES: Lazy<Vec<&'static str>> = Lazy::new(|| {
        vec![
            ";",
            "{}",
            "{ bar(); }",
            "continue;",
            "{ continue; }",
            "{ if (foo) break; }",
            "{ if (foo) { return; } bar(); }",
            "{ if (foo) { bar(); } else { break; } }",
            "{ if (foo) { continue; } return; }",
            "{ switch (foo) { case 1: return; } }",
            "{ switch (foo) { case 1: break; default: return; } }",
            "{ switch (foo) { case 1: continue; default: return; } throw err; }",
            "{ try { return bar(); } catch (e) {} }",
            // unreachable break
            "{ continue; break; }",
            // functions in loops
            "() => a;",
            "{ () => a }",
            "(() => a)();",
            "{ (() => a)() }",
            // loops in loops
            "while (a);",
            "do ; while (a)",
            "for (a; b; c);",
            "for (; b;);",
            "for (; ; c) if (foo) break;",
            "for (;;) if (foo) break;",
            "while (true) if (foo) break;",
            "while (foo) if (bar) return;",
            "for (a in b);",
            "for (a of b);",
        ]
    });

    static INVALID_LOOP_BODIES: Lazy<Vec<&'static str>> = Lazy::new(|| {
        vec![
            "break;",
            "{ break; }",
            "return;",
            "{ return; }",
            "throw err;",
            "{ throw err; }",
            "{ foo(); break; }",
            "{ break; foo(); }",
            "if (foo) break; else return;",
            "{ if (foo) { return; } else { break; } bar(); }",
            "{ if (foo) { return; } throw err; }",
            "{ switch (foo) { default: throw err; } }",
            "{ switch (foo) { case 1: throw err; default: return; } }",
            "{ switch (foo) { case 1: something(); default: return; } }",
            "{ try { return bar(); } catch (e) { break; } }",
            // unreachable continue
            "{ break; continue; }",
            // functions in loops
            "{ () => a; break; }",
            "{ (() => a)(); break; }",
            // loops in loops
            "{ while (a); break; }",
            "{ do ; while (a) break; }",
            "{ for (a; b; c); break; }",
            "{ for (; b;); break; }",
            "{ for (; ; c) if (foo) break; break; }",
            "{ for(;;) if (foo) break; break; }",
            "{ for (a in b); break; }",
            "{ for (a of b); break; }",
            "for (;;);",
            "{ for (var i = 0; ; i< 10) { foo(); } }",
            "while (true);",
        ]
    });

    fn get_source_code(template: &str, body: &str) -> String {
        let loop_ = regex!("<body>").replace(template, body);

        if body.contains("return") && !template.contains("function") {
            format!("function someFunc() {{ {loop_} }}")
        } else {
            loop_.into_owned()
        }
    }

    // TODO: these aren't parsing correctly https://github.com/tree-sitter/tree-sitter-javascript/issues/263
    static NOT_PARSING_TEST_CASES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
        [
            "do do ; while (a) while (a)",
            "do do ; while (a) while (a && b)",
            "for (a in b) { do ; while (a) break; }",
            "do { do ; while (a) break; } while (a)",
            "while (a) { do ; while (a) break; }",
            "for (a; b; c) { do ; while (a) break; }",
            "for (a of b) { do ; while (a) break; }",
            "do { do ; while (a) break; } while (a)",
            "for (a of b) { do ; while (a) break; }",
            "while (a && b) { do ; while (a) break; }",
            "for (a in f(b)) { do ; while (a) break; }",
            "for (var a in b) { do ; while (a) break; }",
            "for (var i = 0; i < a.length; i++) { do ; while (a) break; }",
            "do { do ; while (a) break; } while (a && b)",
            "for (let a in f(b)) { do ; while (a) break; }",
            "for (a of f(b)) { do ; while (a) break; }",
            "for ({ a, b } of c) { do ; while (a) break; }",
            "for (; b; c) { do ; while (a) break; }",
            "for (; b < foo; c++) { do ; while (a) break; }",
            "for (var a of f(b)) { do ; while (a) break; }",
            "for (a; ; c) { do ; while (a) break; }",
            "async function foo() { for await (const a of b) { do ; while (a) break; } }",
            "for (a = 0; ; c++) { do ; while (a) break; }",
            "for (a; b;) { do ; while (a) break; }",
            "for (a = 0; b < foo; ) { do ; while (a) break; }",
            "for (; ; c) { do ; while (a) break; }",
            "for (; ; c++) { do ; while (a) break; }",
            "for (; b;) { do ; while (a) break; }",
            "for (; b < foo; ) { do ; while (a) break; }",
            "for (a; ;) { do ; while (a) break; }",
            "for (a = 0; ;) { do ; while (a) break; }",
            "for (;;) { do ; while (a) break; }",
            "for (a in b) { do ; while (a) break; }",
            "for (a in f(b)) { do ; while (a) break; }",
            "for (var a in b) { do ; while (a) break; }",
            "for (let a in f(b)) { do ; while (a) break; }",
            "for (a of b) { do ; while (a) break; }",
            "for (a of f(b)) { do ; while (a) break; }",
            "for ({ a, b } of c) { do ; while (a) break; }",
            "for (var a of f(b)) { do ; while (a) break; }",
            "async function foo() { for await (const a of b) { do ; while (a) break; } }",
        ]
        .into()
    });

    fn get_basic_valid_tests() -> Vec<String> {
        LOOP_TEMPLATES
            .values()
            .flat_map(|templates| {
                templates.iter().flat_map(|template| {
                    VALID_LOOP_BODIES
                        .iter()
                        .map(|body| get_source_code(template, body))
                })
            })
            .filter(|code| !NOT_PARSING_TEST_CASES.contains(&&**code))
            .collect()
    }

    fn get_basic_invalid_tests() -> Vec<RuleTestInvalid> {
        LOOP_TEMPLATES
            .iter()
            .flat_map(|(type_, templates)| {
                templates.iter().flat_map(|template| {
                    INVALID_LOOP_BODIES.iter().map(|body| {
                        RuleTestInvalidBuilder::default()
                            .code(get_source_code(template, body))
                            .errors(vec![RuleTestExpectedErrorBuilder::default()
                                .type_(match *type_ {
                                    "WhileStatement" => WhileStatement,
                                    "DoWhileStatement" => DoStatement,
                                    "ForStatement" => ForStatement,
                                    "ForInStatement" => ForInStatement,
                                    "ForOfStatement" => ForInStatement,
                                    _ => unreachable!(),
                                })
                                .message_id("invalid")
                                .build()
                                .unwrap()])
                            .build()
                            .unwrap()
                    })
                })
            })
            .filter(|rule_test_invalid| !NOT_PARSING_TEST_CASES.contains(&&*rule_test_invalid.code))
            .collect()
    }

    #[test]
    fn test_no_unreachable_loop_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_unreachable_loop_rule(),
            rule_tests! {
                valid => [
                    ...get_basic_valid_tests(),

                    // out of scope for the code path analysis and consequently out of scope for this rule
                    "while (false) { foo(); }",
                    "while (bar) { foo(); if (true) { break; } }",
                    "do foo(); while (false)",
                    "for (x = 1; x < 10; i++) { if (x > 0) { foo(); throw err; } }",
                    "for (x of []);",
                    "for (x of [1]);",

                    // doesn't report unreachable loop statements, regardless of whether they would be valid or not in a reachable position
                    "function foo() { return; while (a); }",
                    "function foo() { return; while (a) break; }",
                    "while(true); while(true);",
                    "while(true); while(true) break;",

                    // "ignore"
                    {
                        code => "while (a) break;",
                        options => { ignore => ["WhileStatement"] }
                    },
                    {
                        code => "do break; while (a)",
                        options => { ignore => ["DoWhileStatement"] }
                    },
                    {
                        code => "for (a; b; c) break;",
                        options => { ignore => ["ForStatement"] }
                    },
                    {
                        code => "for (a in b) break;",
                        options => { ignore => ["ForInStatement"] }
                    },
                    {
                        code => "for (a of b) break;",
                        options => { ignore => ["ForOfStatement"] }
                    },
                    {
                        code => "for (var key in obj) { hasEnumerableProperties = true; break; } for (const a of b) break;",
                        options => { ignore => ["ForInStatement", "ForOfStatement"] }
                    }
                ],
                invalid => [
                    ...get_basic_invalid_tests(),

                    // invalid loop nested in a valid loop (valid in valid, and valid in invalid are covered by basic tests)
                    {
                        code => "while (foo) { for (a of b) { if (baz) { break; } else { throw err; } } }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            }
                        ]
                    },
                    {
                        code => "lbl: for (var i = 0; i < 10; i++) { while (foo) break lbl; } /* outer is valid because inner can have 0 iterations */",
                        errors => [
                            {
                                message_id => "invalid",
                                type => WhileStatement
                            }
                        ]
                    },

                    // invalid loop nested in another invalid loop
                    {
                        code => "for (a in b) { while (foo) { if(baz) { break; } else { break; } } break; }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            },
                            {
                                message_id => "invalid",
                                type => WhileStatement
                            }
                        ]
                    },

                    // loop and nested loop both invalid because of the same exit statement
                    {
                        code => "function foo() { for (var i = 0; i < 10; i++) { do { return; } while(i) } }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForStatement
                            },
                            {
                                message_id => "invalid",
                                type => DoStatement
                            }
                        ]
                    },
                    {
                        code => "lbl: while(foo) { do { break lbl; } while(baz) }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => WhileStatement
                            },
                            {
                                message_id => "invalid",
                                type => DoStatement
                            }
                        ]
                    },

                    // inner loop has continue, but to an outer loop
                    {
                        code => "lbl: for (a in b) { while(foo) { continue lbl; } }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => WhileStatement
                            }
                        ]
                    },

                    // edge cases - inner loop has only one exit path, but at the same time it exits the outer loop in the first iteration
                    {
                        code => "for (a of b) { for(;;) { if (foo) { throw err; } } }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            }
                        ]
                    },
                    {
                        code => "function foo () { for (a in b) { while (true) { if (bar) { return; } } } }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            }
                        ]
                    },

                    // edge cases where parts of the loops belong to the same code path segment, tests for false negatives
                    {
                        code => "do for (var i = 1; i < 10; i++) break; while(foo)",
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForStatement
                            }
                        ]
                    },
                    {
                        code => "do { for (var i = 1; i < 10; i++) continue; break; } while(foo)",
                        errors => [
                            {
                                message_id => "invalid",
                                type => DoStatement
                            }
                        ]
                    },
                    {
                        code => "for (;;) { for (var i = 1; i < 10; i ++) break; if (foo) break; continue; }",
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForStatement,
                                column => 12
                            }
                        ]
                    },

                    // "ignore"
                    {
                        code => "while (a) break; do break; while (b); for (;;) break; for (c in d) break; for (e of f) break;",
                        options => { ignore => [] },
                        errors => [
                            {
                                message_id => "invalid",
                                type => WhileStatement
                            },
                            {
                                message_id => "invalid",
                                type => DoStatement
                            },
                            {
                                message_id => "invalid",
                                type => ForStatement
                            },
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            },
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            }
                        ]
                    },
                    {
                        code => "while (a) break;",
                        options => { ignore => ["DoWhileStatement"] },
                        errors => [
                            {
                                message_id => "invalid",
                                type => WhileStatement
                            }
                        ]
                    },
                    {
                        code => "do break; while (a)",
                        options => { ignore => ["WhileStatement"] },
                        errors => [
                            {
                                message_id => "invalid",
                                type => DoStatement
                            }
                        ]
                    },
                    {
                        code => "for (a in b) break; for (c of d) break;",
                        options => { ignore => ["ForStatement"] },
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            },
                            {
                                message_id => "invalid",
                                type => ForInStatement
                            }
                        ]
                    },
                    {
                        code => "for (a in b) break; for (;;) break; for (c of d) break;",
                        options => { ignore => ["ForInStatement", "ForOfStatement"] },
                        errors => [
                            {
                                message_id => "invalid",
                                type => ForStatement
                            }
                        ]
                    }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
