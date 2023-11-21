use std::{collections::HashSet, sync::Arc};

use itertools::Itertools;
use once_cell::sync::Lazy;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    assert_kind,
    ast_helpers::{
        get_call_expression_arguments, get_comma_separated_optional_non_comment_named_children,
        get_last_expression_of_sequence_expression, is_logical_expression, NodeExtJs,
    },
    conf::globals::BUILTIN,
    kind::{
        self, is_literal_kind, Array, ArrowFunction, AssignmentExpression,
        AugmentedAssignmentExpression, BinaryExpression, CallExpression, Class, False, Function,
        Identifier, NewExpression, Object, SequenceExpression, SpreadElement, TemplateString,
        TemplateSubstitution, TernaryExpression, True, UnaryExpression, Undefined,
        UpdateExpression,
    },
    scope::{Scope, ScopeManager},
    utils::ast_utils::{
        is_constant, is_logical_assignment_operator, is_null_literal,
        is_reference_to_global_variable,
    },
};

static NUMERIC_OR_STRING_BINARY_OPERATORS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "+", "-", "*", "/", "%", "|", "^", "&", "**", "<<", ">>", ">>>",
    ]
    .into_iter()
    .collect()
});

fn is_null_or_undefined(scope: &Scope, node: Node) -> bool {
    is_null_literal(node)
        || node.kind() == Undefined && is_reference_to_global_variable(scope, node)
        || node.kind() == UnaryExpression && node.field("operator").kind() == "void"
}

fn has_constant_nullishness(
    scope: &Scope,
    node: Node,
    non_nullish: bool,
    context: &QueryMatchContext,
) -> bool {
    if non_nullish && is_null_or_undefined(scope, node) {
        return false;
    }

    match node.kind() {
        Object | Array | ArrowFunction | Function | Class | NewExpression | TemplateString
        | UpdateExpression => true,
        kind if is_literal_kind(kind) => true,
        CallExpression => {
            let callee = node.field("function");
            if callee.kind() != Identifier {
                return false;
            }
            let function_name = callee.text(context);

            matches!(&*function_name, "Boolean" | "String" | "Number")
                && is_reference_to_global_variable(scope, callee)
        }
        BinaryExpression => {
            if !is_logical_expression(node) {
                return true;
            }
            node.field("operator").kind() == "??"
                && has_constant_nullishness(scope, node.field("right"), true, context)
        }
        AssignmentExpression => {
            has_constant_nullishness(scope, node.field("right"), non_nullish, context)
        }
        AugmentedAssignmentExpression => {
            if is_logical_assignment_operator(&node.field("operator").text(context)) {
                return false;
            }

            true
        }
        UnaryExpression => true,
        SequenceExpression => has_constant_nullishness(
            scope,
            get_last_expression_of_sequence_expression(node),
            non_nullish,
            context,
        ),
        Undefined => is_reference_to_global_variable(scope, node),
        _ => false,
    }
}

fn is_boolean_global_call_with_no_or_constant_argument(
    scope: &Scope,
    node: Node,
    context: &QueryMatchContext,
) -> bool {
    assert_kind!(node, CallExpression);

    let callee = node.field("function");
    callee.kind() == Identifier
        && callee.text(context) == "Boolean"
        && is_reference_to_global_variable(scope, callee)
        && get_call_expression_arguments(node).matches(|mut arguments| match arguments.next() {
            None => true,
            Some(first_argument) => is_constant(scope, first_argument, true, context),
        })
}

fn is_static_boolean(scope: &Scope, node: Node, context: &QueryMatchContext) -> bool {
    match node.kind() {
        True | False => true,
        CallExpression => is_boolean_global_call_with_no_or_constant_argument(scope, node, context),
        UnaryExpression => {
            node.field("operator").kind() == "!"
                && is_constant(scope, node.field("argument"), true, context)
        }
        _ => false,
    }
}

