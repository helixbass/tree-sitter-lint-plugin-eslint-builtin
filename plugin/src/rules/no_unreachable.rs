use std::{
    collections::{HashMap, HashSet},
    ops,
    sync::Arc,
};

use once_cell::sync::Lazy;
use regex::Regex;
use squalid::VecExt;
use tree_sitter_lint::{
    compare_nodes, rule,
    tree_sitter::{Node, Range},
    violation, NodeExt, QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::{get_method_definition_kind, MethodDefinitionKind},
    kind::{
        BreakStatement, ClassDeclaration, ClassHeritage, ContinueStatement, DebuggerStatement,
        DoStatement, ExportStatement, ExpressionStatement, FieldDefinition, ForInStatement,
        ForStatement, IfStatement, ImportStatement, LabeledStatement, LexicalDeclaration,
        ReturnStatement, StatementBlock, SwitchStatement, ThrowStatement, TryStatement,
        VariableDeclaration, WhileStatement, WithStatement,
    },
    CodePathAnalyzer, EnterOrExit,
};

static TARGET_NODE_KINDS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(
        r#"^(?:{StatementBlock}|{BreakStatement}|{ClassDeclaration}|{ContinueStatement}|{DebuggerStatement}|{DoStatement}|{ExpressionStatement}|{ForInStatement}|{ForStatement}|{IfStatement}|{ImportStatement}|{LabeledStatement}|{ReturnStatement}|{SwitchStatement}|{ThrowStatement}|{TryStatement}|{WhileStatement}|{WithStatement}|{ExportStatement}|{LexicalDeclaration})$"#
    )).unwrap()
});

fn is_target_node(node: Node, context: &QueryMatchContext) -> bool {
    if TARGET_NODE_KINDS.is_match(node.kind()) {
        return true;
    }
    if node.kind() == VariableDeclaration
        && node
            .non_comment_named_children(context)
            .any(|child| child.child_by_field_name("value").is_some())
    {
        return true;
    }
    false
}

#[derive(Copy, Clone)]
struct ConsecutiveRange<'a> {
    start_node: Node<'a>,
    end_node: Node<'a>,
}

impl<'a> ConsecutiveRange<'a> {
    pub fn new(node: Node<'a>) -> Self {
        Self {
            start_node: node,
            end_node: node,
        }
    }

    pub fn contains(&self, node: Node<'a>) -> bool {
        node.end_byte() <= self.end_node.end_byte()
    }

    pub fn is_consecutive(&self, node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> bool {
        self.contains(context.get_token_before(node, Option::<fn(Node) -> bool>::None))
    }

    pub fn merge(&mut self, node: Node<'a>) {
        self.end_node = node;
    }

    pub fn range(&self) -> Range {
        Range {
            start_byte: self.start_node.start_byte(),
            end_byte: self.end_node.end_byte(),
            start_point: self.start_node.range().start_point,
            end_point: self.end_node.range().end_point,
        }
    }
}

#[derive(Clone, Default)]
struct ConsecutiveRanges<'a>(Vec<ConsecutiveRange<'a>>);

impl<'a> ConsecutiveRanges<'a> {
    pub fn add(&mut self, node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
        if self.is_empty() {
            self.push(ConsecutiveRange::new(node));
            return;
        }
        let range = self.last_mut().unwrap();
        if range.contains(node) {
            return;
        }
        if range.is_consecutive(node, context) {
            range.merge(node);
            return;
        }
        self.push(ConsecutiveRange::new(node));
    }
}

