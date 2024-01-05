use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn prefer_numeric_literals_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-numeric-literals",
        languages => [Javascript],
        messages => [
            use_literal => "Use {{system}} literals instead of {{function_name}}().",
        ],
        fixable => true,
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
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
                        environment => { ecma_version => 2020 }
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
                        errors => [{ message => "Use binary literals instead of (Number).parseInt()." }]
                    },
                    {
                        code => "function *f(){ yield(Number.parseInt)('67', 8) }",
                        output => "function *f(){ yield 0o67 }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Use octal literals instead of Number.parseInt()." }]
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
