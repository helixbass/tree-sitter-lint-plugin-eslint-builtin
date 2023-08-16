use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use once_cell::sync::Lazy;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, Rule};

use crate::{
    ast_helpers::skip_nodes_of_types,
    kind::{
        DoStatement, ExpressionStatement, ForStatement, IfStatement, Kind, ParenthesizedExpression,
        TernaryExpression, WhileStatement,
    },
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ProhibitAssign {
    #[default]
    ExceptParens,
    Always,
}

static TEST_CONDITION_PARENT_TYPES: Lazy<HashSet<Kind>> = Lazy::new(|| {
    [
        IfStatement,
        WhileStatement,
        DoStatement,
        ForStatement,
        TernaryExpression,
    ]
    .into()
});

static NODE_DESCRIPTIONS: Lazy<HashMap<Kind, &'static str>> = Lazy::new(|| {
    [
        (DoStatement, "a 'do...while' statement"),
        (ForStatement, "a 'for' statement"),
        (IfStatement, "an 'if' statement"),
        (WhileStatement, "a 'while' statement"),
    ]
    .into()
});

fn is_conditional_test_expression(node: Node) -> bool {
    matches!(
        node.parent(),
        Some(parent) if TEST_CONDITION_PARENT_TYPES.contains(parent.kind()) &&
            Some(node) == parent.child_by_field_name("condition")
    )
}

fn find_conditional_ancestor(node: Node) -> Option<Node> {
    let mut current_ancestor = node;

    loop {
        if is_conditional_test_expression(current_ancestor) {
            return current_ancestor.parent();
        }
        current_ancestor = current_ancestor.parent()?;
        if ast_utils::is_function(current_ancestor) {
            return None;
        }
    }
}

