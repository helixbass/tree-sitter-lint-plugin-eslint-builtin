use std::{collections::HashSet, sync::Arc};

use id_arena::Id;
use once_cell::sync::Lazy;
use regex::Regex;
use tree_sitter_lint::{compare_nodes, rule, tree_sitter::Node, violation, Rule};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{
        CatchClause, ClassDeclaration, ContinueStatement, DebuggerStatement, DoStatement,
        EmptyStatement, ExportStatement, ExpressionStatement, FinallyClause, ForInStatement,
        ForStatement, IfStatement, ImportStatement, LabeledStatement, LexicalDeclaration,
        ReturnStatement, SwitchStatement, ThrowStatement, TryStatement, VariableDeclaration,
        WhileStatement, WithStatement,
    },
    utils::{ast_utils, fix_tracker::FixTracker},
    CodePathAnalyzer, CodePathSegment, EnterOrExit,
};

static STATEMENT_SENTINEL: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r#"^(?:{ClassDeclaration}|{ContinueStatement}|{DebuggerStatement}|{DoStatement}|{EmptyStatement}|{ExpressionStatement}|{ForInStatement}|{ForStatement}|{IfStatement}|{ImportStatement}|{LabeledStatement}|{SwitchStatement}|{ThrowStatement}|{TryStatement}|{VariableDeclaration}|{LexicalDeclaration}|{WhileStatement}|{WithStatement}|{ExportStatement})$"#)).unwrap()
});

fn is_in_finally(node: Node) -> bool {
    let mut current_node = node;
    while let Some(parent) = current_node
        .parent()
        .filter(|_| !ast_utils::is_function(current_node))
    {
        if parent.kind() == TryStatement && parent.child_by_field_name("finalizer") == Some(node) {
            return true;
        }
        current_node = parent;
    }
    false
}

fn is_removable(node: Node) -> bool {
    ast_utils::STATEMENT_LIST_PARENTS.contains(node.parent().unwrap().kind())
}

fn look_for_trailing_return<'a>(
    segment: Id<CodePathSegment<'a>>,
    code_path_analyzer: &CodePathAnalyzer<'a>,
    nodes_to_report: &mut Vec<Node<'a>>,
    seen_segments: &mut HashSet<Id<CodePathSegment<'a>>>,
    reached_try_statements: &mut HashSet<Node<'a>>,
) {
    if seen_segments.contains(&segment) {
        return;
    }

    seen_segments.insert(segment);

    for (enter_or_exit, node) in code_path_analyzer.code_path_segment_arena[segment]
        .nodes
        .iter()
        .rev()
    {
        if *enter_or_exit == EnterOrExit::Exit {
            if matches!(node.kind(), CatchClause | FinallyClause) {
                reached_try_statements.insert(node.parent().unwrap());
            }
            continue;
        }

        if STATEMENT_SENTINEL.is_match(node.kind()) {
            return;
        }
        if node.kind() == ReturnStatement {
            if node.has_non_comment_named_children() {
                return;
            }
            if ast_utils::is_in_loop(*node) || is_in_finally(*node) {
                return;
            }
            if !code_path_analyzer.code_path_segment_arena[segment].reachable {
                continue;
            }

            nodes_to_report.push(*node);
        }
    }

    for &prev_segment in &code_path_analyzer.code_path_segment_arena[segment].all_prev_segments {
        look_for_trailing_return(
            prev_segment,
            code_path_analyzer,
            nodes_to_report,
            seen_segments,
            reached_try_statements,
        );
    }
}

