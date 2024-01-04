use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn prefer_template_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-template",
        languages => [Javascript],
        messages => [
            unexpected_string_concatenation => "Unexpected string concatenation.",
        ],
        fixable => true,
        listeners => [
            r#"
              (debugger_statement) @c
            "# => |node, context| {
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
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::BinaryExpression;

    #[test]
    fn test_prefer_template_rule() {
        let errors = vec![RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected_string_concatenation")
            .type_(BinaryExpression)
            .build()
            .unwrap()];

        RuleTester::run(
            prefer_template_rule(),
            rule_tests! {
                valid => [
                    "'use strict';",
                    "var foo = 'foo' + '\\0';",
                    "var foo = 'bar';",
                    "var foo = 'bar' + 'baz';",
                    "var foo = foo + +'100';",
                    "var foo = `bar`;",
                    "var foo = `hello, ${name}!`;",

                    // https://github.com/eslint/eslint/issues/3507
                    "var foo = `foo` + `bar` + \"hoge\";",
                    "var foo = `foo` +\n    `bar` +\n    \"hoge\";"
                ],
                invalid => [
                    {
                        code => "var foo = 'hello, ' + name + '!';",
                        output => "var foo = `hello, ${  name  }!`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'baz';",
                        output => "var foo = `${bar  }baz`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + `baz`;",
                        output => "var foo = `${bar  }baz`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = +100 + 'yen';",
                        output => "var foo = `${+100  }yen`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = 'bar' + baz;",
                        output => "var foo = `bar${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '￥' + (n * 1000) + '-'",
                        output => "var foo = `￥${  n * 1000  }-`",
                        errors => errors,
                    },
                    {
                        code => "var foo = 'aaa' + aaa; var bar = 'bbb' + bbb;",
                        output => "var foo = `aaa${  aaa}`; var bar = `bbb${  bbb}`;",
                        errors => [errors[0], errors[0]]
                    },
                    {
                        code => "var string = (number + 1) + 'px';",
                        output => "var string = `${number + 1  }px`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = 'bar' + baz + 'qux';",
                        output => "var foo = `bar${  baz  }qux`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '0 backslashes: ${bar}' + baz;",
                        output => "var foo = `0 backslashes: \\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '1 backslash: \\${bar}' + baz;",
                        output => "var foo = `1 backslash: \\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '2 backslashes: \\\\${bar}' + baz;",
                        output => "var foo = `2 backslashes: \\\\\\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = '3 backslashes: \\\\\\${bar}' + baz;",
                        output => "var foo = `3 backslashes: \\\\\\${bar}${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'this is a backtick: `' + baz;",
                        output => "var foo = `${bar  }this is a backtick: \\`${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'this is a backtick preceded by a backslash: \\`' + baz;",
                        output => "var foo = `${bar  }this is a backtick preceded by a backslash: \\`${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + 'this is a backtick preceded by two backslashes: \\\\`' + baz;",
                        output => "var foo = `${bar  }this is a backtick preceded by two backslashes: \\\\\\`${  baz}`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + `${baz}foo`;",
                        output => "var foo = `${bar  }${baz}foo`;",
                        errors => errors,
                    },
                    {
                        code =>
                        "var foo = 'favorites: ' + favorites.map(f => {
    return f.name;
}) + ';';",
                        output =>
                        "var foo = `favorites: ${  favorites.map(f => {
    return f.name;
})  };`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + baz + 'qux';",
                        output => "var foo = `${bar + baz  }qux`;",
                        errors => errors,
                    },
                    {
                        code =>
                        "var foo = 'favorites: ' +
    favorites.map(f => {
        return f.name;
    }) +
';';",
                        output =>
                        "var foo = `favorites: ${
    favorites.map(f => {
        return f.name;
    }) 
};`;",
                        errors => errors,
                    },
                    {
                        code => "var foo = /* a */ 'bar' /* b */ + /* c */ baz /* d */ + 'qux' /* e */ ;",
                        output => "var foo = /* a */ `bar${ /* b */  /* c */ baz /* d */  }qux` /* e */ ;",
                        errors => errors,
                    },
                    {
                        code => "var foo = bar + ('baz') + 'qux' + (boop);",
                        output => "var foo = `${bar  }baz` + `qux${  boop}`;",
                        errors => errors,
                    },
                    {
                        code => "foo + 'unescapes an escaped single quote in a single-quoted string: \\''",
                        output => "`${foo  }unescapes an escaped single quote in a single-quoted string: '`",
                        errors => errors,
                    },
                    {
                        code => "foo + \"unescapes an escaped double quote in a double-quoted string: \\\"\"",
                        output => "`${foo  }unescapes an escaped double quote in a double-quoted string: \"`",
                        errors => errors,
                    },
                    {
                        code => "foo + 'does not unescape an escaped double quote in a single-quoted string: \\\"'",
                        output => "`${foo  }does not unescape an escaped double quote in a single-quoted string: \\\"`",
                        errors => errors,
                    },
                    {
                        code => "foo + \"does not unescape an escaped single quote in a double-quoted string: \\'\"",
                        output => "`${foo  }does not unescape an escaped single quote in a double-quoted string: \\'`",
                        errors => errors,
                    },
                    {
                        code => "foo + 'handles unicode escapes correctly: \\x27'", // "\x27" === "'"
                        output => "`${foo  }handles unicode escapes correctly: \\x27`",
                        errors => errors,
                    },
                    {
                        code => "foo + 'does not autofix octal escape sequence' + '\\033'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + 'does not autofix non-octal decimal escape sequence' + '\\8'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\n other text \\033'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\0\\1'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\08'",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo + '\\\\033'",
                        output => "`${foo  }\\\\033`",
                        errors => errors,
                    },
                    {
                        code => "foo + '\\0'",
                        output => "`${foo  }\\0`",
                        errors => errors,
                    },

                    // https://github.com/eslint/eslint/issues/15083
                    {
                        code => r#""default-src 'self' https://*.google.com;"
                        + "frame-ancestors 'none';"
                        + "report-to " + foo + ";""#,
                        output => r#"\`default-src 'self' https://*.google.com;\`
                        + \`frame-ancestors 'none';\`
                        + \`report-to \${  foo  };\`"#,
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo",
                        output => "`a` + `b${  foo}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + 'c' + 'd'",
                        output => "`a` + `b${  foo  }c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b + c' + foo + 'd' + 'e'",
                        output => "`a` + `b + c${  foo  }d` + `e`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + 'd')",
                        output => "`a` + `b${  foo  }c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('a' + 'b')",
                        output => "`a` + `b${  foo  }a` + `b`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + 'd') + ('e' + 'f')",
                        output => "`a` + `b${  foo  }c` + `d` + `e` + `f`",
                        errors => errors,
                    },
                    {
                        code => "foo + ('a' + 'b') + ('c' + 'd')",
                        output => "`${foo  }a` + `b` + `c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + foo + ('b' + 'c') + ('d' + bar + 'e')",
                        output => "`a${  foo  }b` + `c` + `d${  bar  }e`",
                        errors => errors,
                    },
                    {
                        code => "foo + ('b' + 'c') + ('d' + bar + 'e')",
                        output => "`${foo  }b` + `c` + `d${  bar  }e`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + 'd' + 'e')",
                        output => "`a` + `b${  foo  }c` + `d` + `e`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + bar + 'd')",
                        output => "`a` + `b${  foo  }c${  bar  }d`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + bar + ('d' + 'e') + 'f')",
                        output => "`a` + `b${  foo  }c${  bar  }d` + `e` + `f`",
                        errors => errors,
                    },
                    {
                        code => "'a' + 'b' + foo + ('c' + bar + 'e') + 'f' + test",
                        output => "`a` + `b${  foo  }c${  bar  }e` + `f${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + foo + ('b' + bar + 'c') + ('d' + test)",
                        output => "`a${  foo  }b${  bar  }c` + `d${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + foo + ('b' + 'c') + ('d' + bar)",
                        output => "`a${  foo  }b` + `c` + `d${  bar}`",
                        errors => errors,
                    },
                    {
                        code => "foo + ('a' + bar + 'b') + 'c' + test",
                        output => "`${foo  }a${  bar  }b` + `c${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + '`b`' + c",
                        output => "`a` + `\\`b\\`${  c}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + '`b` + `c`' + d",
                        output => "`a` + `\\`b\\` + \\`c\\`${  d}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + b + ('`c`' + '`d`')",
                        output => "`a${  b  }\\`c\\`` + `\\`d\\``",
                        errors => errors,
                    },
                    {
                        code => "'`a`' + b + ('`c`' + '`d`')",
                        output => "`\\`a\\`${  b  }\\`c\\`` + `\\`d\\``",
                        errors => errors,
                    },
                    {
                        code => "foo + ('`a`' + bar + '`b`') + '`c`' + test",
                        output => "`${foo  }\\`a\\`${  bar  }\\`b\\`` + `\\`c\\`${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('b' + 'c') + d",
                        output => "`a` + `b` + `c${  d}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('`b`' + '`c`') + d",
                        output => "`a` + `\\`b\\`` + `\\`c\\`${  d}`",
                        errors => errors,
                    },
                    {
                        code => "a + ('b' + 'c') + d",
                        output => "`${a  }b` + `c${  d}`",
                        errors => errors,
                    },
                    {
                        code => "a + ('b' + 'c') + (d + 'e')",
                        output => "`${a  }b` + `c${  d  }e`",
                        errors => errors,
                    },
                    {
                        code => "a + ('`b`' + '`c`') + d",
                        output => "`${a  }\\`b\\`` + `\\`c\\`${  d}`",
                        errors => errors,
                    },
                    {
                        code => "a + ('`b` + `c`' + '`d`') + e",
                        output => "`${a  }\\`b\\` + \\`c\\`` + `\\`d\\`${  e}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('b' + 'c' + 'd') + e",
                        output => "`a` + `b` + `c` + `d${  e}`",
                        errors => errors,
                    },
                    {
                        code => "'a' + ('b' + 'c' + 'd' + (e + 'f') + 'g' +'h' + 'i') + j",
                        output => "`a` + `b` + `c` + `d${  e  }fg` +`h` + `i${  j}`",
                        errors => errors,
                    },
                    {
                        code => "a + (('b' + 'c') + 'd')",
                        output => "`${a  }b` + `c` + `d`",
                        errors => errors,
                    },
                    {
                        code => "(a + 'b') + ('c' + 'd') + e",
                        output => "`${a  }b` + `c` + `d${  e}`",
                        errors => errors,
                    },
                    {
                        code => "var foo = \"Hello \" + \"world \" + \"another \" + test",
                        output => "var foo = `Hello ` + `world ` + `another ${  test}`",
                        errors => errors,
                    },
                    {
                        code => "'Hello ' + '\"world\" ' + test",
                        output => "`Hello ` + `\"world\" ${  test}`",
                        errors => errors,
                    },
                    {
                        code => "\"Hello \" + \"'world' \" + test",
                        output => "`Hello ` + `'world' ${  test}`",
                        errors => errors,
                    }
                ]
            },
        )
    }
}
