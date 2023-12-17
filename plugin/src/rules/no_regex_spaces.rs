use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

pub fn no_regex_spaces_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-regex-spaces",
        languages => [Javascript],
        messages => [
            multiple_spaces => "Spaces are hard to count. Use {{{length}}}.",
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
    use crate::{kind, kind::NewExpression};

    #[test]
    fn test_no_regex_spaces_rule() {
        RuleTester::run(
            no_regex_spaces_rule(),
            rule_tests! {
                valid => [
                    "var foo = /foo/;",
                    "var foo = RegExp('foo')",
                    "var foo = / /;",
                    "var foo = RegExp(' ')",
                    "var foo = / a b c d /;",
                    "var foo = /bar {3}baz/g;",
                    "var foo = RegExp('bar {3}baz', 'g')",
                    "var foo = new RegExp('bar {3}baz')",
                    "var foo = /bar\t\t\tbaz/;",
                    "var foo = RegExp('bar\t\t\tbaz');",
                    "var foo = new RegExp('bar\t\t\tbaz');",
                    "var RegExp = function() {}; var foo = new RegExp('bar   baz');",
                    "var RegExp = function() {}; var foo = RegExp('bar   baz');",
                    "var foo = /  +/;",
                    "var foo = /  ?/;",
                    "var foo = /  */;",
                    "var foo = /  {2}/;",

                    // don't report if there are no consecutive spaces in the source code
                    "var foo = /bar \\ baz/;",
                    "var foo = /bar\\ \\ baz/;",
                    "var foo = /bar \\u0020 baz/;",
                    "var foo = /bar\\u0020\\u0020baz/;",
                    "var foo = new RegExp('bar \\ baz')",
                    "var foo = new RegExp('bar\\ \\ baz')",
                    "var foo = new RegExp('bar \\\\ baz')",
                    "var foo = new RegExp('bar \\u0020 baz')",
                    "var foo = new RegExp('bar\\u0020\\u0020baz')",
                    "var foo = new RegExp('bar \\\\u0020 baz')",

                    // don't report spaces in character classes
                    "var foo = /[  ]/;",
                    "var foo = /[   ]/;",
                    "var foo = / [  ] /;",
                    "var foo = / [  ] [  ] /;",
                    "var foo = new RegExp('[  ]');",
                    "var foo = new RegExp('[   ]');",
                    "var foo = new RegExp(' [  ] ');",
                    "var foo = RegExp(' [  ] [  ] ');",
                    "var foo = new RegExp(' \\[   ');",
                    "var foo = new RegExp(' \\[   \\] ');",

                    // ES2024
                    { code => "var foo = /  {2}/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[\\q{    }]/v;", environment => { ecma_version => 2024 } },

                    // don't report invalid regex
                    "var foo = new RegExp('[  ');",
                    "var foo = new RegExp('{  ', 'u');",

                    // don't report if flags cannot be determined
                    "new RegExp('  ', flags)",
                    "new RegExp('[[abc]  ]', flags + 'v')",
                    "new RegExp('[[abc]\\\\q{  }]', flags + 'v')"
                ],
                invalid => [
                    {
                        code => "var foo = /bar  baz/;",
                        output => "var foo = /bar {2}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /bar    baz/;",
                        output => "var foo = /bar {4}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = / a b  c d /;",
                        output => "var foo = / a b {2}c d /;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp(' a b c d  ');",
                        output => "var foo = RegExp(' a b c d {2}');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => "CallExpression"
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('bar    baz');",
                        output => "var foo = RegExp('bar {4}baz');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => "CallExpression"
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('bar    baz');",
                        output => "var foo = new RegExp('bar {4}baz');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => NewExpression
                            }
                        ]
                    },
                    {

                        // `RegExp` is not shadowed in the scope where it's called
                        code => "{ let RegExp = function() {}; } var foo = RegExp('bar    baz');",
                        output => "{ let RegExp = function() {}; } var foo = RegExp('bar {4}baz');",
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => "CallExpression"
                            }
                        ]
                    },
                    {
                        code => "var foo = /bar   {3}baz/;",
                        output => "var foo = /bar {2} {3}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /bar    ?baz/;",
                        output => "var foo = /bar {3} ?baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "3" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('bar   *baz')",
                        output => "var foo = new RegExp('bar {2} *baz')",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('bar   +baz')",
                        output => "var foo = RegExp('bar {2} +baz')",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => "CallExpression"
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('bar    ');",
                        output => "var foo = new RegExp('bar {4}');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = /bar\\  baz/;",
                        output => "var foo = /bar\\ {2}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /[   ]  /;",
                        output => "var foo = /[   ] {2}/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /  [   ] /;",
                        output => "var foo = / {2}[   ] /;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('[   ]  ');",
                        output => "var foo = new RegExp('[   ] {2}');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('  [ ]');",
                        output => "var foo = RegExp(' {2}[ ]');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => "CallExpression"
                            }
                        ]
                    },
                    {
                        code => "var foo = /\\[  /;",
                        output => "var foo = /\\[ {2}/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /\\[  \\]/;",
                        output => "var foo = /\\[ {2}\\]/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /(?:  )/;",
                        output => "var foo = /(?: {2})/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('^foo(?=   )');",
                        output => "var foo = RegExp('^foo(?= {3})');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "3" },
                                type => "CallExpression"
                            }
                        ]
                    },
                    {
                        code => "var foo = /\\  /",
                        output => "var foo = /\\ {2}/",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = / \\  /",
                        output => "var foo = / \\ {2}/",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },

                    // report only the first occurrence of consecutive spaces
                    {
                        code => "var foo = /  foo   /;",
                        output => "var foo = / {2}foo   /;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },

                    // don't fix strings with escape sequences
                    {
                        code => "var foo = new RegExp('\\\\d  ')",
                        output => None,
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('\\u0041   ')",
                        output => None,
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "3" },
                                type => "CallExpression"
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('\\\\[  \\\\]');",
                        output => None,
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },

                    // ES2024
                    {
                        code => "var foo = /[[    ]    ]    /v;",
                        output => "var foo = /[[    ]    ] {4}/v;",
                        environment => {
                            ecma_version => 2024
                        },
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('[[    ]    ]    ', 'v');",
                        output => "var foo = new RegExp('[[    ]    ] {4}', 'v');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => NewExpression
                            }
                        ]
                    }
                ]
            },
        )
    }
}