impl<'a> ops::Deref for ConsecutiveRanges<'a> {
    type Target = Vec<ConsecutiveRange<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> ops::DerefMut for ConsecutiveRanges<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

pub fn no_unreachable_rule() -> Arc<dyn Rule> {
    type HasSuperCall = bool;

    rule! {
        name => "no-unreachable",
        languages => [Javascript],
        messages => [
            unreachable_code => "Unreachable code.",
        ],
        state => {
            [per-file-run]
            constructor_infos: Vec<HasSuperCall>,
            ranges: ConsecutiveRanges<'a>,
        },
        listeners => [
            "program:exit" => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                type NodeId = usize;
                let mut reachable_nodes: HashSet<NodeId> = Default::default();
                let mut maybe_unreachable_nodes: HashMap<NodeId, Node<'_>> = Default::default();
                for &code_path in &code_path_analyzer.code_paths {
                    code_path_analyzer.code_path_arena[code_path]
                        .traverse_all_segments(
                            &code_path_analyzer.code_path_segment_arena,
                            None,
                            |_, segment, _| {
                                code_path_analyzer.code_path_segment_arena[segment]
                                    .nodes
                                    .iter()
                                    .filter(|(enter_or_exit, _)| {
                                        matches!(
                                            enter_or_exit,
                                            EnterOrExit::Enter,
                                        )
                                    })
                                    .for_each(|(_, node)| {
                                        if is_target_node(*node, context) {
                                            if code_path_analyzer.code_path_segment_arena[segment]
                                                .reachable {
                                                reachable_nodes.insert(node.id());
                                            } else {
                                                maybe_unreachable_nodes.insert(node.id(), *node);
                                            }
                                        }
                                    });
                            }
                        );
                }
                for range in maybe_unreachable_nodes
                    .into_iter()
                    .filter(|(node_id, _)| !reachable_nodes.contains(node_id))
                    .map(|(_, node)| node)
                    .collect::<Vec<_>>()
                    .and_sort_by(compare_nodes)
                    .into_iter()
                    .fold(self.ranges.clone(), |mut ranges, node| {
                        ranges.add(node, context);
                        ranges
                    }).iter() {
                    context.report(violation! {
                        message_id => "unreachable_code",
                        range => range.range(),
                        node => range.start_node,
                    });
                }
            },
            "method_definition" => |node, context| {
                if get_method_definition_kind(node, context) !=
                    MethodDefinitionKind::Constructor {
                    return;
                }

                self.constructor_infos.push(false);
            },
            "method_definition:exit" => |node, context| {
                if get_method_definition_kind(node, context) !=
                    MethodDefinitionKind::Constructor {
                    return;
                }

                let has_super_call = self.constructor_infos.pop().unwrap();

                let class_definition = node.parent().unwrap().parent().unwrap();

                if class_definition.has_child_of_kind(ClassHeritage) &&
                    !has_super_call {
                    for element in class_definition.field("body").non_comment_named_children(context) {
                        if element.kind() == FieldDefinition && !element.has_child_of_kind("static") {
                            self.ranges.add(element, context);
                            // `;` wasn't getting included
                            self.ranges.add(context.get_token_after(element, Option::<fn(Node) -> bool>::None), context);
                        }
                    }
                }
                // if (!node.value.body) {
                //     return;
                // }
            },
            "(call_expression
              function: (super)
            ) @c" => |node, context| {
                if let Some(constructor_info) = self.constructor_infos.last_mut() {
                    *constructor_info = true;
                }
            }
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kind::{ExpressionStatement, LexicalDeclaration, StatementBlock, VariableDeclaration},
        get_instance_provider_factory,
    };

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_unreachable_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_unreachable_rule(),
            rule_tests! {
                valid => [
                    "function foo() { function bar() { return 1; } return bar(); }",
                    "function foo() { return bar(); function bar() { return 1; } }",
                    "function foo() { return x; var x; }",
                    "function foo() { var x = 1; var y = 2; }",
                    "function foo() { var x = 1; var y = 2; return; }",
                    "while (true) { switch (foo) { case 1: x = 1; x = 2;} }",
                    "while (true) { break; var x; }",
                    "while (true) { continue; var x, y; }",
                    "while (true) { throw 'message'; var x; }",
                    "while (true) { if (true) break; var x = 1; }",
                    "while (true) continue;",
                    "switch (foo) { case 1: break; var x; }",
                    "switch (foo) { case 1: break; var x; default: throw true; };",
                    {
                        code => "const arrow_direction = arrow => {  switch (arrow) { default: throw new Error();  };}",
                        // parserOptions: {
                        //     ecmaVersion: 6
                        // }
                    },
                    "var x = 1; y = 2; throw 'uh oh'; var y;",
                    "function foo() { var x = 1; if (x) { return; } x = 2; }",
                    "function foo() { var x = 1; if (x) { } else { return; } x = 2; }",
                    "function foo() { var x = 1; switch (x) { case 0: break; default: return; } x = 2; }",
                    "function foo() { var x = 1; while (x) { return; } x = 2; }",
                    "function foo() { var x = 1; for (x in {}) { return; } x = 2; }",
                    "function foo() { var x = 1; try { return; } finally { x = 2; } }",
                    "function foo() { var x = 1; for (;;) { if (x) break; } x = 2; }",
                    "A: { break A; } foo()",
                    {
                        code => "function* foo() { try { yield 1; return; } catch (err) { return err; } }",
                        // parserOptions: {
                        //     ecmaVersion: 6
                        // }
                    },
                    {
                        code => "function foo() { try { bar(); return; } catch (err) { return err; } }",
                        // parserOptions: {
                        //     ecmaVersion: 6
                        // }
                    },
                    {
                        code => "function foo() { try { a.b.c = 1; return; } catch (err) { return err; } }",
                        // parserOptions: {
                        //     ecmaVersion: 6
                        // }
                    },
                    {
                        code => "class C { foo = reachable; }",
                        // parserOptions: { ecmaVersion: 2022 }
                    },
                    {
                        code => "class C { foo = reachable; constructor() {} }",
                        // parserOptions: { ecmaVersion: 2022 }
                    },
                    {
                        code => "class C extends B { foo = reachable; }",
                        // parserOptions: { ecmaVersion: 2022 }
                    },
                    {
                        code => "class C extends B { foo = reachable; constructor() { super(); } }",
                        // parserOptions: { ecmaVersion: 2022 }
                    },
                    {
                        code => "class C extends B { static foo = reachable; constructor() {} }",
                        // parserOptions: { ecmaVersion: 2022 }
                    }
                ],
                invalid => [
                    { code => "function foo() { return x; var x = 1; }", errors => [{ message_id => "unreachable_code", type => VariableDeclaration }] },
                    { code => "function foo() { return x; var x, y = 1; }", errors => [{ message_id => "unreachable_code", type => VariableDeclaration }] },
                    { code => "while (true) { continue; var x = 1; }", errors => [{ message_id => "unreachable_code", type => VariableDeclaration }] },
                    { code => "function foo() { return; x = 1; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { throw error; x = 1; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "while (true) { break; x = 1; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "while (true) { continue; x = 1; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { switch (foo) { case 1: return; x = 1; } }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { switch (foo) { case 1: throw e; x = 1; } }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "while (true) { switch (foo) { case 1: break; x = 1; } }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "while (true) { switch (foo) { case 1: continue; x = 1; } }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "var x = 1; throw 'uh oh'; var y = 2;", errors => [{ message_id => "unreachable_code", type => VariableDeclaration }] },
                    { code => "function foo() { var x = 1; if (x) { return; } else { throw e; } x = 2; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { var x = 1; if (x) return; else throw -1; x = 2; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { var x = 1; try { return; } finally {} x = 2; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { var x = 1; try { } finally { return; } x = 2; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { var x = 1; do { return; } while (x); x = 2; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { var x = 1; while (x) { if (x) break; else continue; x = 2; } }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { var x = 1; for (;;) { if (x) continue; } x = 2; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    { code => "function foo() { var x = 1; while (true) { } x = 2; }", errors => [{ message_id => "unreachable_code", type => ExpressionStatement }] },
                    {
                        code => "const arrow_direction = arrow => {  switch (arrow) { default: throw new Error();  }; g() }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 1,
                                column => 86,
                                end_line => 1,
                                end_column => 89
                            }
                        ]
                    },

                    // Merge the warnings of continuous unreachable nodes.
                    {
                        code => "
                function foo() {
                    return;

                    a();  // ← ERROR: Unreachable code. (no-unreachable)

                    b()   // ↑ ';' token is included in the unreachable code, so this statement will be merged.
                    // comment
                    c();  // ↑ ')' token is included in the unreachable code, so this statement will be merged.
                }
            ",
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 5,
                                column => 21,
                                end_line => 9,
                                end_column => 25
                            }
                        ]
                    },
                    {
                        code => "
                function foo() {
                    return;

                    a();

                    if (b()) {
                        c()
                    } else {
                        d()
                    }
                }
            ",
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 5,
                                column => 21,
                                end_line => 11,
                                end_column => 22
                            }
                        ]
                    },
                    {
                        code => "
                function foo() {
                    if (a) {
                        return
                        b();
                        c();
                    } else {
                        throw err
                        d();
                    }
                }
            ",
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 5,
                                column => 25,
                                end_line => 6,
                                end_column => 29
                            },
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 9,
                                column => 25,
                                end_line => 9,
                                end_column => 29
                            }
                        ]
                    },
                    {
                        code => "
                function foo() {
                    if (a) {
                        return
                        b();
                        c();
                    } else {
                        throw err
                        d();
                    }
                    e();
                }
            ",
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 5,
                                column => 25,
                                end_line => 6,
                                end_column => 29
                            },
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 9,
                                column => 25,
                                end_line => 9,
                                end_column => 29
                            },
                            {
                                message_id => "unreachable_code",
                                type => ExpressionStatement,
                                line => 11,
                                column => 21,
                                end_line => 11,
                                end_column => 25
                            }
                        ]
                    },
                    {
                        code => "
                function* foo() {
                    try {
                        return;
                    } catch (err) {
                        return err;
                    }
                }",
                        // parserOptions: {
                        //     ecmaVersion: 6
                        // },
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => StatementBlock,
                                line => 5,
                                column => 35,
                                end_line => 7,
                                end_column => 22
                            }
                        ]
                    },
                    {
                        code => "
                function foo() {
                    try {
                        return;
                    } catch (err) {
                        return err;
                    }
                }",
                        // parserOptions: {
                        //     ecmaVersion: 6
                        // },
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => StatementBlock,
                                line => 5,
                                column => 35,
                                end_line => 7,
                                end_column => 22
                            }
                        ]
                    },
                    {
                        code => "
                function foo() {
                    try {
                        return;
                        let a = 1;
                    } catch (err) {
                        return err;
                    }
                }",
                        // parserOptions: {
                        //     ecmaVersion: 6
                        // },
                        errors => [
                            {
                                message_id => "unreachable_code",
                                type => LexicalDeclaration,
                                line => 5,
                                column => 25,
                                end_line => 5,
                                end_column => 35
                            },
                            {
                                message_id => "unreachable_code",
                                type => StatementBlock,
                                line => 6,
                                column => 35,
                                end_line => 8,
                                end_column => 22
                            }
                        ]
                    },

                    /*
                     * If `extends` exists, constructor exists, and the constructor doesn't
                     * contain `super()`, then the fields are unreachable because the
                     * evaluation of `super()` initializes fields in that case.
                     * In most cases, such an instantiation throws runtime errors, but
                     * doesn't throw if the constructor returns a value.
                     */
                    {
                        code => "class C extends B { foo; constructor() {} }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [{ message_id => "unreachable_code", column => 21, end_column => 25 }]
                    },
                    {
                        code => "class C extends B { foo = unreachable + code; constructor() {} }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [{ message_id => "unreachable_code", column => 21, end_column => 46 }]
                    },
                    {
                        code => "class C extends B { foo; bar; constructor() {} }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [{ message_id => "unreachable_code", column => 21, end_column => 30 }]
                    },
                    {
                        code => "class C extends B { foo; constructor() {} bar; }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            { message_id => "unreachable_code", column => 21, end_column => 25 },
                            { message_id => "unreachable_code", column => 43, end_column => 47 }
                        ]
                    },
                    {
                        code => "(class extends B { foo; constructor() {} bar; })",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            { message_id => "unreachable_code", column => 20, end_column => 24 },
                            { message_id => "unreachable_code", column => 42, end_column => 46 }
                        ]
                    },
                    {
                        code => "class B extends A { x; constructor() { class C extends D { [super().x]; constructor() {} } } }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            { message_id => "unreachable_code", column => 60, end_column => 72 }
                        ]
                    },
                    {
                        code => "class B extends A { x; constructor() { class C extends super().x { y; constructor() {} } } }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            { message_id => "unreachable_code", column => 68, end_column => 70 }
                        ]
                    },
                    {
                        code => "class B extends A { x; static y; z; static q; constructor() {} }",
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [
                            { message_id => "unreachable_code", column => 21, end_column => 23 },
                            { message_id => "unreachable_code", column => 34, end_column => 36 }
                        ]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