fn has_constant_loose_boolean_comparison(
    scope: &Scope,
    node: Node,
    context: &QueryMatchContext,
) -> bool {
    match node.kind() {
        Object | Class => true,
        Array => {
            let elements =
                get_comma_separated_optional_non_comment_named_children(node).collect_vec();
            if elements.is_empty() {
                return true;
            }
            elements
                .iter()
                .filter(|e| e.matches(|e| e.kind() != SpreadElement))
                .count()
                > 1
        }
        ArrowFunction | Function => true,
        UnaryExpression => match node.field("operator").kind() {
            "void" | "typeof" => true,
            "!" => is_constant(scope, node.field("argument"), true, context),
            _ => false,
        },
        NewExpression => false,
        CallExpression => is_boolean_global_call_with_no_or_constant_argument(scope, node, context),
        kind if is_literal_kind(kind) => true,
        Undefined => is_reference_to_global_variable(scope, node),
        TemplateString => node.children_of_kind(TemplateSubstitution).next().is_none(),
        AssignmentExpression => {
            has_constant_loose_boolean_comparison(scope, node.field("right"), context)
        }
        SequenceExpression => has_constant_loose_boolean_comparison(
            scope,
            get_last_expression_of_sequence_expression(node),
            context,
        ),
        _ => false,
    }
}

fn has_constant_strict_boolean_comparison(
    scope: &Scope,
    node: Node,
    context: &QueryMatchContext,
) -> bool {
    match node.kind() {
        Object | Array | ArrowFunction | Function | Class | NewExpression | TemplateString
        | UpdateExpression => true,
        kind if is_literal_kind(kind) => true,
        BinaryExpression => {
            NUMERIC_OR_STRING_BINARY_OPERATORS.contains(&node.field("operator").kind())
        }
        UnaryExpression => match node.field("operator").kind() {
            "delete" => false,
            "!" => is_constant(scope, node.field("argument"), true, context),
            _ => true,
        },
        SequenceExpression => has_constant_strict_boolean_comparison(
            scope,
            get_last_expression_of_sequence_expression(node),
            context,
        ),
        Undefined => is_reference_to_global_variable(scope, node),
        AssignmentExpression => {
            has_constant_strict_boolean_comparison(scope, node.field("right"), context)
        }
        AugmentedAssignmentExpression => {
            !is_logical_assignment_operator(node.field("operator").kind())
        }
        CallExpression => {
            if is_boolean_global_call_with_no_or_constant_argument(scope, node, context) {
                return true;
            }
            let callee = node.field("function");
            if callee.kind() != Identifier {
                return false;
            }
            let function_name = callee.text(context);

            if matches!(&*function_name, "String" | "Number")
                && is_reference_to_global_variable(scope, callee)
            {
                return true;
            }
            false
        }
        _ => false,
    }
}

fn is_always_new(scope: &Scope, node: Node, context: &QueryMatchContext) -> bool {
    match node.kind() {
        Object | Array | ArrowFunction | Function | Class => true,
        NewExpression => {
            let callee = node.field("constructor");
            if callee.kind() != Identifier {
                return false;
            }

            BUILTIN.contains_key(&callee.text(context))
                && is_reference_to_global_variable(scope, callee)
        }
        kind::Regex => true,
        SequenceExpression => is_always_new(
            scope,
            get_last_expression_of_sequence_expression(node),
            context,
        ),
        AssignmentExpression => is_always_new(scope, node.field("right"), context),
        TernaryExpression => {
            is_always_new(scope, node.field("consequence"), context)
                && is_always_new(scope, node.field("alternative"), context)
        }
        _ => false,
    }
}

fn find_binary_expression_constant_operand<'a>(
    scope: &Scope<'a, '_>,
    a: Node<'a>,
    b: Node<'a>,
    operator: &str,
    context: &QueryMatchContext,
) -> Option<Node<'a>> {
    match operator {
        "==" | "!=" => {
            if is_null_or_undefined(scope, a) && has_constant_nullishness(scope, b, false, context)
                || is_static_boolean(scope, a, context)
                    && has_constant_loose_boolean_comparison(scope, b, context)
            {
                return Some(b);
            }
        }
        "===" | "!==" => {
            if is_null_or_undefined(scope, a) && has_constant_nullishness(scope, b, false, context)
                || is_static_boolean(scope, a, context)
                    && has_constant_strict_boolean_comparison(scope, b, context)
            {
                return Some(b);
            }
        }
        _ => (),
    }
    None
}

