use std::sync::Arc;

use regex::Regex;
use serde::Deserialize;
use squalid::{regex, NonEmpty};
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_constructor_flags: Option<Vec<char>>,
}

pub fn no_invalid_regexp_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-invalid-regexp",
        languages => [Javascript],
        messages => [
            regex_message => "{{message}}.",
        ],
        options_type => Options,
        state => {
            [per-config]
            allowed_flags: Option<Regex> = options.allow_constructor_flags.as_ref().map(|allow_constructor_flags| {
                let allow_constructor_flags = allow_constructor_flags.into_iter().collect::<String>();
                regex!(r#"[dgimsuvy]"#).replace_all(&allow_constructor_flags, "").into_owned()
            }).non_empty().map(|allow_constructor_flags| {
                Regex::new(&format!("(?i)[{}]", allow_constructor_flags)).unwrap()
            }),
        },
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
use crate::kind::NewExpression;

    #[test]
    fn test_no_invalid_regexp_rule() {
        RuleTester::run(
            no_invalid_regexp_rule(),
            rule_tests! {
                valid => [
                    "RegExp('')",
                    "RegExp()",
                    "RegExp('.', 'g')",
                    "new RegExp('.')",
                    "new RegExp",
                    "new RegExp('.', 'im')",
                    "global.RegExp('\\\\')",
                    "new RegExp('.', y)",
                    "new RegExp('.', 'y')",
                    "new RegExp('.', 'u')",
                    "new RegExp('.', 'yu')",
                    "new RegExp('/', 'yu')",
                    "new RegExp('\\/', 'yu')",
                    "new RegExp('\\\\u{65}', 'u')",
                    "new RegExp('\\\\u{65}*', 'u')",
                    "new RegExp('[\\\\u{0}-\\\\u{1F}]', 'u')",
                    "new RegExp('.', 's')",
                    "new RegExp('(?<=a)b')",
                    "new RegExp('(?<!a)b')",
                    "new RegExp('(?<a>b)\\k<a>')",
                    "new RegExp('(?<a>b)\\k<a>', 'u')",
                    "new RegExp('\\\\p{Letter}', 'u')",

                    // unknown flags
                    "RegExp('{', flags)", // valid without the "u" flag
                    "new RegExp('{', flags)", // valid without the "u" flag
                    "RegExp('\\\\u{0}*', flags)", // valid with the "u" flag
                    "new RegExp('\\\\u{0}*', flags)", // valid with the "u" flag
                    {
                        code => "RegExp('{', flags)", // valid without the "u" flag
                        options => { allow_constructor_flags => ["u"] }
                    },
                    {
                        code => "RegExp('\\\\u{0}*', flags)", // valid with the "u" flag
                        options => { allow_constructor_flags => ["a"] }
                    },

                    // unknown pattern
                    "new RegExp(pattern, 'g')",
                    "new RegExp('.' + '', 'g')",
                    "new RegExp(pattern, '')",
                    "new RegExp(pattern)",

                    // ES2020
                    "new RegExp('(?<\\\\ud835\\\\udc9c>.)', 'g')",
                    "new RegExp('(?<\\\\u{1d49c}>.)', 'g')",
                    "new RegExp('(?<𝒜>.)', 'g');",
                    "new RegExp('\\\\p{Script=Nandinagari}', 'u');",

                    // ES2022
                    "new RegExp('a+(?<Z>z)?', 'd')",
                    "new RegExp('\\\\p{Script=Cpmn}', 'u')",
                    "new RegExp('\\\\p{Script=Cypro_Minoan}', 'u')",
                    "new RegExp('\\\\p{Script=Old_Uyghur}', 'u')",
                    "new RegExp('\\\\p{Script=Ougr}', 'u')",
                    "new RegExp('\\\\p{Script=Tangsa}', 'u')",
                    "new RegExp('\\\\p{Script=Tnsa}', 'u')",
                    "new RegExp('\\\\p{Script=Toto}', 'u')",
                    "new RegExp('\\\\p{Script=Vith}', 'u')",
                    "new RegExp('\\\\p{Script=Vithkuqi}', 'u')",

                    // ES2024
                    "new RegExp('[A--B]', 'v')",
                    "new RegExp('[A&&B]', 'v')",
                    "new RegExp('[A--[0-9]]', 'v')",
                    "new RegExp('[\\\\p{Basic_Emoji}--\\\\q{a|bc|def}]', 'v')",
                    "new RegExp('[A--B]', flags)", // valid only with `v` flag
                    "new RegExp('[[]\\\\u{0}*', flags)", // valid only with `u` flag

                    // allowConstructorFlags
                    {
                        code => "new RegExp('.', 'g')",
                        options => { allow_constructor_flags => [] }
                    },
                    {
                        code => "new RegExp('.', 'g')",
                        options => { allow_constructor_flags => ["a"] }
                    },
                    {
                        code => "new RegExp('.', 'a')",
                        options => { allow_constructor_flags => ["a"] }
                    },
                    {
                        code => "new RegExp('.', 'ag')",
                        options => { allow_constructor_flags => ["a"] }
                    },
                    {
                        code => "new RegExp('.', 'ga')",
                        options => { allow_constructor_flags => ["a"] }
                    },
                    {
                        code => "new RegExp(pattern, 'ga')",
                        options => { allow_constructor_flags => ["a"] }
                    },
                    {
                        code => "new RegExp('.' + '', 'ga')",
                        options => { allow_constructor_flags => ["a"] }
                    },
                    {
                        code => "new RegExp('.', 'a')",
                        options => { allow_constructor_flags => ["a", "z"] }
                    },
                    {
                        code => "new RegExp('.', 'z')",
                        options => { allow_constructor_flags => ["a", "z"] }
                    },
                    {
                        code => "new RegExp('.', 'az')",
                        options => { allow_constructor_flags => ["a", "z"] }
                    },
                    {
                        code => "new RegExp('.', 'za')",
                        options => { allow_constructor_flags => ["a", "z"] }
                    },
                    {
                        code => "new RegExp('.', 'agz')",
                        options => { allow_constructor_flags => ["a", "z"] }
                    }
                ],
                invalid => [
                    {
                        code => "RegExp('[');",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /[/: Unterminated character class" },
                            type => "CallExpression"
                        }]
                    },
                    {
                        code => "RegExp('.', 'z');",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'z'" },
                            type => "CallExpression"
                        }]
                    },
                    {
                        code => "RegExp('.', 'a');",
                        options => {},
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'a'" },
                            type => "CallExpression"
                        }]
                    },
                    {
                        code => "new RegExp('.', 'a');",
                        options => { allow_constructor_flags => [] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'a'" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp('.', 'z');",
                        options => { allow_constructor_flags => ["a"] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'z'" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp('.', 'az');",
                        options => { allow_constructor_flags => ["z"] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'a'" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp(')');",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /)/: Unmatched ')'" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => r#"new RegExp('\\a', 'u');"#,
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\a/u: Invalid escape" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => r#"new RegExp('\\a', 'u');"#,
                        options => { allow_constructor_flags => ["u"] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\a/u: Invalid escape" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => r#"RegExp('\\u{0}*');"#,
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\u{0}*/: Nothing to repeat" },
                            type => "CallExpression"
                        }]
                    },
                    {
                        code => r#"new RegExp('\\u{0}*');"#,
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\u{0}*/: Nothing to repeat" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => r#"new RegExp('\\u{0}*', '');"#,
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\u{0}*/: Nothing to repeat" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => r#"new RegExp('\\u{0}*', 'a');"#,
                        options => { allow_constructor_flags => ["a"] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\u{0}*/: Nothing to repeat" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => r#"RegExp('\\u{0}*');"#,
                        options => { allow_constructor_flags => ["a"] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\u{0}*/: Nothing to repeat" },
                            type => "CallExpression"
                        }]
                    },

                    // https://github.com/eslint/eslint/issues/10861
                    {
                        code => r#"new RegExp('\\');"#,
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /\\/: \\ at end of pattern" },
                            type => NewExpression
                        }]
                    },

                    // https://github.com/eslint/eslint/issues/16573
                    {
                        code => "RegExp(')' + '', 'a');",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'a'" },
                            type => "CallExpression"
                        }]
                    },
                    {
                        code => "new RegExp('.' + '', 'az');",
                        options => { allow_constructor_flags => ["z"] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'a'" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp(pattern, 'az');",
                        options => { allow_constructor_flags => ["a"] },
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid flags supplied to RegExp constructor 'z'" },
                            type => NewExpression
                        }]
                    },

                    // ES2024
                    {
                        code => "new RegExp('[[]', 'v');",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /[[]/v: Unterminated character class" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp('.', 'uv');",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Regex 'u' and 'v' flags cannot be used together" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp(pattern, 'uv');",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Regex 'u' and 'v' flags cannot be used together" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp('[A--B]' /* valid only with `v` flag */, 'u')",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /[A--B]/u: Range out of order in character class" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new RegExp('[[]\\\\u{0}*' /* valid only with `u` flag */, 'v')",
                        errors => [{
                            message_id => "regex_message",
                            data => { message => "Invalid regular expression: /[[]\\u{0}*/v: Unterminated character class" },
                            type => NewExpression
                        }]
                    }
                ]
            },
        )
    }
}
