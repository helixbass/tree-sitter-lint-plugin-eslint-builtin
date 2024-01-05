use std::{collections::HashMap, sync::Arc};

use once_cell::sync::Lazy;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{
        get_call_expression_arguments, get_number_literal_value, NodeExtJs, Number, NumberOrBigInt,
    },
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum System {
    Binary,
    Octal,
    Hexadecimal,
}

struct RadixSpec {
    system: System,
    literal_prefix: &'static str,
}

static RADIX_MAP: Lazy<HashMap<Number, RadixSpec>> = Lazy::new(|| {
    [
        (
            Number::Integer(2),
            RadixSpec {
                system: System::Binary,
                literal_prefix: "0b",
            },
        ),
        (
            Number::Integer(8),
            RadixSpec {
                system: System::Octal,
                literal_prefix: "0o",
            },
        ),
        (
            Number::Integer(16),
            RadixSpec {
                system: System::Hexadecimal,
                literal_prefix: "0x",
            },
        ),
    ]
    .into()
});

fn is_parse_int(callee_node: Node, context: &QueryMatchContext) -> bool {
    ast_utils::is_specific_id(callee_node, "parseInt", context)
        || ast_utils::is_specific_member_access(
            callee_node,
            Some("Number"),
            Some("parseInt"),
            context,
        )
}