pub fn no_constant_binary_expression_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-constant-binary-expression",
        languages => [Javascript],
        messages => [
            constant_binary_operand => "Unexpected constant binary expression. Compares constantly with the {{other_side}}-hand side of the `{{operator}}`.",
            constant_short_circuit => "Unexpected constant {{property}} on the left-hand side of a `{{operator}}` expression.",
            always_new => "Unexpected comparison to newly constructed object. These two values can never be equal.",
            both_always_new => "Unexpected comparison of two newly constructed objects. These two values can never be equal.",
        ],
        listeners => [
            r#"
              (binary_expression) @c
            "# => |node, context| {
                if is_logical_expression(node) {
                    let operator = node.field("operator").kind();
                    let left = node.field("left").skip_parentheses();
                    let scope_manager = context.retrieve::<ScopeManager<'a>>();
                    let scope = scope_manager.get_scope(node);

                    match operator {
                        "&&" | "||" => {
                            if is_constant(&scope, left, true, context) {
                                context.report(violation! {
                                    node => left,
                                    message_id => "constant_short_circuit",
                                    data => {
                                        property => "truthiness",
                                        operator => operator,
                                    }
                                });
                            }
                        }
                        "??" => {
                            if has_constant_nullishness(&scope, left, false, context) {
                                context.report(violation! {
                                    node => left,
                                    message_id => "constant_short_circuit",
                                    data => {
                                        property => "nullishness",
                                        operator => operator,
                                    }
                                });
                            }
                        }
                        _ => unreachable!()
                    }
                } else {
                    let scope_manager = context.retrieve::<ScopeManager<'a>>();
                    let scope = scope_manager.get_scope(node);
                    let right = node.field("right").skip_parentheses();
                    let left = node.field("left").skip_parentheses();
                    let operator = node.field("operator").kind();
                    let right_constant_operand = find_binary_expression_constant_operand(&scope, left, right, operator, context);
                    let left_constant_operand = find_binary_expression_constant_operand(&scope, right, left, operator, context);

                    if let Some(right_constant_operand) = right_constant_operand {
                        context.report(violation! {
                            node => right_constant_operand,
                            message_id => "constant_binary_operand",
                            data => {
                                operator => operator,
                                other_side => "left",
                            }
                        });
                    } else if let Some(left_constant_operand) = left_constant_operand {
                        context.report(violation! {
                            node => left_constant_operand,
                            message_id => "constant_binary_operand",
                            data => {
                                operator => operator,
                                other_side => "right",
                            }
                        });
                    } else {
                        match operator {
                            "===" | "!==" => {
                                if is_always_new(&scope, left, context) {
                                    context.report(violation! {
                                        node => left,
                                        message_id => "always_new",
                                    });
                                } else if is_always_new(&scope, right, context) {
                                    context.report(violation! {
                                        node => right,
                                        message_id => "always_new",
                                    });
                                }
                            }
                            "==" | "!=" => {
                                if is_always_new(&scope, left, context) && is_always_new(&scope, right, context) {
                                    context.report(violation! {
                                        node => left,
                                        message_id => "both_always_new",
                                    });
                                }
                            }
                            _ => ()
                        }
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use squalid::json_object;
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{get_instance_provider_factory, tests::helpers::tracing_subscribe};

    #[test]
    fn test_no_constant_binary_expression_rule() {
        tracing_subscribe();

        RuleTester::run_with_instance_provider_and_environment(
            no_constant_binary_expression_rule(),
            rule_tests! {
                valid => [
                    // While this _would_ be a constant condition in React, ESLint has a policy of not attributing any specific behavior to JSX.
                    "<p /> && foo",
                    "<></> && foo",
                    "<p /> ?? foo",
                    "<></> ?? foo",
                    "arbitraryFunction(n) ?? foo",
                    "foo.Boolean(n) ?? foo",
                    "(x += 1) && foo",
                    "`${bar}` && foo",
                    "bar && foo",
                    "delete bar.baz && foo",
                    "true ? foo : bar", // We leave ConditionalExpression for `no-constant-condition`.
                    "new Foo() == true",
                    "foo == true",
                    "`${foo}` == true",
                    "`${foo}${bar}` == true",
                    "`0${foo}` == true",
                    "`00000000${foo}` == true",
                    "`0${foo}.000` == true",
                    "[n] == true",

                    "delete bar.baz === true",

                    "foo.Boolean(true) && foo",
                    "function Boolean(n) { return n; }; Boolean(x) ?? foo",
                    "function String(n) { return n; }; String(x) ?? foo",
                    "function Number(n) { return n; }; Number(x) ?? foo",
                    "function Boolean(n) { return Math.random(); }; Boolean(x) === 1",
                    "function Boolean(n) { return Math.random(); }; Boolean(1) == true",

                    "new Foo() === x",
                    "x === new someObj.Promise()",
                    "Boolean(foo) === true",
                    "function foo(undefined) { undefined ?? bar;}",
                    "function foo(undefined) { undefined == true;}",
                    "function foo(undefined) { undefined === true;}",
                    "[...arr, 1] == true",
                    "[,,,] == true",
                    { code => "new Foo() === bar;", environment => { globals => { Foo => "writable" } } },
                    "(foo && true) ?? bar",
                    "foo ?? null ?? bar",
                    "a ?? (doSomething(), undefined) ?? b",
                    "a ?? (something = null) ?? b"
                ],
                invalid => [
                    // Error messages
                    { code => "[] && greeting", errors => [{ message => "Unexpected constant truthiness on the left-hand side of a `&&` expression." }] },
                    { code => "[] || greeting", errors => [{ message => "Unexpected constant truthiness on the left-hand side of a `||` expression." }] },
                    { code => "[] ?? greeting", errors => [{ message => "Unexpected constant nullishness on the left-hand side of a `??` expression." }] },
                    { code => "[] == true", errors => [{ message => "Unexpected constant binary expression. Compares constantly with the right-hand side of the `==`." }] },
                    { code => "true == []", errors => [{ message => "Unexpected constant binary expression. Compares constantly with the left-hand side of the `==`." }] },
                    { code => "[] != true", errors => [{ message => "Unexpected constant binary expression. Compares constantly with the right-hand side of the `!=`." }] },
                    { code => "[] === true", errors => [{ message => "Unexpected constant binary expression. Compares constantly with the right-hand side of the `===`." }] },
                    { code => "[] !== true", errors => [{ message => "Unexpected constant binary expression. Compares constantly with the right-hand side of the `!==`." }] },

                    // Motivating examples from the original proposal https://github.com/eslint/eslint/issues/13752
                    { code => "!foo == null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "!foo ?? bar", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a + b) / 2 ?? bar", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "String(foo.bar) ?? baz", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => r#""hello" + name ?? """#, errors => [{ message_id => "constant_short_circuit" }] },
                    { code => r#"[foo?.bar ?? ""] ?? []"#, errors => [{ message_id => "constant_short_circuit" }] },

                    // Logical expression with constant truthiness
                    { code => "true && hello", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "true || hello", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "true && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "'' && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "100 && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "+100 && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "-100 && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "~100 && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "/[a-z]/ && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "Boolean([]) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "Boolean() && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "Boolean([], n) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "({}) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "[] && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(() => {}) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(function() {}) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(class {}) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(class { valueOf() { return x; } }) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(class { [x]() { return x; } }) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "new Foo() && foo", errors => [{ message_id => "constant_short_circuit" }] },

                    // (boxed values are always truthy)
                    { code => "new Boolean(unknown) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(bar = false) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(bar.baz = false) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(bar[0] = false) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "`hello ${hello}` && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "void bar && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "!true && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "typeof bar && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(bar, baz, true) && foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "undefined && foo", errors => [{ message_id => "constant_short_circuit" }] },

                    // Logical expression with constant nullishness
                    { code => "({}) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "([]) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(() => {}) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(function() {}) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(class {}) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "new Foo() ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "1 ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "/[a-z]/ ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "`${''}` ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a = true) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a += 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a -= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a *= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a /= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a %= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a <<= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a >>= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a >>>= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a |= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a ^= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(a &= 1) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "undefined ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "!bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "void bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "typeof bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "+bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "-bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "~bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "++bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "bar++ ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "--bar ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "bar-- ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(x == y) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(x + y) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(x / y) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(x instanceof String) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "(x in y) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "Boolean(x) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "String(x) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "Number(x) ?? foo", errors => [{ message_id => "constant_short_circuit" }] },

                    // Binary expression with comparison to null
                    { code => "({}) != null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) == null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "null == ({})", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) == undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "undefined == ({})", errors => [{ message_id => "constant_binary_operand" }] },

                    // Binary expression with loose comparison to boolean
                    { code => "({}) != true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "([]) == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "([a, b]) == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(() => {}) == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(function() {}) == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "void foo == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "typeof foo == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "![] == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true == class {}", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true == 1", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "undefined == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true == undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "`hello` == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "/[a-z]/ == true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) == Boolean({})", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) == Boolean()", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) == Boolean(() => {}, foo)", errors => [{ message_id => "constant_binary_operand" }] },

                    // Binary expression with strict comparison to boolean
                    { code => "({}) !== true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) == !({})", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "([]) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(function() {}) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(() => {}) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "!{} === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "typeof n === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "void n === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "+n === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "-n === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "~n === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "1 === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "'hello' === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "/[a-z]/ === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "undefined === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a = {}) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a += 1) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a -= 1) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a *= 1) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a %= 1) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a ** b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a << b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a >> b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a >>> b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "--a === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "a-- === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "++a === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "a++ === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a + b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a - b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a * b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a / b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a % b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a | b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a ^ b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a & b) === true", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "Boolean(0) === Boolean(1)", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true === String(x)", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true === Number(x)", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "Boolean(0) == !({})", errors => [{ message_id => "constant_binary_operand" }] },

                    // Binary expression with strict comparison to null
                    { code => "({}) !== null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "([]) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(() => {}) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(function() {}) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(class {}) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "new Foo() === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "`` === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "1 === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "'hello' === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "/[a-z]/ === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "null === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "a++ === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "++a === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "--a === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "a-- === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "!a === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "typeof a === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "delete a === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "void a === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "undefined === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(x = {}) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(x += y) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(x -= y) === null", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a, b, {}) === null", errors => [{ message_id => "constant_binary_operand" }] },

                    // Binary expression with strict comparison to undefined
                    { code => "({}) !== undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "({}) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "([]) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(() => {}) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(function() {}) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(class {}) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "new Foo() === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "`` === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "1 === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "'hello' === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "/[a-z]/ === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "true === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "null === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "a++ === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "++a === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "--a === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "a-- === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "!a === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "typeof a === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "delete a === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "void a === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "undefined === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(x = {}) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(x += y) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(x -= y) === undefined", errors => [{ message_id => "constant_binary_operand" }] },
                    { code => "(a, b, {}) === undefined", errors => [{ message_id => "constant_binary_operand" }] },

                    /*
                     * If both sides are newly constructed objects, we can tell they will
                     * never be equal, even with == equality.
                     */
                    { code => "[a] == [a]", errors => [{ message_id => "both_always_new" }] },
                    { code => "[a] != [a]", errors => [{ message_id => "both_always_new" }] },
                    { code => "({}) == []", errors => [{ message_id => "both_always_new" }] },

                    // Comparing to always new objects
                    { code => "x === {}", errors => [{ message_id => "always_new" }] },
                    { code => "x !== {}", errors => [{ message_id => "always_new" }] },
                    { code => "x === []", errors => [{ message_id => "always_new" }] },
                    { code => "x === (() => {})", errors => [{ message_id => "always_new" }] },
                    { code => "x === (function() {})", errors => [{ message_id => "always_new" }] },
                    { code => "x === (class {})", errors => [{ message_id => "always_new" }] },
                    { code => "x === new Boolean()", errors => [{ message_id => "always_new" }] },
                    { code => "x === new Promise()", environment => { env => { es6 => true } }, errors => [{ message_id => "always_new" }] },
                    { code => "x === new WeakSet()", environment => { env => { es6 => true } }, errors => [{ message_id => "always_new" }] },
                    { code => "x === (foo, {})", errors => [{ message_id => "always_new" }] },
                    { code => "x === (y = {})", errors => [{ message_id => "always_new" }] },
                    { code => "x === (y ? {} : [])", errors => [{ message_id => "always_new" }] },
                    { code => "x === /[a-z]/", errors => [{ message_id => "always_new" }] },

                    // It's not obvious what this does, but it compares the old value of `x` to the new object.
                    { code => "x === (x = {})", errors => [{ message_id => "always_new" }] },

                    { code => "window.abc && false && anything", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "window.abc || true || anything", errors => [{ message_id => "constant_short_circuit" }] },
                    { code => "window.abc ?? 'non-nullish' ?? anything", errors => [{ message_id => "constant_short_circuit" }] }
                ]
            },
            get_instance_provider_factory(),
            json_object!({
                "ecma_version": 2021,
            }),
        )
    }
}