pub fn no_cond_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-cond-assign",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected assignment within {{type}}.",
            missing => "Expected a conditional expression and instead saw an assignment.",
        ],
        options_type => ProhibitAssign,
        state => {
            [per-run]
            prohibit_assign: ProhibitAssign = options,
        },
        listeners => [
            r#"(assignment_expression) @c"# => |node, context| {
                if self.prohibit_assign != ProhibitAssign::Always {
                    return;
                }

                if let Some(ancestor) = find_conditional_ancestor(node) {
                    context.report(violation! {
                        node => node,
                        message_id => "unexpected",
                        data => {
                            type => NODE_DESCRIPTIONS.get(ancestor.kind()).copied().unwrap_or_else(|| ancestor.kind()),
                        }
                    });
                }
            },
            r#"[
              (do_statement
                condition: (parenthesized_expression
                  [
                    (assignment_expression)
                    (augmented_assignment_expression)
                  ]
                )
              )
              (for_statement
                condition: (expression_statement
                  [
                    (assignment_expression)
                    (augmented_assignment_expression)
                  ]
                )
              )
              (if_statement
                condition: (parenthesized_expression
                  [
                    (assignment_expression)
                    (augmented_assignment_expression)
                  ]
                )
              )
              (while_statement
                condition: (parenthesized_expression
                  [
                    (assignment_expression)
                    (augmented_assignment_expression)
                  ]
                )
              )
              (ternary_expression
                condition: [
                  (assignment_expression)
                  (augmented_assignment_expression)
                  (parenthesized_expression
                    [
                      (assignment_expression)
                      (augmented_assignment_expression)
                    ]
                  )
                ]
              )
            ] @c"# => |node, context| {
                if self.prohibit_assign != ProhibitAssign::ExceptParens {
                    return;
                }
                context.report(violation! {
                    node => skip_nodes_of_types(node.child_by_field_name("condition").unwrap(), &[ParenthesizedExpression, ExpressionStatement]),
                    message_id => "missing",
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    use crate::kind::{AssignmentExpression, AugmentedAssignmentExpression};

    #[test]
    fn test_no_cond_assign_rule() {
        RuleTester::run(
            no_cond_assign_rule(),
            rule_tests! {
                valid => [
                    "var x = 0; if (x == 0) { var b = 1; }",
                    { code => "var x = 0; if (x == 0) { var b = 1; }", options => "always" },
                    "var x = 5; while (x < 5) { x = x + 1; }",
                    "if ((someNode = someNode.parentNode) !== null) { }",
                    { code => "if ((someNode = someNode.parentNode) !== null) { }", options => "except-parens" },
                    "if ((a = b));",
                    "while ((a = b));",
                    "do {} while ((a = b));",
                    "for (;(a = b););",
                    "for (;;) {}",
                    "if (someNode || (someNode = parentNode)) { }",
                    "while (someNode || (someNode = parentNode)) { }",
                    "do { } while (someNode || (someNode = parentNode));",
                    "for (;someNode || (someNode = parentNode););",
                    { code => "if ((function(node) { return node = parentNode; })(someNode)) { }", options => "except-parens" },
                    { code => "if ((function(node) { return node = parentNode; })(someNode)) { }", options => "always" },
                    { code => "if ((node => node = parentNode)(someNode)) { }", options => "except-parens", /*parserOptions => { ecmaVersion => 6 }*/ },
                    { code => "if ((node => node = parentNode)(someNode)) { }", options => "always", /*parserOptions => { ecmaVersion => 6 }*/ },
                    { code => "if (function(node) { return node = parentNode; }) { }", options => "except-parens" },
                    { code => "if (function(node) { return node = parentNode; }) { }", options => "always" },
                    { code => "x = 0;", options => "always" },
                    "var x; var b = (x === 0) ? 1 : 0;",
                    { code => "switch (foo) { case a = b: bar(); }", options => "except-parens" },
                    { code => "switch (foo) { case a = b: bar(); }", options => "always" },
                    { code => "switch (foo) { case baz + (a = b): bar(); }", options => "always" }
                ],
                invalid => [
                    { code => "var x; if (x = 0) { var b = 1; }", errors => [{ message_id => "missing", type => AssignmentExpression, line => 1, column => 12, end_line => 1, end_column => 17 }] },
                    { code => "var x; while (x = 0) { var b = 1; }", errors => [{ message_id => "missing", type => AssignmentExpression }] },
                    { code => "var x = 0, y; do { y = x; } while (x = x + 1);", errors => [{ message_id => "missing", type => AssignmentExpression }] },
                    { code => "var x; for(; x+=1 ;){};", errors => [{ message_id => "missing", type => AugmentedAssignmentExpression }] },
                    { code => "var x; if ((x) = (0));", errors => [{ message_id => "missing", type => AssignmentExpression }] },
                    { code => "if (someNode || (someNode = parentNode)) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "an 'if' statement" }, type => AssignmentExpression, column => 18, end_column => 39 }] },
                    { code => "while (someNode || (someNode = parentNode)) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'while' statement" }, type => AssignmentExpression }] },
                    { code => "do { } while (someNode || (someNode = parentNode));", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'do...while' statement" }, type => AssignmentExpression }] },
                    { code => "for (; (typeof l === 'undefined' ? (l = 0) : l); i++) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'for' statement" }, type => AssignmentExpression }] },
                    { code => "if (x = 0) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "an 'if' statement" }, type => AssignmentExpression }] },
                    { code => "while (x = 0) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'while' statement" }, type => AssignmentExpression }] },
                    { code => "do { } while (x = x + 1);", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'do...while' statement" }, type => AssignmentExpression }] },
                    { code => "for(; x = y; ) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'for' statement" }, type => AssignmentExpression }] },
                    { code => "if ((x = 0)) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "an 'if' statement" }, type => AssignmentExpression }] },
                    { code => "while ((x = 0)) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'while' statement" }, type => AssignmentExpression }] },
                    { code => "do { } while ((x = x + 1));", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'do...while' statement" }, type => AssignmentExpression }] },
                    { code => "for(; (x = y); ) { }", options => "always", errors => [{ message_id => "unexpected", data => { type => "a 'for' statement" }, type => AssignmentExpression }] },
                    { code => "var x; var b = (x = 0) ? 1 : 0;", errors => [{ message_id => "missing", type => AssignmentExpression }] },
                    { code => "var x; var b = x && (y = 0) ? 1 : 0;", options => "always", errors => [{ message_id => "unexpected", type => AssignmentExpression }] },
                    { code => "(((3496.29)).bkufyydt = 2e308) ? foo : bar;", errors => [{ message_id => "missing", type => AssignmentExpression }] }
                ]
            },
        )
    }
}
