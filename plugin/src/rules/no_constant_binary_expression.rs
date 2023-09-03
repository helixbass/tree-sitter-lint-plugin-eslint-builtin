use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule, NodeExt};

use crate::{ast_helpers::is_logical_expression, scope::ScopeManager, utils::ast_utils::is_constant};

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
                    let left = node.field("left");
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
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_no_constant_binary_expression_rule() {
        RuleTester::run(
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
                    { code => "x === new Promise()", /*env: { es6: true },*/ errors => [{ message_id => "always_new" }] },
                    { code => "x === new WeakSet()", /*env: { es6: true },*/ errors => [{ message_id => "always_new" }] },
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
        )
    }
}