pub fn prefer_numeric_literals_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-numeric-literals",
        languages => [Javascript],
        messages => [
            use_literal => "Use {{system}} literals instead of {{function_name}}().",
        ],
        fixable => true,
        listeners => [
            r#"
              (call_expression
                arguments: (arguments
                  [
                    (string)
                    (template_string)
                  ]
                  (number)
                )
              ) @c
            "# => |node, context| {
                let mut args = get_call_expression_arguments(node).unwrap();
                let str_node = args.next().unwrap();
                if !ast_utils::is_string_literal(str_node) {
                    return;
                }
                let Some(str_) = ast_utils::get_static_string_value(str_node, context) else {
                    return;
                };
                let radix_node = args.next().unwrap();
                let NumberOrBigInt::Number(radix) = get_number_literal_value(radix_node, context) else {
                    return;
                };
                let Some(RadixSpec { system, literal_prefix }) = RADIX_MAP.get(&radix) else {
                    return;
                };

                if !is_parse_int(node.field("function").skip_parentheses(), context) {
                    return;
                }

                context.report(violation! {
                    node => node,
                    message_id => "use_literal",
                    data => {
                        system => match system {
                            System::Binary => "binary",
                            System::Octal => "octal",
                            System::Hexadecimal => "hexadecimal",
                        },
                        function_name => node.field("function").skip_parentheses().text(context),
                    },
                    fix => |fixer| {
                        if context.get_comments_inside(node).next().is_some() {
                            return;
                        }

                        let replacement = format!(
                            "{}{}",
                            literal_prefix, str_
                        );

                        if !matches!(
                            i64::from_str_radix(
                                &str_,
                                match radix {
                                    Number::Integer(radix) => u32::try_from(radix).unwrap(),
                                    _ => unreachable!(),
                                }
                            ),
                            Ok(parsed) if NumberOrBigInt::from(&*replacement) == NumberOrBigInt::Number(Number::Integer(parsed))
                        ) {
                            return;
                        }

                        let token_before = context.maybe_get_token_before(node, Option::<fn(Node) -> bool>::None);
                        let token_after = context.maybe_get_token_after(node, Option::<fn(Node) -> bool>::None);
                        let mut prefix = "";
                        let mut suffix = "";

                        if token_before.matches(|token_before| {
                            token_before.end_byte() == node.start_byte() &&
                                !ast_utils::can_tokens_be_adjacent(
                                    token_before,
                                    &*replacement,
                                    context,
                                )
                        }) {
                            prefix = " ";
                        }

                        if token_after.matches(|token_after| {
                            node.end_byte() == token_after.start_byte() &&
                                !ast_utils::can_tokens_be_adjacent(
                                    &*replacement,
                                    token_after,
                                    context,
                                )
                        }) {
                            suffix = " ";
                        }

                        fixer.replace_text(
                            node,
                            format!("{prefix}{replacement}{suffix}")
                        );
                    }
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_prefer_numeric_literals_rule() {
        RuleTester::run(
            prefer_numeric_literals_rule(),
            rule_tests! {
                valid => [
                    "parseInt(1);",
                    "parseInt(1, 3);",
                    "Number.parseInt(1);",
                    "Number.parseInt(1, 3);",
                    "0b111110111 === 503;",
                    "0o767 === 503;",
                    "0x1F7 === 503;",
                    "a[parseInt](1,2);",
                    "parseInt(foo);",
                    "parseInt(foo, 2);",
                    "Number.parseInt(foo);",
                    "Number.parseInt(foo, 2);",
                    "parseInt(11, 2);",
                    "Number.parseInt(1, 8);",
                    "parseInt(1e5, 16);",
                    "parseInt('11', '2');",
                    "Number.parseInt('11', '8');",
                    "parseInt(/foo/, 2);",
                    "parseInt(`11${foo}`, 2);",
                    {
                        code => "parseInt('11', 2n);",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "Number.parseInt('11', 8n);",
                        environment => { ecma_version => 2020 },
                    },
                    {
                        code => "parseInt('11', 16n);",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "parseInt(`11`, 16n);",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "parseInt(1n, 2);",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "class C { #parseInt; foo() { Number.#parseInt(\"111110111\", 2); } }",
                        environment => { ecma_version => 2022 }
                    }
                ],
                invalid => [
                    {
                        code => "parseInt(\"111110111\", 2) === 503;",
                        output => "0b111110111 === 503;",
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    }, {
                        code => "parseInt(\"767\", 8) === 503;",
                        output => "0o767 === 503;",
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    }, {
                        code => "parseInt(\"1F7\", 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    }, {
                        code => "Number.parseInt(\"111110111\", 2) === 503;",
                        output => "0b111110111 === 503;",
                        errors => [{ message => "Use binary literals instead of Number.parseInt()." }]
                    }, {
                        code => "Number.parseInt(\"767\", 8) === 503;",
                        output => "0o767 === 503;",
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    }, {
                        code => "Number.parseInt(\"1F7\", 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of Number.parseInt()." }]
                    }, {
                        code => "parseInt('7999', 8);",
                        output => None, // not fixed, unexpected 9 in parseInt string
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    }, {
                        code => "parseInt('1234', 2);",
                        output => None, // not fixed, invalid binary string
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    }, {
                        code => "parseInt('1234.5', 8);",
                        output => None, // not fixed, this isn't an integer
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    }, {
                        code => "parseInt('1️⃣3️⃣3️⃣7️⃣', 16);",
                        output => None, // not fixed, javascript doesn't support emoji literals
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    }, {
                        code => "Number.parseInt('7999', 8);",
                        output => None, // not fixed, unexpected 9 in parseInt string
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    }, {
                        code => "Number.parseInt('1234', 2);",
                        output => None, // not fixed, invalid binary string
                        errors => [{ message => "Use binary literals instead of Number.parseInt()." }]
                    }, {
                        code => "Number.parseInt('1234.5', 8);",
                        output => None, // not fixed, this isn't an integer
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    }, {
                        code => "Number.parseInt('1️⃣3️⃣3️⃣7️⃣', 16);",
                        output => None, // not fixed, javascript doesn't support emoji literals
                        errors => [{ message => "Use hexadecimal literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "parseInt(`111110111`, 2) === 503;",
                        output => "0b111110111 === 503;",
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    }, {
                        code => "parseInt(`767`, 8) === 503;",
                        output => "0o767 === 503;",
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    }, {
                        code => "parseInt(`1F7`, 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {
                        code => "parseInt('', 8);",
                        output => None, // not fixed, it's empty string
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    },
                    {
                        code => "parseInt(``, 8);",
                        output => None, // not fixed, it's empty string
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    },
                    {
                        code => "parseInt(`7999`, 8);",
                        output => None, // not fixed, unexpected 9 in parseInt string
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    }, {
                        code => "parseInt(`1234`, 2);",
                        output => None, // not fixed, invalid binary string
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    }, {
                        code => "parseInt(`1234.5`, 8);",
                        output => None, // not fixed, this isn't an integer
                        errors => [{ message => "Use octal literals instead of parseInt()." }]
                    },

                    // Adjacent tokens tests
                    {
                        code => "parseInt('11', 2)",
                        output => "0b11",
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    },
                    {
                        code => "Number.parseInt('67', 8)",
                        output => "0o67",
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "5+parseInt('A', 16)",
                        output => "5+0xA",
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {
                        code => "function *f(){ yield(Number).parseInt('11', 2) }",
                        output => "function *f(){ yield 0b11 }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Use binary literals instead of (Number).parseInt()." }],
                    },
                    {
                        code => "function *f(){ yield(Number.parseInt)('67', 8) }",
                        output => "function *f(){ yield 0o67 }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }],
                    },
                    {
                        code => "function *f(){ yield(parseInt)('A', 16) }",
                        output => "function *f(){ yield 0xA }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {
                        code => "function *f(){ yield Number.parseInt('11', 2) }",
                        output => "function *f(){ yield 0b11 }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Use binary literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "function *f(){ yield/**/Number.parseInt('67', 8) }",
                        output => "function *f(){ yield/**/0o67 }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "function *f(){ yield(parseInt('A', 16)) }",
                        output => "function *f(){ yield(0xA) }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {
                        code => "parseInt('11', 2)+5",
                        output => "0b11+5",
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    },
                    {
                        code => "Number.parseInt('17', 8)+5",
                        output => "0o17+5",
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "parseInt('A', 16)+5",
                        output => "0xA+5",
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {
                        code => "parseInt('11', 2)in foo",
                        output => "0b11 in foo",
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    },
                    {
                        code => "Number.parseInt('17', 8)in foo",
                        output => "0o17 in foo",
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "parseInt('A', 16)in foo",
                        output => "0xA in foo",
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {
                        code => "parseInt('11', 2) in foo",
                        output => "0b11 in foo",
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    },
                    {
                        code => "Number.parseInt('17', 8)/**/in foo",
                        output => "0o17/**/in foo",
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "(parseInt('A', 16))in foo",
                        output => "(0xA)in foo",
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },

                    // Should not autofix if it would remove comments
                    {
                        code => "/* comment */Number.parseInt('11', 2);",
                        output => "/* comment */0b11;",
                        errors => 1
                    },
                    {
                        code => "Number/**/.parseInt('11', 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "Number//\n.parseInt('11', 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "Number./**/parseInt('11', 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "Number.parseInt(/**/'11', 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "Number.parseInt('11', /**/2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "Number.parseInt('11', 2)/* comment */;",
                        output => "0b11/* comment */;",
                        errors => 1
                    },
                    {
                        code => "parseInt/**/('11', 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "parseInt(//\n'11', 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "parseInt('11'/**/, 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "parseInt(`11`/**/, 2);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "parseInt('11', 2 /**/);",
                        output => None,
                        errors => 1
                    },
                    {
                        code => "parseInt('11', 2)//comment\n;",
                        output => "0b11//comment\n;",
                        errors => 1
                    },

                    // Optional chaining
                    {
                        code => "parseInt?.(\"1F7\", 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {
                        code => "Number?.parseInt(\"1F7\", 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of Number?.parseInt()." }]
                    },
                    {
                        code => "Number?.parseInt?.(\"1F7\", 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of Number?.parseInt()." }]
                    },
                    {
                        code => "(Number?.parseInt)(\"1F7\", 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of Number?.parseInt()." }]
                    },
                    {
                        code => "(Number?.parseInt)?.(\"1F7\", 16) === 255;",
                        output => "0x1F7 === 255;",
                        errors => [{ message => "Use hexadecimal literals instead of Number?.parseInt()." }]
                    },

                    // `parseInt` doesn't support numeric separators. The rule shouldn't autofix in those cases.
                    {
                        code => "parseInt('1_0', 2);",
                        output => None,
                        errors => [{ message => "Use binary literals instead of parseInt()." }]
                    },
                    {
                        code => "Number.parseInt('5_000', 8);",
                        output => None,
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
                    },
                    {
                        code => "parseInt('0_1', 16);",
                        output => None,
                        errors => [{ message => "Use hexadecimal literals instead of parseInt()." }]
                    },
                    {

                        // this would be indeed the same as `0x0_0`, but there's no need to autofix this edge case that looks more like a mistake.
                        code => "Number.parseInt('0_0', 16);",
                        output => None,
                        errors => [{ message => "Use hexadecimal literals instead of Number.parseInt()." }]
                    }
                ]
            },
        )
    }
}
