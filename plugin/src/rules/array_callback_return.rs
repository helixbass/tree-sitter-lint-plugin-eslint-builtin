use std::{borrow::Cow, sync::Arc};

use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{get_call_expression_arguments, is_logical_expression, NodeExtJs},
    kind::{
        Arguments, ArrowFunction, BinaryExpression, CallExpression, Function,
        ParenthesizedExpression, ReturnStatement, StatementBlock, TernaryExpression,
    },
    utils::ast_utils,
    CodePathAnalyzer, EnterOrExit,
};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_implicit: bool,
    check_for_each: bool,
}

static TARGET_NODE_TYPE: Lazy<Regex> =
    Lazy::new(|| Regex::new(&format!(r#"^(?:{ArrowFunction}|{Function})$"#)).unwrap());
static TARGET_METHODS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^(?:every|filter|find(?:Last)?(?:Index)?|flatMap|forEach|map|reduce(?:Right)?|some|sort|toSorted)$"#).unwrap()
});

fn is_target_method(node: Node, context: &QueryMatchContext) -> bool {
    ast_utils::is_specific_member_access(
        node,
        Option::<&'static str>::None,
        Some(&*TARGET_METHODS),
        context,
    )
}

fn get_array_method_name<'a>(
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<Cow<'a, str>> {
    let mut current_node = node;

    loop {
        let parent = current_node.parent().unwrap();

        match parent.kind() {
            BinaryExpression => {
                if !is_logical_expression(parent) {
                    return None;
                }
                current_node = parent;
            }
            TernaryExpression | ParenthesizedExpression => current_node = parent,
            ReturnStatement => {
                let func = ast_utils::get_upper_function(parent)
                    .filter(|&func| ast_utils::is_callee(func))?;

                current_node = func.maybe_next_non_parentheses_ancestor()?;
            }
            Arguments => {
                let call_expression = parent.parent().unwrap();
                if call_expression.kind() != CallExpression {
                    return None;
                }
                let callee = call_expression.field("function").skip_parentheses();
                if ast_utils::is_array_from_method(callee, context) {
                    let arguments = get_call_expression_arguments(call_expression)?.collect_vec();
                    if arguments.len() >= 2 && arguments[1] == current_node {
                        return Some("from".into());
                    }
                }
                if is_target_method(callee, context) {
                    let arguments = get_call_expression_arguments(call_expression)?.collect_vec();
                    if arguments.get(0).copied() == Some(current_node) {
                        return ast_utils::get_static_property_name(callee, context);
                    }
                }
                return None;
            }
            CallExpression => {
                let call_expression = parent;
                if call_expression.kind() != CallExpression {
                    return None;
                }
                let callee = call_expression.field("function").skip_parentheses();
                if ast_utils::is_array_from_method(callee, context) {
                    let arguments = get_call_expression_arguments(call_expression)?.collect_vec();
                    if arguments.len() >= 2 && arguments[1] == current_node {
                        return Some("from".into());
                    }
                }
                if is_target_method(callee, context) {
                    let arguments = get_call_expression_arguments(call_expression)?.collect_vec();
                    if arguments.get(0).copied() == Some(current_node) {
                        return ast_utils::get_static_property_name(callee, context);
                    }
                }
                return None;
            }
            _ => return None,
        }
    }
}

fn full_method_name(array_method_name: &str) -> String {
    match array_method_name {
        "from" | "of" | "isArray" => format!("Array.{array_method_name}"),
        _ => format!("Array.prototype.{array_method_name}"),
    }
}

