use std::sync::Arc;

use once_cell::sync::Lazy;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter_grep::RopeOrSlice, Rule};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    skip_comments: bool,
    skip_strings: bool,
    skip_templates: bool,
    skip_reg_exps: bool,
    skip_jsx_text: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            skip_strings: true,
            skip_comments: Default::default(),
            skip_templates: Default::default(),
            skip_reg_exps: Default::default(),
            skip_jsx_text: Default::default(),
        }
    }
}

static ALL_IRREGULARS: Lazy<regex::bytes::Regex> = Lazy::new(|| {
    regex::bytes::Regex::new(r#"[\f\v\u0085\ufeff\u00a0\u1680\u180e\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u200b\u202f\u205f\u3000\u2028\u2029]"#).unwrap()
});

pub fn no_irregular_whitespace_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-irregular-whitespace",
        languages => [Javascript],
        messages => [
            no_irregular_whitespace => "Irregular whitespace not allowed.",
        ],
        options_type => Options,
        state => {
            [per-config]
            skip_comments: bool = options.skip_comments,
            skip_strings: bool = options.skip_strings,
            skip_templates: bool = options.skip_templates,
            skip_reg_exps: bool = options.skip_reg_exps,
            skip_jsx_text: bool = options.skip_jsx_text,

            [per-file-run]
            were_there_any_matches_in_the_file: Option<bool>,
        },
        listeners => [
            r#"
              (program) @c
            "# => |node, context| {
                if !match context.file_run_context.file_contents {
                    RopeOrSlice::Slice(slice) => {
                        ALL_IRREGULARS.is_match(slice)
                    }
                    RopeOrSlice::Rope(rope) => {
                        rope.chunks().any(|chunk| ALL_IRREGULARS.is_match(chunk.as_bytes()))
                    }
                } {
                    self.were_there_any_matches_in_the_file = Some(false);
                    return;
                }

                self.were_there_any_matches_in_the_file = Some(true);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::Program;

    #[test]
    fn test_no_irregular_whitespace_rule() {
        let expected_errors = [RuleTestExpectedErrorBuilder::default()
            .message_id("no_irregular_whitespace")
            .type_(Program)
            .build()
            .unwrap()];
        let expected_comment_errors = [RuleTestExpectedErrorBuilder::default()
            .message_id("no_irregular_whitespace")
            .type_(Program)
            .line(1)
            .column(4)
            .build()
            .unwrap()];

        RuleTester::run(
            no_irregular_whitespace_rule(),
            rule_tests! {
                valid => [
                    "'\\u000B';",
                    "'\\u000C';",
                    "'\\u0085';",
                    "'\\u00A0';",
                    "'\\u180E';",
                    "'\\ufeff';",
                    "'\\u2000';",
                    "'\\u2001';",
                    "'\\u2002';",
                    "'\\u2003';",
                    "'\\u2004';",
                    "'\\u2005';",
                    "'\\u2006';",
                    "'\\u2007';",
                    "'\\u2008';",
                    "'\\u2009';",
                    "'\\u200A';",
                    "'\\u200B';",
                    "'\\u2028';",
                    "'\\u2029';",
                    "'\\u202F';",
                    "'\\u205f';",
                    "'\\u3000';",
                    "'\u{000B}';",
                    "'\u{000C}';",
                    "'\u{0085}';",
                    "'\u{00A0}';",
                    "'\u{180E}';",
                    "'\u{feff}';",
                    "'\u{2000}';",
                    "'\u{2001}';",
                    "'\u{2002}';",
                    "'\u{2003}';",
                    "'\u{2004}';",
                    "'\u{2005}';",
                    "'\u{2006}';",
                    "'\u{2007}';",
                    "'\u{2008}';",
                    "'\u{2009}';",
                    "'\u{200A}';",
                    "'\u{200B}';",
                    "'\\\u{2028}';", // multiline string
                    "'\\\u{2029}';", // multiline string
                    "'\u{202F}';",
                    "'\u{205f}';",
                    "'\u{3000}';",
                    { code => "// \u{000B}", options => { skip_comments => true } },
                    { code => "// \u{000C}", options => { skip_comments => true } },
                    { code => "// \u{0085}", options => { skip_comments => true } },
                    { code => "// \u{00A0}", options => { skip_comments => true } },
                    { code => "// \u{180E}", options => { skip_comments => true } },
                    { code => "// \u{feff}", options => { skip_comments => true } },
                    { code => "// \u{2000}", options => { skip_comments => true } },
                    { code => "// \u{2001}", options => { skip_comments => true } },
                    { code => "// \u{2002}", options => { skip_comments => true } },
                    { code => "// \u{2003}", options => { skip_comments => true } },
                    { code => "// \u{2004}", options => { skip_comments => true } },
                    { code => "// \u{2005}", options => { skip_comments => true } },
                    { code => "// \u{2006}", options => { skip_comments => true } },
                    { code => "// \u{2007}", options => { skip_comments => true } },
                    { code => "// \u{2008}", options => { skip_comments => true } },
                    { code => "// \u{2009}", options => { skip_comments => true } },
                    { code => "// \u{200A}", options => { skip_comments => true } },
                    { code => "// \u{200B}", options => { skip_comments => true } },
                    { code => "// \u{202F}", options => { skip_comments => true } },
                    { code => "// \u{205f}", options => { skip_comments => true } },
                    { code => "// \u{3000}", options => { skip_comments => true } },
                    { code => "/* \u{000B} */", options => { skip_comments => true } },
                    { code => "/* \u{000C} */", options => { skip_comments => true } },
                    { code => "/* \u{0085} */", options => { skip_comments => true } },
                    { code => "/* \u{00A0} */", options => { skip_comments => true } },
                    { code => "/* \u{180E} */", options => { skip_comments => true } },
                    { code => "/* \u{feff} */", options => { skip_comments => true } },
                    { code => "/* \u{2000} */", options => { skip_comments => true } },
                    { code => "/* \u{2001} */", options => { skip_comments => true } },
                    { code => "/* \u{2002} */", options => { skip_comments => true } },
                    { code => "/* \u{2003} */", options => { skip_comments => true } },
                    { code => "/* \u{2004} */", options => { skip_comments => true } },
                    { code => "/* \u{2005} */", options => { skip_comments => true } },
                    { code => "/* \u{2006} */", options => { skip_comments => true } },
                    { code => "/* \u{2007} */", options => { skip_comments => true } },
                    { code => "/* \u{2008} */", options => { skip_comments => true } },
                    { code => "/* \u{2009} */", options => { skip_comments => true } },
                    { code => "/* \u{200A} */", options => { skip_comments => true } },
                    { code => "/* \u{200B} */", options => { skip_comments => true } },
                    { code => "/* \u{2028} */", options => { skip_comments => true } },
                    { code => "/* \u{2029} */", options => { skip_comments => true } },
                    { code => "/* \u{202F} */", options => { skip_comments => true } },
                    { code => "/* \u{205f} */", options => { skip_comments => true } },
                    { code => "/* \u{3000} */", options => { skip_comments => true } },
                    { code => "/\u{000B}/", options => { skip_reg_exps => true } },
                    { code => "/\u{000C}/", options => { skip_reg_exps => true } },
                    { code => "/\u{0085}/", options => { skip_reg_exps => true } },
                    { code => "/\u{00A0}/", options => { skip_reg_exps => true } },
                    { code => "/\u{180E}/", options => { skip_reg_exps => true } },
                    { code => "/\u{feff}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2000}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2001}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2002}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2003}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2004}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2005}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2006}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2007}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2008}/", options => { skip_reg_exps => true } },
                    { code => "/\u{2009}/", options => { skip_reg_exps => true } },
                    { code => "/\u{200A}/", options => { skip_reg_exps => true } },
                    { code => "/\u{200B}/", options => { skip_reg_exps => true } },
                    { code => "/\u{202F}/", options => { skip_reg_exps => true } },
                    { code => "/\u{205f}/", options => { skip_reg_exps => true } },
                    { code => "/\u{3000}/", options => { skip_reg_exps => true } },
                    { code => "`\u{000B}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{000C}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{0085}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{00A0}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{180E}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{feff}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2000}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2001}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2002}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2003}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2004}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2005}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2006}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2007}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2008}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{2009}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{200A}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{200B}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{202F}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{205f}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "`\u{3000}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },

                    { code => "`\u{3000}${foo}\u{3000}`", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "const error = ` \u{3000} `;", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "const error = `\n\u{3000}`;", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "const error = `\u{3000}\n`;", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "const error = `\n\u{3000}\n`;", options => { skip_templates => true }, environment => { ecma_version => 6 } },
                    { code => "const error = `foo\u{3000}bar\nfoo\u{3000}bar`;", options => { skip_templates => true }, environment => { ecma_version => 6 } },

                    { code => "<div>\u{000B}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{000C}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{0085}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{00A0}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{180E}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{feff}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2000}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2001}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2002}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2003}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2004}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2005}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2006}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2007}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2008}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{2009}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{200A}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{200B}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{202F}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{205f}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },
                    { code => "<div>\u{3000}</div>;", options => { skip_jsx_text => true }, environment => { ecma_features => { jsx => true } } },

                    // Unicode BOM.
                    "\u{FEFF}console.log('hello BOM');"
                ],
                invalid => [
                    {
                        code => "var any \u{000B} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{000C} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{00A0} = 'thing';",
                        errors => expected_errors
                    },

                    /*
                     * it was moved out of General_Category=Zs (separator, space) in Unicode 6.3.0, so should not be considered a whitespace character.
                     * https://codeblog.jonskeet.uk/2014/12/01/when-is-an-identifier-not-an-identifier-attack-of-the-mongolian-vowel-separator/
                     * {
                     *     code => "var any \u180E = 'thing';",
                     *     errors => expected_errors
                     * },
                     */
                    {
                        code => "var any \u{feff} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2000} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2001} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2002} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2003} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2004} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2005} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2006} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2007} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2008} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2009} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{200A} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2028} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{2029} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{202F} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{205f} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var any \u{3000} = 'thing';",
                        errors => expected_errors
                    },
                    {
                        code => "var a = 'b',\u{2028}c = 'd',\ne = 'f'\u{2028}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 13
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 3,
                                column => 8
                            }
                        ]
                    },
                    {
                        code => "var any \u{3000} = 'thing', other \u{3000} = 'thing';\nvar third \u{3000} = 'thing';",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 9
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 28
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 11
                            }
                        ]
                    },
                    {
                        code => "// \u{000B}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{000C}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{0085}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{00A0}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{180E}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{feff}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2000}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2001}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2002}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2003}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2004}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2005}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2006}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2007}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2008}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{2009}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{200A}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{200B}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{202F}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{205f}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "// \u{3000}",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{000B} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{000C} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{0085} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{00A0} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{180E} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{feff} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2000} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2001} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2002} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2003} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2004} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2005} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2006} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2007} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2008} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2009} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{200A} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{200B} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2028} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{2029} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{202F} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{205f} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "/* \u{3000} */",
                        errors => expected_comment_errors
                    },
                    {
                        code => "var any = /\u{3000}/, other = /\u{000B}/;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 12
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 25
                            }
                        ]
                    },
                    {
                        code => "var any = '\u{3000}', other = '\u{000B}';",
                        options => { skip_strings => false },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 12
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 25
                            }
                        ]
                    },
                    {
                        code => "var any = `\u{3000}`, other = `\u{000B}`;",
                        options => { skip_templates => false },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 12
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 25
                            }
                        ]
                    },
                    {
                        code => "`something ${\u{3000} 10} another thing`",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 14
                            }
                        ]
                    },
                    {
                        code => "`something ${10\u{3000}} another thing`",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 16
                            }
                        ]
                    },
                    {
                        code => "\u{3000}\n`\u{3000}template`",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "\u{3000}\n`\u{3000}multiline\ntemplate`",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "\u{3000}`\u{3000}template`",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "\u{3000}`\u{3000}multiline\ntemplate`",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "`\u{3000}template`\u{3000}",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 12
                            }
                        ]
                    },
                    {
                        code => "`\u{3000}multiline\ntemplate`\u{3000}",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 10
                            }
                        ]
                    },
                    {
                        code => "`\u{3000}template`\n\u{3000}",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "`\u{3000}multiline\ntemplate`\n\u{3000}",
                        options => { skip_templates => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 3,
                                column => 1
                            }
                        ]
                    },

                    // full location tests
                    {
                        code => "var foo = \u{000B} bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            }
                        ]
                    },
                    {
                        code => "var foo =\u{000B}bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 10,
                                end_line => 1,
                                end_column => 11
                            }
                        ]
                    },
                    {
                        code => "var foo = \u{000B}\u{000B} bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 13
                            }
                        ]
                    },
                    {
                        code => "var foo = \u{000B}\u{000C} bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 13
                            }
                        ]
                    },
                    {
                        code => "var foo = \u{000B} \u{000B} bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 13,
                                end_line => 1,
                                end_column => 14
                            }
                        ]
                    },
                    {
                        code => "var foo = \u{000B}bar\u{000B};",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 15,
                                end_line => 1,
                                end_column => 16
                            }
                        ]
                    },
                    {
                        code => "\u{000B}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 1,
                                end_line => 1,
                                end_column => 2
                            }
                        ]
                    },
                    {
                        code => "\u{00A0}\u{2002}\u{2003}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 1,
                                end_line => 1,
                                end_column => 4
                            }
                        ]
                    },
                    {
                        code => "var foo = \u{000B}\nbar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            }
                        ]
                    },
                    {
                        code => "var foo =\u{000B}\n\u{000B}bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 10,
                                end_line => 1,
                                end_column => 11
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 1,
                                end_line => 2,
                                end_column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = \u{000C}\u{000B}\n\u{000C}\u{000B}\u{000C}bar\n;\u{000B}\u{000C}\n",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 13
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 1,
                                end_line => 2,
                                end_column => 4
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 3,
                                column => 2,
                                end_line => 3,
                                end_column => 4
                            }
                        ]
                    },
                    {
                        code => "var foo = \u{2028}bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 11,
                                end_line => 2,
                                end_column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo =\u{2029} bar;",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 10,
                                end_line => 2,
                                end_column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = bar;\u{2028}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 15,
                                end_line => 2,
                                end_column => 1
                            }
                        ]
                    },
                    {
                        code => "\u{2029}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 1,
                                end_line => 2,
                                end_column => 1
                            }
                        ]
                    },
                    {
                        code => "foo\u{2028}\u{2028}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 4,
                                end_line => 2,
                                end_column => 1
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 1,
                                end_line => 3,
                                end_column => 1
                            }
                        ]
                    },
                    {
                        code => "foo\u{2029}\u{2028}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 4,
                                end_line => 2,
                                end_column => 1
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 1,
                                end_line => 3,
                                end_column => 1
                            }
                        ]
                    },
                    {
                        code => "foo\u{2028}\n\u{2028}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 4,
                                end_line => 2,
                                end_column => 1
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 3,
                                column => 1,
                                end_line => 4,
                                end_column => 1
                            }
                        ]
                    },
                    {
                        code => "foo\u{000B}\u{2028}\u{000B}",
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 4,
                                end_line => 1,
                                end_column => 5
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 5,
                                end_line => 2,
                                end_column => 1
                            },
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 2,
                                column => 1,
                                end_line => 2,
                                end_column => 2
                            }
                        ]
                    },
                    {
                        code => "<div>\u{000B}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{000C}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{0085}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{00A0}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{180E}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{feff}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2000}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2001}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2002}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2003}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2004}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2005}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2006}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2007}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2008}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{2009}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{200A}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{200B}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{202F}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{205f}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "<div>\u{3000}</div>;",
                        environment => {
                            ecma_features => {
                                jsx => true
                            }
                        },
                        errors => [
                            {
                                message_id => "no_irregular_whitespace",
                                type => Program,
                                line => 1,
                                column => 6
                            }
                        ]
                    }
                ]
            },
        )
    }
}
