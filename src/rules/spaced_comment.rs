use std::sync::Arc;
use serde::Deserialize;

use tree_sitter_lint::{rule, violation, Rule};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Always {
    #[default]
    Always,
    Never,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionsVariants {
    EmptyList(),
    JustAlways([Always; 1]),
    AlwaysAndOptionsObject(Always, OptionsObject),
}

impl Default for OptionsVariants {
    fn default() -> Self {
        Self::EmptyList()
    }
}

#[derive(Default, Deserialize)]
struct PerCommentTypeOptions {
    exceptions: Option<Vec<String>>,
    markers: Option<Vec<String>>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct OptionsObject {
    exceptions: Vec<String>,
    markers: Vec<String>,
    line: Option<PerCommentTypeOptions>,
    block: Option<PerCommentTypeOptions>,
}

struct Options {
    always: Always,
    exceptions: Vec<String>,
    markers: Vec<String>,
    line: Option<PerCommentTypeOptions>,
    block: Option<PerCommentTypeOptions>,
}

impl Options {
    pub fn from_always_and_options_object(
        always: Always,
        options_object: OptionsObject,
    ) -> Self {
        Self {
            always,
            exceptions: options_object.exceptions,
            markers: options_object.markers,
            line: options_object.line,
            block: options_object.block,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        OptionsVariants::default().into()
    }
}

impl From<OptionsVariants> for Options {
    fn from(value: OptionsVariants) -> Self {
        match value {
            OptionsVariants::EmptyList() => {
                Self::from_always_and_options_object(Default::default(), Default::default())
            }
            OptionsVariants::JustAlways(always) => {
                Self::from_always_and_options_object(always[0], Default::default())
            }
            OptionsVariants::AlwaysAndOptionsObject(always, options_object) => {
                Self::from_always_and_options_object(always, options_object)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Options {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(OptionsVariants::deserialize(deserializer)?.into())
    }
}

pub fn spaced_comment_rule() -> Arc<dyn Rule> {
    rule! {
        name => "spaced-comment",
        languages => [Javascript],
        messages => [
            unexpected_space_after_marker => "Unexpected space or tab after marker ({{ref_char}}) in comment.",
            expected_exception_after => "Expected exception block, space or tab after '{{ref_char}}' in comment.",
            unexpected_space_before => "Unexpected space or tab before '*/' in comment.",
            unexpected_space_after => "Unexpected space or tab after '{{ref_char}}' in comment.",
            expected_space_before => "Expected space or tab before '*/' in comment.",
            expected_space_after => "Expected space or tab after '{{ref_char}}' in comment.",
        ],
        options_type => Options,
        listeners => [
            r#"(
              (debugger_statement) @c
            )"# => |node, context| {
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_spaced_comment_rule() {
        RuleTester::run(
            spaced_comment_rule(),
            rule_tests! {
                valid => [
                    {
                        code => "// A valid comment starting with space\nvar a = 1;",
                        options => ["always"]
                    },
                    {
                        code => "//   A valid comment starting with tab\nvar a = 1;",
                        options => ["always"]
                    },
                    {
                        code => "//A valid comment NOT starting with space\nvar a = 2;",
                        options => ["never"]
                    },

                    // exceptions - line comments
                    {
                        code => "//-----------------------\n// A comment\n//-----------------------",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"]
                        }]
                    },
                    {
                        code => "//-----------------------\n// A comment\n//-----------------------",
                        options => ["always", {
                            line => { exceptions => ["-", "=", "*", "#", "!@#"] }
                        }]
                    },
                    {
                        code => "//===========\n// A comment\n//*************",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"]
                        }]
                    },
                    {
                        code => "//######\n// A comment",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"]
                        }]
                    },
                    {
                        code => "//!@#!@#!@#\n// A comment\n//!@#",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"]
                        }]
                    },

                    // exceptions - block comments
                    {
                        code => "var a = 1; /*######*/",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"]
                        }]
                    },
                    {
                        code => "var a = 1; /*######*/",
                        options => ["always", {
                            block => { exceptions => ["-", "=", "*", "#", "!@#"] }
                        }]
                    },
                    {
                        code => "/*****************\n * A comment\n *****************/",
                        options => ["always", {
                            exceptions => ["*"]
                        }]
                    },
                    {
                        code => "/*++++++++++++++\n * A comment\n +++++++++++++++++*/",
                        options => ["always", {
                            exceptions => ["+"]
                        }]
                    },
                    {
                        code => "/*++++++++++++++\n + A comment\n * B comment\n - C comment\n----------------*/",
                        options => ["always", {
                            exceptions => ["+", "-"]
                        }]
                    },

                    // markers - line comments
                    {
                        code => "//!< docblock style comment",
                        options => ["always", {
                            markers => ["/", "!<"]
                        }]
                    },
                    {
                        code => "//!< docblock style comment",
                        options => ["always", {
                            line => { markers => ["/", "!<"] }
                        }]
                    },
                    {
                        code => "//----\n// a comment\n//----\n/// xmldoc style comment\n//!< docblock style comment",
                        options => ["always", {
                            exceptions => ["-"],
                            markers => ["/", "!<"]
                        }]
                    },
                    {
                        code => "/*\u2028x*/",
                        options => ["always", {
                            markers => ["/", "!<"]
                        }]
                    },
                    {
                        code => "///xmldoc style comment",
                        options => ["never", {
                            markers => ["/", "!<"]
                        }]
                    },

                    // markers - block comments
                    {
                        code => "var a = 1; /*# This is an example of a marker in a block comment\nsubsequent lines do not count*/",
                        options => ["always", {
                            markers => ["#"]
                        }]
                    },
                    {
                        code => "/*!\n *comment\n */",
                        options => ["always", { markers => ["!"] }]
                    },
                    {
                        code => "/*!\n *comment\n */",
                        options => ["always", { block => { markers => ["!"] } }]
                    },
                    {
                        code => "/**\n *jsdoc\n */",
                        options => ["always", { markers => ["*"] }]
                    },
                    {
                        code => "/*global ABC*/",
                        options => ["always", { markers => ["global"] }]
                    },
                    {
                        code => "/*eslint-env node*/",
                        options => ["always", { markers => ["eslint-env"] }]
                    },
                    {
                        code => "/*eslint eqeqeq:0, curly: 2*/",
                        options => ["always", { markers => ["eslint"] }]
                    },
                    {
                        code => "/*eslint-disable no-alert, no-console */\nalert()\nconsole.log()\n/*eslint-enable no-alert */",
                        options => ["always", { markers => ["eslint-enable", "eslint-disable"] }]
                    },

                    // misc. variations
                    {
                        code => validShebangProgram,
                        options => ["always"]
                    },
                    {
                        code => validShebangProgram,
                        options => ["never"]
                    },
                    {
                        code => "//",
                        options => ["always"]
                    },
                    {
                        code => "//\n",
                        options => ["always"]
                    },
                    {
                        code => "// space only at start; valid since balanced doesn't apply to line comments",
                        options => ["always", { block => { balanced => true } }]
                    },
                    {
                        code => "//space only at end; valid since balanced doesn't apply to line comments ",
                        options => ["never", { block => { balanced => true } }]
                    },

                    // block comments
                    {
                        code => "var a = 1; /* A valid comment starting with space */",
                        options => ["always"]
                    },
                    {
                        code => "var a = 1; /*A valid comment NOT starting with space */",
                        options => ["never"]
                    },
                    {
                        code => "function foo(/* height */a) { \n }",
                        options => ["always"]
                    },
                    {
                        code => "function foo(/*height */a) { \n }",
                        options => ["never"]
                    },
                    {
                        code => "function foo(a/* height */) { \n }",
                        options => ["always"]
                    },
                    {
                        code => "/*\n * Test\n */",
                        options => ["always"]
                    },
                    {
                        code => "/*\n *Test\n */",
                        options => ["never"]
                    },
                    {
                        code => "/*     \n *Test\n */",
                        options => ["always"]
                    },
                    {
                        code => "/*\r\n *Test\r\n */",
                        options => ["never"]
                    },
                    {
                        code => "/*     \r\n *Test\r\n */",
                        options => ["always"]
                    },
                    {
                        code => "/**\n *jsdoc\n */",
                        options => ["always"]
                    },
                    {
                        code => "/**\r\n *jsdoc\r\n */",
                        options => ["always"]
                    },
                    {
                        code => "/**\n *jsdoc\n */",
                        options => ["never"]
                    },
                    {
                        code => "/**   \n *jsdoc \n */",
                        options => ["always"]
                    },

                    // balanced block comments
                    {
                        code => "var a = 1; /* comment */",
                        options => ["always", { block => { balanced => true } }]
                    },
                    {
                        code => "var a = 1; /*comment*/",
                        options => ["never", { block => { balanced => true } }]
                    },
                    {
                        code => "function foo(/* height */a) { \n }",
                        options => ["always", { block => { balanced => true } }]
                    },
                    {
                        code => "function foo(/*height*/a) { \n }",
                        options => ["never", { block => { balanced => true } }]
                    },
                    {
                        code => "var a = 1; /*######*/",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"],
                            block => { balanced => true }
                        }]
                    },
                    {
                        code => "/*****************\n * A comment\n *****************/",
                        options => ["always", {
                            exceptions => ["*"],
                            block => { balanced => true }
                        }]
                    },
                    {
                        code => "/*! comment */",
                        options => ["always", { markers => ["!"], block => { balanced => true } }]
                    },
                    {
                        code => "/*!comment*/",
                        options => ["never", { markers => ["!"], block => { balanced => true } }]
                    },
                    {
                        code => "/*!\n *comment\n */",
                        options => ["always", { markers => ["!"], block => { balanced => true } }]
                    },
                    {
                        code => "/*global ABC */",
                        options => ["always", { markers => ["global"], block => { balanced => true } }]
                    },

                    // markers & exceptions
                    {
                        code => "///--------\r\n/// test\r\n///--------",
                        options => ["always", { markers => ["/"], exceptions => ["-"] }]
                    },
                    {
                        code => "///--------\r\n/// test\r\n///--------\r\n/* blah */",
                        options => ["always", { markers => ["/"], exceptions => ["-"], block => { markers => [] } }]
                    },
                    {
                        code => "/***\u2028*/",
                        options => ["always", { exceptions => ["*"] }]
                    },

                    // ignore marker-only comments, https://github.com/eslint/eslint/issues/12036
                    {
                        code => "//#endregion",
                        options => ["always", { line => { markers => ["#endregion"] } }]
                    },
                    {
                        code => "/*foo*/",
                        options => ["always", { block => { markers => ["foo"] } }]
                    },
                    {
                        code => "/*foo*/",
                        options => ["always", { block => { markers => ["foo"], balanced => true } }]
                    },
                    {
                        code => "/*foo*/ /*bar*/",
                        options => ["always", { markers => ["foo", "bar"] }]
                    },
                    {
                        code => "//foo\n//bar",
                        options => ["always", { markers => ["foo", "bar"] }]
                    },
                    {
                        code => "/* foo */",
                        options => ["never", { markers => [" foo "] }]
                    },
                    {
                        code => "// foo ",
                        options => ["never", { markers => [" foo "] }]
                    },
                    {
                        code => "//*", // "*" is a marker by default
                        options => ["always"]
                    },
                    {
                        code => "/***/", // "*" is a marker by default
                        options => ["always"]
                    }
                ],
                invalid => [
                    {
                        code => "//An invalid comment NOT starting with space\nvar a = 1;",
                        output => "// An invalid comment NOT starting with space\nvar a = 1;",
                        options => ["always"],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "//" },
                            type => "Line"
                        }]
                    },
                    {
                        code => "// An invalid comment starting with space\nvar a = 2;",
                        output => "//An invalid comment starting with space\nvar a = 2;",
                        options => ["never"],
                        errors => [{
                            message_id => "unexpected_space_after",
                            data => { ref_char => "//" },
                            type => "Line"
                        }]
                    },
                    {
                        code => "//   An invalid comment starting with tab\nvar a = 2;",
                        output => "//An invalid comment starting with tab\nvar a = 2;",
                        options => ["never"],
                        errors => [{
                            message_id => "unexpected_space_after",
                            data => { ref_char => "//" },
                            type => "Line"
                        }]
                    },
                    {
                        /*
                         * note that the first line in the comment is not a valid exception
                         * block pattern because of the minus sign at the end of the line =>
                         * `//\*********************-`
                         */
                        code => "//*********************-\n// Comment Block 3\n//***********************",
                        output => "//* ********************-\n// Comment Block 3\n//***********************",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"]
                        }],
                        errors => [{
                            message_id => "expectedExceptionAfter",
                            data => { ref_char => "//*" },
                            type => "Line"
                        }]
                    },
                    {
                        code => "//-=-=-=-=-=-=\n// A comment\n//-=-=-=-=-=-=",
                        output => "// -=-=-=-=-=-=\n// A comment\n// -=-=-=-=-=-=",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "#", "!@#"]
                        }],
                        errors => [
                            {
                                message_id => "expectedExceptionAfter",
                                data => { ref_char => "//" },
                                type => "Line"
                            },
                            {
                                message_id => "expectedExceptionAfter",
                                data => { ref_char => "//" },
                                type => "Line"
                            }
                        ]
                    },
                    {
                        code => "//!<docblock style comment",
                        output => "//!< docblock style comment",
                        options => ["always", {
                            markers => ["/", "!<"]
                        }],
                        errors => 1
                    },
                    {
                        code => "//!< docblock style comment",
                        output => "//!<docblock style comment",
                        options => ["never", {
                            markers => ["/", "!<"]
                        }],
                        errors => 1
                    },
                    {
                        code => "var a = 1; /* A valid comment starting with space */",
                        output => "var a = 1; /*A valid comment starting with space */",
                        options => ["never"],
                        errors => [{
                            message_id => "unexpected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "var a = 1; /*######*/",
                        output => "var a = 1; /* ######*/",
                        options => ["always", {
                            exceptions => ["-", "=", "*", "!@#"]
                        }],
                        errors => [{
                            message_id => "expectedExceptionAfter",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "var a = 1; /*A valid comment NOT starting with space */",
                        output => "var a = 1; /* A valid comment NOT starting with space */",
                        options => ["always"],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "function foo(/* height */a) { \n }",
                        output => "function foo(/*height */a) { \n }",
                        options => ["never"],
                        errors => [{
                            message_id => "unexpected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "function foo(/*height */a) { \n }",
                        output => "function foo(/* height */a) { \n }",
                        options => ["always"],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "function foo(a/*height */) { \n }",
                        output => "function foo(a/* height */) { \n }",
                        options => ["always"],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "/*     \n *Test\n */",
                        output => "/*\n *Test\n */",
                        options => ["never"],
                        errors => [{
                            message_id => "unexpected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "//-----------------------\n// A comment\n//-----------------------",
                        output => "// -----------------------\n// A comment\n// -----------------------",
                        options => ["always", {
                            block => { exceptions => ["-", "=", "*", "#", "!@#"] }
                        }],
                        errors => [
                            { message_id => "expected_space_after", data => { ref_char => "//" }, type => "Line" },
                            { message_id => "expected_space_after", data => { ref_char => "//" }, type => "Line" }
                        ]
                    },
                    {
                        code => "var a = 1; /*######*/",
                        output => "var a = 1; /* ######*/",
                        options => ["always", {
                            line => { exceptions => ["-", "=", "*", "#", "!@#"] }
                        }],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "//!< docblock style comment",
                        output => "// !< docblock style comment",
                        options => ["always", {
                            block => { markers => ["/", "!<"] }
                        }],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "//" },
                            type => "Line"
                        }]
                    },
                    {
                        code => "/*!\n *comment\n */",
                        output => "/* !\n *comment\n */",
                        options => ["always", { line => { markers => ["!"] } }],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "///--------\r\n/// test\r\n///--------\r\n/*/ blah *//*-----*/",
                        output => "///--------\r\n/// test\r\n///--------\r\n/* / blah *//*-----*/",
                        options => ["always", { markers => ["/"], exceptions => ["-"], block => { markers => [] } }],
                        errors => [{
                            message_id => "expectedExceptionAfter",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "///--------\r\n/// test\r\n///--------\r\n/*/ blah */ /*-----*/",
                        output => "///--------\r\n/// test\r\n///--------\r\n/* / blah */ /* -----*/",
                        options => ["always", { line => { markers => ["/"], exceptions => ["-"] } }],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock,
                            line => 4,
                            column => 1
                        }, {
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock,
                            line => 4,
                            column => 13
                        }]
                    },

                    // balanced block comments
                    {
                        code => "var a = 1; /* A balanced comment starting with space*/",
                        output => "var a = 1; /* A balanced comment starting with space */",
                        options => ["always", { block => { balanced => true } }],
                        errors => [{
                            message_id => "expected_space_before",
                            data => { ref_char => "/**" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "var a = 1; /*A balanced comment NOT starting with space */",
                        output => "var a = 1; /*A balanced comment NOT starting with space*/",
                        options => ["never", { block => { balanced => true } }],
                        errors => [{
                            message_id => "unexpected_space_before",
                            data => { ref_char => "*/" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "function foo(/* height*/a) { \n }",
                        output => "function foo(/* height */a) { \n }",
                        options => ["always", { block => { balanced => true } }],
                        errors => [{
                            message_id => "expected_space_before",
                            data => { ref_char => "/**" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "function foo(/*height */a) { \n }",
                        output => "function foo(/*height*/a) { \n }",
                        options => ["never", { block => { balanced => true } }],
                        errors => [{
                            message_id => "unexpected_space_before",
                            data => { ref_char => "*/" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "/*! comment*/",
                        output => "/*! comment */",
                        options => ["always", { markers => ["!"], block => { balanced => true } }],
                        errors => [{
                            message_id => "expected_space_before",
                            data => { ref_char => "/**" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "/*!comment */",
                        output => "/*!comment*/",
                        options => ["never", { markers => ["!"], block => { balanced => true } }],
                        errors => [{
                            message_id => "unexpected_space_before",
                            data => { ref_char => "*/" },
                            type => StatementBlock
                        }]
                    },

                    // not a marker-only comment, regression tests for https://github.com/eslint/eslint/issues/12036
                    {
                        code => "//#endregionfoo",
                        output => "//#endregion foo",
                        options => ["always", { line => { markers => ["#endregion"] } }],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "//#endregion" },
                            type => "Line"
                        }]
                    },
                    {
                        code => "/*#endregion*/",
                        output => "/* #endregion*/", // not an allowed marker for block comments
                        options => ["always", { line => { markers => ["#endregion"] } }],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/*" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "/****/",
                        output => "/** **/",
                        options => ["always"],
                        errors => [{
                            message_id => "expected_space_after",
                            data => { ref_char => "/**" },
                            type => StatementBlock
                        }]
                    },
                    {
                        code => "/****/",
                        output => "/** * */",
                        options => ["always", { block => { balanced => true } }],
                        errors => [
                            {
                                message_id => "expected_space_after",
                                data => { ref_char => "/**" },
                                type => StatementBlock
                            },
                            {
                                message_id => "expected_space_before",
                                data => { ref_char => "*/" },
                                type => StatementBlock
                            }
                        ]
                    },
                    {
                        code => "/* foo */",
                        output => "/*foo*/",
                        options => ["never", { block => { markers => ["foo"], balanced => true } }], // not " foo "
                        errors => [
                            {
                                message_id => "unexpected_space_after",
                                data => { ref_char => "/*" },
                                type => StatementBlock
                            },
                            {
                                message_id => "unexpected_space_before",
                                data => { ref_char => "*/" },
                                type => StatementBlock
                            }
                        ]
                    }
                ]
            },
        )
    }
}