pub fn array_callback_return_rule() -> Arc<dyn Rule> {
    rule! {
        name => "array-callback-return",
        languages => [Javascript],
        messages => [
            expected_at_end =>
                "{{array_method_name}}() expects a value to be returned at the end of {{name}}.",
            expected_inside => "{{array_method_name}}() expects a return value from {{name}}.",
            expected_return_value =>
                "{{array_method_name}}() expects a return value from {{name}}.",
            expected_no_return_value =>
                "{{array_method_name}}() expects no useless return value from {{name}}.",
        ],
        options_type => Options,
        state => {
            [per-config]
            allow_implicit: bool = options.allow_implicit,
            check_for_each: bool = options.check_for_each,
        },
        listeners => [
            "program:exit" => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                for (code_path, root_node, array_method_name) in code_path_analyzer
                    .code_paths
                    .iter()
                    .filter_map(|&code_path| {
                        let node = code_path_analyzer.code_path_arena[code_path]
                            .root_node(&code_path_analyzer.code_path_segment_arena);
                        if !TARGET_NODE_TYPE.is_match(node.kind()) {
                            return None;
                        }

                        let array_method_name = get_array_method_name(node, context)?;

                        if node.has_child_of_kind("async") {
                            return None;
                        }

                        Some((code_path, node, array_method_name))
                    })
                {
                    let mut has_return = false;

                    code_path_analyzer
                        .code_path_arena[code_path]
                        .traverse_segments_in_any_order(
                            &code_path_analyzer.code_path_segment_arena,
                            None,
                            |_, segment, _| {
                                for (_, return_statement_node) in code_path_analyzer
                                    .code_path_segment_arena[segment]
                                    .nodes
                                    .iter()
                                    .filter(|(enter_or_exit, node)| {
                                        *enter_or_exit == EnterOrExit::Enter &&
                                            node.kind() == ReturnStatement
                                    })
                                {
                                    has_return = true;

                                    let mut message_id = None;

                                    #[allow(clippy::collapsible_else_if)]
                                    if array_method_name == "forEach" {
                                        if self.check_for_each && return_statement_node.has_non_comment_named_children(context) {
                                            message_id = Some("expected_no_return_value");
                                        }
                                    } else {
                                        if !self.allow_implicit && !return_statement_node.has_non_comment_named_children(context) {
                                            message_id = Some("expected_return_value");
                                        }
                                    }

                                    if let Some(message_id) = message_id {
                                        context.report(violation! {
                                            node => *return_statement_node,
                                            message_id => message_id,
                                            data => {
                                                name => ast_utils::get_function_name_with_kind(root_node, context),
                                                array_method_name => full_method_name(&array_method_name),
                                            },
                                        });
                                    }
                                }
                            }
                        );

                    let mut message_id = None;

                    #[allow(clippy::collapsible_else_if)]
                    if array_method_name == "forEach" {
                        if self.check_for_each
                            && root_node.kind() == ArrowFunction
                            && root_node.field("body").kind() != StatementBlock
                        {
                            message_id = Some("expected_no_return_value");
                        }
                    } else {
                        if root_node.field("body").kind() == StatementBlock
                            && code_path_analyzer.code_path_arena[code_path]
                                .state
                                .head_segments(&code_path_analyzer.fork_context_arena)
                                .reachable(&code_path_analyzer.code_path_segment_arena)
                        {
                            message_id = Some(if has_return {
                                "expected_at_end"
                            } else {
                                "expected_inside"
                            });
                        }
                    }

                    if let Some(message_id) = message_id {
                        let name = ast_utils::get_function_name_with_kind(root_node, context);

                        context.report(violation! {
                            node => root_node,
                            range => ast_utils::get_function_head_range(root_node),
                            message_id => message_id,
                            data => {
                                name => name,
                                array_method_name => full_method_name(&array_method_name),
                            },
                        });
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, serde_json::json, RuleTester};

    use super::*;
    use crate::get_instance_provider_factory;

    #[test]
    fn test_array_callback_return_rule() {
        let allow_implicit_options = json!({ "allow_implicit": true });

        let check_for_each_options = json!({ "check_for_each": true });

        let allow_implicit_check_for_each =
            json!({ "allow_implicit": true, "check_for_each": true });

        RuleTester::run_with_from_file_run_context_instance_provider(
            array_callback_return_rule(),
            rule_tests! {
                valid => [
                    "foo.every(function(){}())",
                    "foo.every(function(){ return function() { return true; }; }())",
                    "foo.every(function(){ return function() { return; }; })",

                    "foo.forEach(bar || function(x) { var a=0; })",
                    "foo.forEach(bar || function(x) { return a; })",
                    "foo.forEach(function() {return function() { var a = 0;}}())",
                    "foo.forEach(function(x) { var a=0; })",
                    "foo.forEach(function(x) { return a;})",
                    "foo.forEach(function(x) { return; })",
                    "foo.forEach(function(x) { if (a === b) { return;} var a=0; })",
                    "foo.forEach(function(x) { if (a === b) { return x;} var a=0; })",
                    "foo.bar().forEach(function(x) { return; })",
                    "[\"foo\",\"bar\",\"baz\"].forEach(function(x) { return x; })",
                    { code => "foo.forEach(x => { var a=0; })", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.forEach(x => { if (a === b) { return;} var a=0; })", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.forEach(x => x)", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.forEach(val => y += val)", /*parserOptions: { ecmaVersion: 6 }*/ },

                    { code => "foo.map(async function(){})", /*parserOptions: { ecmaVersion: 8 }*/ },
                    { code => "foo.map(async () => {})", /*parserOptions: { ecmaVersion: 8 }*/ },
                    { code => "foo.map(function* () {})", /*parserOptions: { ecmaVersion: 6 }*/ },

                    // options => { allow_implicit => false }
                    { code => "Array.from(x, function() { return true; })", options => { allow_implicit => false } },
                    { code => "Int32Array.from(x, function() { return true; })", options => { allow_implicit => false } },
                    "foo.every(function() { return true; })",
                    "foo.filter(function() { return true; })",
                    "foo.find(function() { return true; })",
                    "foo.findIndex(function() { return true; })",
                    "foo.findLast(function() { return true; })",
                    "foo.findLastIndex(function() { return true; })",
                    "foo.flatMap(function() { return true; })",
                    "foo.forEach(function() { return; })",
                    "foo.map(function() { return true; })",
                    "foo.reduce(function() { return true; })",
                    "foo.reduceRight(function() { return true; })",
                    "foo.some(function() { return true; })",
                    "foo.sort(function() { return 0; })",
                    "foo.toSorted(function() { return 0; })",
                    { code => "foo.every(() => { return true; })", /*parserOptions: { ecmaVersion: 6 }*/ },
                    "foo.every(function() { if (a) return true; else return false; })",
                    "foo.every(function() { switch (a) { case 0: bar(); default: return true; } })",
                    "foo.every(function() { try { bar(); return true; } catch (err) { return false; } })",
                    "foo.every(function() { try { bar(); } finally { return true; } })",

                    // options => { allow_implicit => true }
                    { code => "Array.from(x, function() { return; })", options => allow_implicit_options },
                    { code => "Int32Array.from(x, function() { return; })", options => allow_implicit_options },
                    { code => "foo.every(function() { return; })", options => allow_implicit_options },
                    { code => "foo.filter(function() { return; })", options => allow_implicit_options },
                    { code => "foo.find(function() { return; })", options => allow_implicit_options },
                    { code => "foo.findIndex(function() { return; })", options => allow_implicit_options },
                    { code => "foo.findLast(function() { return; })", options => allow_implicit_options },
                    { code => "foo.findLastIndex(function() { return; })", options => allow_implicit_options },
                    { code => "foo.flatMap(function() { return; })", options => allow_implicit_options },
                    { code => "foo.forEach(function() { return; })", options => allow_implicit_options },
                    { code => "foo.map(function() { return; })", options => allow_implicit_options },
                    { code => "foo.reduce(function() { return; })", options => allow_implicit_options },
                    { code => "foo.reduceRight(function() { return; })", options => allow_implicit_options },
                    { code => "foo.some(function() { return; })", options => allow_implicit_options },
                    { code => "foo.sort(function() { return; })", options => allow_implicit_options },
                    { code => "foo.toSorted(function() { return; })", options => allow_implicit_options },
                    { code => "foo.every(() => { return; })", options => allow_implicit_options, /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.every(function() { if (a) return; else return a; })", options => allow_implicit_options },
                    { code => "foo.every(function() { switch (a) { case 0: bar(); default: return; } })", options => allow_implicit_options },
                    { code => "foo.every(function() { try { bar(); return; } catch (err) { return; } })", options => allow_implicit_options },
                    { code => "foo.every(function() { try { bar(); } finally { return; } })", options => allow_implicit_options },

                    // options => { checkForEach: true }
                    { code => "foo.forEach(function(x) { return; })", options => check_for_each_options },
                    { code => "foo.forEach(function(x) { var a=0; })", options => check_for_each_options },
                    { code => "foo.forEach(function(x) { if (a === b) { return;} var a=0; })", options => check_for_each_options },
                    { code => "foo.forEach(function() {return function() { if (a == b) { return; }}}())", options => check_for_each_options },
                    { code => "foo.forEach(x => { var a=0; })", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.forEach(x => { if (a === b) { return;} var a=0; })", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.forEach(x => { x })", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.forEach(bar || function(x) { return; })", options => check_for_each_options },
                    { code => "Array.from(x, function() { return true; })", options => check_for_each_options },
                    { code => "Int32Array.from(x, function() { return true; })", options => check_for_each_options },
                    { code => "foo.every(() => { return true; })", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.every(function() { if (a) return 1; else return a; })", options => check_for_each_options },
                    { code => "foo.every(function() { switch (a) { case 0: return bar(); default: return a; } })", options => check_for_each_options },
                    { code => "foo.every(function() { try { bar(); return 1; } catch (err) { return err; } })", options => check_for_each_options },
                    { code => "foo.every(function() { try { bar(); } finally { return 1; } })", options => check_for_each_options },
                    { code => "foo.every(function() { return; })", options => allow_implicit_check_for_each },

                    "Arrow.from(x, function() {})",
                    "foo.abc(function() {})",
                    "every(function() {})",
                    "foo[every](function() {})",
                    "var every = function() {}",
                    { code => "foo[`${every}`](function() {})", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "foo.every(() => true)", /*parserOptions: { ecmaVersion: 6 }*/ }
                ],
                invalid => [
                    { code => "Array.from(x, function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.from" } }] },
                    { code => "Array.from(x, function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.from" } }] },
                    { code => "Int32Array.from(x, function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.from" } }] },
                    { code => "Int32Array.from(x, function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.from" } }] },
                    { code => "foo.every(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.filter(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.filter" } }] },
                    { code => "foo.filter(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.filter" } }] },
                    { code => "foo.find(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.find" } }] },
                    { code => "foo.find(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.find" } }] },
                    { code => "foo.findLast(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.findLast" } }] },
                    { code => "foo.findLast(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.findLast" } }] },
                    { code => "foo.findIndex(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.findIndex" } }] },
                    { code => "foo.findIndex(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.findIndex" } }] },
                    { code => "foo.findLastIndex(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.findLastIndex" } }] },
                    { code => "foo.findLastIndex(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.findLastIndex" } }] },
                    { code => "foo.flatMap(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.flatMap" } }] },
                    { code => "foo.flatMap(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.flatMap" } }] },
                    { code => "foo.map(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.map" } }] },
                    { code => "foo.map(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.map" } }] },
                    { code => "foo.reduce(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.reduce" } }] },
                    { code => "foo.reduce(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.reduce" } }] },
                    { code => "foo.reduceRight(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.reduceRight" } }] },
                    { code => "foo.reduceRight(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.reduceRight" } }] },
                    { code => "foo.some(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.some" } }] },
                    { code => "foo.some(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.some" } }] },
                    { code => "foo.sort(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.sort" } }] },
                    { code => "foo.sort(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.sort" } }] },
                    { code => "foo.toSorted(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.toSorted" } }] },
                    { code => "foo.toSorted(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.toSorted" } }] },
                    { code => "foo.bar.baz.every(function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.bar.baz.every(function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo[\"every\"](function() {})", errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo[\"every\"](function foo() {})", errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo[`every`](function() {})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo[`every`](function foo() {})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(() => {})", /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message => "Array.prototype.every() expects a return value from arrow function.", column => 14 }] },
                    { code => "foo.every(function() { if (a) return true; })", errors => [{ message => "Array.prototype.every() expects a value to be returned at the end of function.", column => 11 }] },
                    { code => "foo.every(function cb() { if (a) return true; })", errors => [{ message => "Array.prototype.every() expects a value to be returned at the end of function 'cb'.", column => 11 }] },
                    { code => "foo.every(function() { switch (a) { case 0: break; default: return true; } })", errors => [{ message_id => "expected_at_end", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function foo() { switch (a) { case 0: break; default: return true; } })", errors => [{ message_id => "expected_at_end", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function() { try { bar(); } catch (err) { return true; } })", errors => [{ message_id => "expected_at_end", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function foo() { try { bar(); } catch (err) { return true; } })", errors => [{ message_id => "expected_at_end", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function() { return; })", errors => [{ message_id => "expected_return_value", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function foo() { return; })", errors => [{ message_id => "expected_return_value", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function() { if (a) return; })", errors => ["Array.prototype.every() expects a value to be returned at the end of function.", { message_id => "expected_return_value", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function foo() { if (a) return; })", errors => ["Array.prototype.every() expects a value to be returned at the end of function 'foo'.", { message_id => "expected_return_value", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function() { if (a) return; else return; })", errors => [{ message_id => "expected_return_value", data => { name => "function", array_method_name => "Array.prototype.every" } }, { message_id => "expected_return_value", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(function foo() { if (a) return; else return; })", errors => [{ message_id => "expected_return_value", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }, { message_id => "expected_return_value", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(cb || function() {})", errors => ["Array.prototype.every() expects a return value from function."] },
                    { code => "foo.every(cb || function foo() {})", errors => ["Array.prototype.every() expects a return value from function 'foo'."] },
                    { code => "foo.every(a ? function() {} : function() {})", errors => ["Array.prototype.every() expects a return value from function.", "Array.prototype.every() expects a return value from function."] },
                    { code => "foo.every(a ? function foo() {} : function bar() {})", errors => ["Array.prototype.every() expects a return value from function 'foo'.", "Array.prototype.every() expects a return value from function 'bar'."] },
                    { code => "foo.every(function(){ return function() {}; }())", errors => [{ message => "Array.prototype.every() expects a return value from function.", column => 30 }] },
                    { code => "foo.every(function(){ return function foo() {}; }())", errors => [{ message => "Array.prototype.every() expects a return value from function 'foo'.", column => 30 }] },
                    { code => "foo.every(() => {})", options => { allow_implicit => false }, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message => "Array.prototype.every() expects a return value from arrow function." }] },
                    { code => "foo.every(() => {})", options => { allow_implicit => true }, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message => "Array.prototype.every() expects a return value from arrow function." }] },

                    // options => { allow_implicit => true }
                    { code => "Array.from(x, function() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.from" } }] },
                    { code => "foo.every(function() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.filter(function foo() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.filter" } }] },
                    { code => "foo.find(function foo() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.find" } }] },
                    { code => "foo.map(function() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.map" } }] },
                    { code => "foo.reduce(function() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.reduce" } }] },
                    { code => "foo.reduceRight(function() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.reduceRight" } }] },
                    { code => "foo.bar.baz.every(function foo() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.every(cb || function() {})", options => allow_implicit_options, errors => ["Array.prototype.every() expects a return value from function."] },
                    { code => "[\"foo\",\"bar\"].sort(function foo() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.sort" } }] },
                    { code => "[\"foo\",\"bar\"].toSorted(function foo() {})", options => allow_implicit_options, errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.toSorted" } }] },
                    { code => "foo.forEach(x => x)", options => allow_implicit_check_for_each, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_no_return_value", data => { name => "arrow function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(function(x) { if (a == b) {return x;}})", options => allow_implicit_check_for_each, errors => [{ message_id => "expected_no_return_value", data => { name => "function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(function bar(x) { return x;})", options => allow_implicit_check_for_each, errors => [{ message_id => "expected_no_return_value", data => { name => "function 'bar'", array_method_name => "Array.prototype.forEach" } }] },

                    // // options => { checkForEach: true }
                    { code => "foo.forEach(x => x)", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_no_return_value", data => { name => "arrow function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(val => y += val)", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_no_return_value", data => { name => "arrow function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "[\"foo\",\"bar\"].forEach(x => ++x)", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_no_return_value", data => { name => "arrow function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.bar().forEach(x => x === y)", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_no_return_value", data => { name => "arrow function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(function() {return function() { if (a == b) { return a; }}}())", options => check_for_each_options, errors => [{ message_id => "expected_no_return_value", data => { name => "function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(function(x) { if (a == b) {return x;}})", options => check_for_each_options, errors => [{ message_id => "expected_no_return_value", data => { name => "function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(function(x) { if (a == b) {return undefined;}})", options => check_for_each_options, errors => [{ message_id => "expected_no_return_value", data => { name => "function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(function bar(x) { return x;})", options => check_for_each_options, errors => [{ message_id => "expected_no_return_value", data => { name => "function 'bar'", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach(function bar(x) { return x;})", options => check_for_each_options, errors => ["Array.prototype.forEach() expects no useless return value from function 'bar'."] },
                    { code => "foo.bar().forEach(function bar(x) { return x;})", options => check_for_each_options, errors => [{ message_id => "expected_no_return_value", data => { name => "function 'bar'", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "[\"foo\",\"bar\"].forEach(function bar(x) { return x;})", options => check_for_each_options, errors => [{ message_id => "expected_no_return_value", data => { name => "function 'bar'", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "foo.forEach((x) => { return x;})", options => check_for_each_options, /*parserOptions: { ecmaVersion: 6 },*/ errors => [{ message_id => "expected_no_return_value", data => { name => "arrow function", array_method_name => "Array.prototype.forEach" } }] },
                    { code => "Array.from(x, function() {})", options => check_for_each_options, errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.from" } }] },
                    { code => "foo.every(function() {})", options => check_for_each_options, errors => [{ message_id => "expected_inside", data => { name => "function", array_method_name => "Array.prototype.every" } }] },
                    { code => "foo.filter(function foo() {})", options => check_for_each_options, errors => [{ message_id => "expected_inside", data => { name => "function 'foo'", array_method_name => "Array.prototype.filter" } }] },
                    { code => "foo.filter(function foo() { return; })", options => check_for_each_options, errors => [{ message_id => "expected_return_value", data => { name => "function 'foo'", array_method_name => "Array.prototype.filter" } }] },
                    { code => "foo.every(cb || function() {})", options => check_for_each_options, errors => ["Array.prototype.every() expects a return value from function."] },

                    // full location tests
                    {
                        code => "foo.filter(bar => { baz(); } )",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "arrow function", array_method_name => "Array.prototype.filter" },
                            type => ArrowFunction,
                            line => 1,
                            column => 16,
                            end_line => 1,
                            end_column => 18
                        }]
                    },
                    {
                        code => "foo.filter(\n() => {} )",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "arrow function", array_method_name => "Array.prototype.filter" },
                            type => ArrowFunction,
                            line => 2,
                            column => 4,
                            end_line => 2,
                            end_column => 6
                        }]
                    },
                    {
                        code => "foo.filter(bar || ((baz) => {}) )",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "arrow function", array_method_name => "Array.prototype.filter" },
                            type => ArrowFunction,
                            line => 1,
                            column => 26,
                            end_line => 1,
                            end_column => 28
                        }]
                    },
                    {
                        code => "foo.filter(bar => { return; })",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_return_value",
                            data => { name => "arrow function", array_method_name => "Array.prototype.filter" },
                            type => ReturnStatement,
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 28
                        }]
                    },
                    {
                        code => "Array.from(foo, bar => { bar })",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "arrow function", array_method_name => "Array.from" },
                            type => ArrowFunction,
                            line => 1,
                            column => 21,
                            end_line => 1,
                            end_column => 23
                        }]
                    },
                    {
                        code => "foo.forEach(bar => bar)",
                        options => check_for_each_options,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_no_return_value",
                            data => { name => "arrow function", array_method_name => "Array.prototype.forEach" },
                            type => ArrowFunction,
                            line => 1,
                            column => 17,
                            end_line => 1,
                            end_column => 19
                        }]
                    },
                    {
                        code => "foo.forEach((function () { return (bar) => bar; })())",
                        options => check_for_each_options,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_no_return_value",
                            data => { name => "arrow function", array_method_name => "Array.prototype.forEach" },
                            type => ArrowFunction,
                            line => 1,
                            column => 41,
                            end_line => 1,
                            end_column => 43
                        }],
                    },
                    {
                        code => "foo.forEach((() => {\n return bar => bar; })())",
                        options => check_for_each_options,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_no_return_value",
                            data => { name => "arrow function", array_method_name => "Array.prototype.forEach" },
                            type => ArrowFunction,
                            line => 2,
                            column => 13,
                            end_line => 2,
                            end_column => 15
                        }]
                    },
                    {
                        code => "foo.forEach((bar) => { if (bar) { return; } else { return bar ; } })",
                        options => check_for_each_options,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "expected_no_return_value",
                            data => { name => "arrow function", array_method_name => "Array.prototype.forEach" },
                            type => ReturnStatement,
                            line => 1,
                            column => 52,
                            end_line => 1,
                            end_column => 64
                        }]
                    },
                    {
                        code => "foo.filter(function(){})",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function", array_method_name => "Array.prototype.filter" },
                            type => Function,
                            line => 1,
                            column => 12,
                            end_line => 1,
                            end_column => 20
                        }]
                    },
                    {
                        code => "foo.filter(function (){})",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function", array_method_name => "Array.prototype.filter" },
                            type => Function,
                            line => 1,
                            column => 12,
                            end_line => 1,
                            end_column => 21
                        }]
                    },
                    {
                        code => "foo.filter(function\n(){})",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function", array_method_name => "Array.prototype.filter" },
                            type => Function,
                            line => 1,
                            column => 12,
                            end_line => 2,
                            end_column => 1
                        }]
                    },
                    {
                        code => "foo.filter(function bar(){})",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function 'bar'", array_method_name => "Array.prototype.filter" },
                            type => Function,
                            line => 1,
                            column => 12,
                            end_line => 1,
                            end_column => 24
                        }]
                    },
                    {
                        code => "foo.filter(function bar  (){})",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function 'bar'", array_method_name => "Array.prototype.filter" },
                            type => Function,
                            line => 1,
                            column => 12,
                            end_line => 1,
                            end_column => 26
                        }]
                    },
                    {
                        code => "foo.filter(function\n bar() {})",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function 'bar'", array_method_name => "Array.prototype.filter" },
                            type => Function,
                            line => 1,
                            column => 12,
                            end_line => 2,
                            end_column => 5
                        }]
                    },
                    {
                        code => "Array.from(foo, function bar(){})",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function 'bar'", array_method_name => "Array.from" },
                            type => Function,
                            line => 1,
                            column => 17,
                            end_line => 1,
                            end_column => 29
                        }]
                    },
                    {
                        code => "Array.from(foo, bar ? function (){} : baz)",
                        errors => [{
                            message_id => "expected_inside",
                            data => { name => "function", array_method_name => "Array.from" },
                            type => Function,
                            line => 1,
                            column => 23,
                            end_line => 1,
                            end_column => 32
                        }]
                    },
                    {
                        code => "foo.filter(function bar() { return \n })",
                        errors => [{
                            message_id => "expected_return_value",
                            data => { name => "function 'bar'", array_method_name => "Array.prototype.filter" },
                            type => ReturnStatement,
                            line => 1,
                            column => 29,
                            end_line => 1,
                            end_column => 35
                        }]
                    },
                    {
                        code => "foo.forEach(function () { \nif (baz) return bar\nelse return\n })",
                        options => check_for_each_options,
                        errors => [{
                            message_id => "expected_no_return_value",
                            data => { name => "function", array_method_name => "Array.prototype.forEach" },
                            type => ReturnStatement,
                            line => 2,
                            column => 10,
                            end_line => 2,
                            end_column => 20
                        }]
                    },

                    // Optional chaining
                    {
                        code => "foo?.filter(() => { console.log('hello') })",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected_inside", data => { name => "arrow function", array_method_name => "Array.prototype.filter" } }]
                    },
                    {
                        code => "(foo?.filter)(() => { console.log('hello') })",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected_inside", data => { name => "arrow function", array_method_name => "Array.prototype.filter" } }]
                    },
                    {
                        code => "Array?.from([], () => { console.log('hello') })",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected_inside", data => { name => "arrow function", array_method_name => "Array.from" } }]
                    },
                    {
                        code => "(Array?.from)([], () => { console.log('hello') })",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected_inside", data => { name => "arrow function", array_method_name => "Array.from" } }]
                    },
                    {
                        code => "foo?.filter((function() { return () => { console.log('hello') } })?.())",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected_inside", data => { name => "arrow function", array_method_name => "Array.prototype.filter" } }]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