pub fn no_useless_return_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-useless-return",
        languages => [Javascript],
        fixable => true,
        allow_self_conflicting_fixes => true,
        messages => [
            unnecessary_return => "Unnecessary return statement.",
        ],
        state => {
            [per-file-run]
            all_try_body_blocks: Vec<Node<'a>>,
        },
        listeners => [
            "(try_statement
              body: (statement_block) @c
            )" => |node, context| {
                self.all_try_body_blocks.push(node);
            },
            "program:exit" => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                let mut nodes_to_report: Vec<Node<'a>> = Default::default();

                let mut seen_segments: HashSet<Id<CodePathSegment<'a>>> = Default::default();

                let mut reached_try_statements: HashSet<Node<'a>> = Default::default();

                for &code_path in &code_path_analyzer.code_paths {
                    for &segment in &*code_path_analyzer.code_path_arena[code_path]
                        // .returned_segments() {
                        .state
                        .head_segments(&code_path_analyzer.fork_context_arena) {
                        look_for_trailing_return(
                            segment,
                            code_path_analyzer,
                            &mut nodes_to_report,
                            &mut seen_segments,
                            &mut reached_try_statements,
                        );
                    }
                }

                for &try_body_block in &self.all_try_body_blocks {
                    if !reached_try_statements.contains(&try_body_block.parent().unwrap()) {
                        continue;
                    }

                    for segment in code_path_analyzer.get_segments_that_include_node_exit(try_body_block) {
                        look_for_trailing_return(
                            segment,
                            code_path_analyzer,
                            &mut nodes_to_report,
                            &mut seen_segments,
                            &mut reached_try_statements,
                        );
                    }
                }

                nodes_to_report.sort_by(compare_nodes);
                nodes_to_report.dedup();

                for node in nodes_to_report {
                    context.report(violation! {
                        node => node,
                        message_id => "unnecessary_return",
                        fix => |fixer| {
                            if is_removable(node) && context.get_comments_inside(node).count() == 0 {
                                FixTracker::new(fixer, context)
                                    .retain_enclosing_function(node)
                                    .remove(node);
                            }
                        }
                    });
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::{kind::ReturnStatement, CodePathAnalyzerInstanceProviderFactory};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    #[test]
    fn test_no_useless_return_rule() {
        let errors = [RuleTestExpectedErrorBuilder::default()
            .message_id("unnecessary_return")
            .type_(ReturnStatement)
            .build()
            .unwrap()];

        RuleTester::run_with_from_file_run_context_instance_provider(
            no_useless_return_rule(),
            rule_tests! {
                valid => [
                    "function foo() { return 5; }",
                    "function foo() { return null; }",
                    "function foo() { return doSomething(); }",
                    "
                      function foo() {
                        if (bar) {
                          doSomething();
                          return;
                        } else {
                          doSomethingElse();
                        }
                        qux();
                      }
                    ",
                    "
                      function foo() {
                        switch (bar) {
                          case 1:
                            doSomething();
                            return;
                          default:
                            doSomethingElse();
                        }
                      }
                    ",
                    "
                      function foo() {
                        switch (bar) {
                          default:
                            doSomething();
                            return;
                          case 1:
                            doSomethingElse();
                        }
                      }
                    ",
                    "
                      function foo() {
                        switch (bar) {
                          case 1:
                            if (a) {
                              doSomething();
                              return;
                            } else {
                              doSomething();
                              return;
                            }
                          default:
                            doSomethingElse();
                        }
                      }
                    ",
                    "
                      function foo() {
                        for (var foo = 0; foo < 10; foo++) {
                          return;
                        }
                      }
                    ",
                    "
                      function foo() {
                        for (var foo in bar) {
                          return;
                        }
                      }
                    ",
                    "
                      function foo() {
                        try {
                          return 5;
                        } finally {
                          return; // This is allowed because it can override the returned value of 5
                        }
                      }
                    ",
                    "
                      function foo() {
                        try {
                          bar();
                          return;
                        } catch (err) {}
                        baz();
                      }
                    ",
                    "
                      function foo() {
                          if (something) {
                              try {
                                  bar();
                                  return;
                              } catch (err) {}
                          }
                          baz();
                      }
                    ",
                    "
                      function foo() {
                        return;
                        doSomething();
                      }
                    ",
                    {
                        code => "
                          function foo() {
                            for (var foo of bar) return;
                          }
                        ",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "() => { if (foo) return; bar(); }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "() => 5",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "() => { return; doSomething(); }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "if (foo) { return; } doSomething();",
                        // parserOptions: { ecmaFeatures: { globalReturn: true } }
                    },

                    // https://github.com/eslint/eslint/issues/7477
                    "
                      function foo() {
                        if (bar) return;
                        return baz;
                      }
                    ",
                    "
                      function foo() {
                        if (bar) {
                          return;
                        }
                        return baz;
                      }
                    ",
                    "
                      function foo() {
                        if (bar) baz();
                        else return;
                        return 5;
                      }
                    ",

                    // https://github.com/eslint/eslint/issues/7583
                    "
                      function foo() {
                        return;
                        while (foo) return;
                        foo;
                      }
                    ",

                    // https://github.com/eslint/eslint/issues/7855
                    "
                      try {
                        throw new Error('foo');
                        while (false);
                      } catch (err) {}
                    ",

                    // https://github.com/eslint/eslint/issues/11647
                    "
                      function foo(arg) {
                        throw new Error(\"Debugging...\");
                        if (!arg) {
                          return;
                        }
                        console.log(arg);
                      }
                    ",

                    // https://github.com/eslint/eslint/pull/16996#discussion_r1138622844
                    "
                    function foo() {
                      try {
                          bar();
                          return;
                      } finally {
                          baz();
                      }
                      qux();
                    }
                    "
                ],
                invalid => [
                    {
                        code => "function foo() { return; }",
                        output => "function foo() {  }",
                        errors => errors,
                    },
                    {
                        code => "function foo() { doSomething(); return; }",
                        output => "function foo() { doSomething();  }",
                        errors => errors,
                    },
                    {
                        code => "function foo() { if (condition) { bar(); return; } else { baz(); } }",
                        output => "function foo() { if (condition) { bar();  } else { baz(); } }",
                        errors => errors,
                    },
                    {
                        code => "function foo() { if (foo) return; }",
                        output => "function foo() { if (foo) return; }",
                        errors => errors,
                    },
                    {
                        code => "function foo() { bar(); return/**/; }",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "function foo() { bar(); return//\n; }",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo(); return;",
                        output => "foo(); ",
                        errors => errors,
                        // parserOptions: { ecmaFeatures: { globalReturn: true } }
                    },
                    {
                        code => "if (foo) { bar(); return; } else { baz(); }",
                        output => "if (foo) { bar();  } else { baz(); }",
                        errors => errors,
                        // parserOptions: { ecmaFeatures: { globalReturn: true } }
                    },
                    {
                        code => "
                          function foo() {
                            if (foo) {
                              return;
                            }
                            return;
                          }
                        ",
                        output => "
                          function foo() {
                            if (foo) {
                              
                            }
                            return;
                          }
                        ", // Other case is fixed in the second pass.
                        errors => [
                            { message_id => "unnecessary_return", type => ReturnStatement },
                            { message_id => "unnecessary_return", type => ReturnStatement }
                        ]
                    },
                    {
                        code => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                doSomething();
                              default:
                                doSomethingElse();
                                return;
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                doSomething();
                              default:
                                doSomethingElse();
                                
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            switch (bar) {
                              default:
                                doSomething();
                              case 1:
                                doSomething();
                                return;
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            switch (bar) {
                              default:
                                doSomething();
                              case 1:
                                doSomething();
                                
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                if (a) {
                                  doSomething();
                                  return;
                                }
                                break;
                              default:
                                doSomethingElse();
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                if (a) {
                                  doSomething();
                                  
                                }
                                break;
                              default:
                                doSomethingElse();
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                if (a) {
                                  doSomething();
                                  return;
                                } else {
                                  doSomething();
                                }
                                break;
                              default:
                                doSomethingElse();
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                if (a) {
                                  doSomething();
                                  
                                } else {
                                  doSomething();
                                }
                                break;
                              default:
                                doSomethingElse();
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                if (a) {
                                  doSomething();
                                  return;
                                }
                              default:
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            switch (bar) {
                              case 1:
                                if (a) {
                                  doSomething();
                                  
                                }
                              default:
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            try {} catch (err) { return; }
                          }
                        ",
                        output => "
                          function foo() {
                            try {} catch (err) {  }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            try {
                              foo();
                              return;
                            } catch (err) {
                              return 5;
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            try {
                              foo();
                              
                            } catch (err) {
                              return 5;
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                              if (something) {
                                  try {
                                      bar();
                                      return;
                                  } catch (err) {}
                              }
                          }
                        ",
                        output => "
                          function foo() {
                              if (something) {
                                  try {
                                      bar();
                                      
                                  } catch (err) {}
                              }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            try {
                              return;
                            } catch (err) {
                              foo();
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            try {
                              
                            } catch (err) {
                              foo();
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                              try {
                                  return;
                              } finally {
                                  bar();
                              }
                          }
                        ",
                        output => "
                          function foo() {
                              try {
                                  
                              } finally {
                                  bar();
                              }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            try {
                              bar();
                            } catch (e) {
                              try {
                                baz();
                                return;
                              } catch (e) {
                                qux();
                              }
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            try {
                              bar();
                            } catch (e) {
                              try {
                                baz();
                                
                              } catch (e) {
                                qux();
                              }
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            try {} finally {}
                            return;
                          }
                        ",
                        output => "
                          function foo() {
                            try {} finally {}
                            
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "
                          function foo() {
                            try {
                              return 5;
                            } finally {
                              function bar() {
                                return;
                              }
                            }
                          }
                        ",
                        output => "
                          function foo() {
                            try {
                              return 5;
                            } finally {
                              function bar() {
                                
                              }
                            }
                          }
                        ",
                        errors => errors,
                    },
                    {
                        code => "() => { return; }",
                        output => "() => {  }",
                        errors => errors,
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function foo() { return; return; }",
                        output => "function foo() {  return; }",
                        errors => [
                            {
                                message_id => "unnecessary_return",
                                type => ReturnStatement,
                                column => 18
                            }
                        ]
                    }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
