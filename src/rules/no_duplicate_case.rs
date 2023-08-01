use std::sync::Arc;

use tree_sitter_lint::{rule, tree_sitter::Node, violation, QueryMatchContext, Rule};

use crate::utils::ast_utils;

fn equal(a: Node, b: Node, context: &QueryMatchContext) -> bool {
    if a.kind_id() != b.kind_id() {
        return false;
    }

    ast_utils::equal_tokens(a, b, context)
}

pub fn no_duplicate_case_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-duplicate-case",
        languages => [Javascript],
        messages => [
            unexpected => "Duplicate case label.",
        ],
        listeners => [
            r#"(
              ((switch_case) @switch_case (comment)*)+
            )"# => |captures, context| {
                let mut previous_tests = vec![];

                for switch_case in captures.get_all("switch_case") {
                    let test = switch_case.child_by_field_name("value").unwrap();

                    if previous_tests.iter().any(|&previous_test| {
                        equal(previous_test, test, context)
                    }) {
                        context.report(violation! {
                            node => switch_case,
                            message_id => "unexpected",
                        });
                    } else {
                        previous_tests.push(test);
                    }
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_duplicate_case_rule() {
        RuleTester::run(
            no_duplicate_case_rule(),
            rule_tests! {
                valid => [
                    "var a = 1; switch (a) {case 1: break; case 2: break; default: break;}",
                    "var a = 1; switch (a) {case 1: break; case '1': break; default: break;}",
                    "var a = 1; switch (a) {case 1: break; case true: break; default: break;}",
                    "var a = 1; switch (a) {default: break;}",
                    "var a = 1, p = {p: {p1: 1, p2: 1}}; switch (a) {case p.p.p1: break; case p.p.p2: break; default: break;}",
                    "var a = 1, f = function(b) { return b ? { p1: 1 } : { p1: 2 }; }; switch (a) {case f(true).p1: break; case f(true, false).p1: break; default: break;}",
                    "var a = 1, f = function(s) { return { p1: s } }; switch (a) {case f(a + 1).p1: break; case f(a + 2).p1: break; default: break;}",
                    "var a = 1, f = function(s) { return { p1: s } }; switch (a) {case f(a == 1 ? 2 : 3).p1: break; case f(a === 1 ? 2 : 3).p1: break; default: break;}",
                    "var a = 1, f1 = function() { return { p1: 1 } }, f2 = function() { return { p1: 2 } }; switch (a) {case f1().p1: break; case f2().p1: break; default: break;}",
                    "var a = [1,2]; switch(a.toString()){case ([1,2]).toString():break; case ([1]).toString():break; default:break;}",
                    "switch(a) { case a: break; } switch(a) { case a: break; }",
                    "switch(a) { case toString: break; }"
                ],
                invalid => [
                    {
                        code => "var a = 1; switch (a) {case 1: break; case 1: break; case 2: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 39
                        }]
                    },
                    {
                        code => "var a = '1'; switch (a) {case '1': break; case '1': break; case '2': break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 43
                        }]
                    },
                    {
                        code => "var a = 1, one = 1; switch (a) {case one: break; case one: break; case 2: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 50
                        }]
                    },
                    {
                        code => "var a = 1, p = {p: {p1: 1, p2: 1}}; switch (a) {case p.p.p1: break; case p.p.p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 69
                        }]
                    },
                    {
                        code => "var a = 1, f = function(b) { return b ? { p1: 1 } : { p1: 2 }; }; switch (a) {case f(true).p1: break; case f(true).p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 103
                        }]
                    },
                    {
                        code => "var a = 1, f = function(s) { return { p1: s } }; switch (a) {case f(a + 1).p1: break; case f(a + 1).p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 87
                        }]
                    },
                    {
                        code => "var a = 1, f = function(s) { return { p1: s } }; switch (a) {case f(a === 1 ? 2 : 3).p1: break; case f(a === 1 ? 2 : 3).p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 97
                        }]
                    },
                    {
                        code => "var a = 1, f1 = function() { return { p1: 1 } }; switch (a) {case f1().p1: break; case f1().p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 83
                        }]
                    },
                    {
                        code => "var a = [1, 2]; switch(a.toString()){case ([1, 2]).toString():break; case ([1, 2]).toString():break; default:break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 70
                        }]
                    },
                    {
                        code => "switch (a) { case a: case a: }",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 22
                        }]
                    },
                    {
                        code => "switch (a) { case a: break; case b: break; case a: break; case c: break; case a: break; }",
                        errors => [
                            {
                                message_id => "unexpected",
                                type => "switch_case",
                                column => 44
                            },
                            {
                                message_id => "unexpected",
                                type => "switch_case",
                                column => 74
                            }
                        ]
                    },
                    {
                        code => "var a = 1, p = {p: {p1: 1, p2: 1}}; switch (a) {case p.p.p1: break; case p. p // comment\n .p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 69
                        }]
                    },
                    {
                        code => "var a = 1, p = {p: {p1: 1, p2: 1}}; switch (a) {case p .p\n/* comment */\n.p1: break; case p.p.p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            line => 3,
                            column => 13
                        }]
                    },
                    {
                        code => "var a = 1, p = {p: {p1: 1, p2: 1}}; switch (a) {case p .p\n/* comment */\n.p1: break; case p. p // comment\n .p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            line => 3,
                            column => 13
                        }]
                    },
                    {
                        code => "var a = 1, p = {p: {p1: 1, p2: 1}}; switch (a) {case p.p.p1: break; case p. p // comment\n .p1: break; case p .p\n/* comment */\n.p1: break; default: break;}",
                        errors => [
                            {
                                message_id => "unexpected",
                                type => "switch_case",
                                line => 1,
                                column => 69
                            },
                            {
                                message_id => "unexpected",
                                type => "switch_case",
                                line => 2,
                                column => 14
                            }
                        ]
                    },
                    {
                        code => "var a = 1, f = function(s) { return { p1: s } }; switch (a) {case f(a + 1).p1: break; case f(a+1).p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            column => 87
                        }]
                    },
                    {
                        code => "var a = 1, f = function(s) { return { p1: s } }; switch (a) {case f(\na + 1 // comment\n).p1: break; case f(a+1)\n.p1: break; default: break;}",
                        errors => [{
                            message_id => "unexpected",
                            type => "switch_case",
                            line => 3,
                            column => 14
                        }]
                    }
                ]
            },
        )
    }
}
