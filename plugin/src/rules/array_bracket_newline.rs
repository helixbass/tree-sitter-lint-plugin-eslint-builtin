use std::sync::Arc;

use itertools::Itertools;
use serde::Deserialize;
use tree_sitter_lint::{
    range_between_end_and_start, rule, tree_sitter::Node, violation, QueryMatchContext, Rule,
    SkipOptionsBuilder,
};

use crate::{
    ast_helpers::{
        get_comma_separated_optional_non_comment_named_children, get_comment_type, CommentType,
    },
    kind::Comment,
    utils::ast_utils,
};

#[derive(Default, Deserialize)]
struct OptionsObject {
    multiline: Option<bool>,
    min_items: Option<usize>,
}

impl OptionsObject {
    pub fn consistent(&self) -> bool {
        false
    }

    pub fn multiline(&self) -> bool {
        self.multiline == Some(true)
    }

    pub fn min_items(&self) -> usize {
        self.min_items.unwrap_or(usize::MAX)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum OptionsEnum {
    Always,
    Never,
    Consistent,
}

impl OptionsEnum {
    pub fn consistent(&self) -> bool {
        matches!(self, Self::Consistent)
    }

    pub fn multiline(&self) -> bool {
        false
    }

    pub fn min_items(&self) -> usize {
        match self {
            Self::Always => 0,
            _ => usize::MAX,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Enum(OptionsEnum),
    Object(OptionsObject),
}

impl Options {
    pub fn consistent(&self) -> bool {
        match self {
            Options::Enum(value) => value.consistent(),
            Options::Object(value) => value.consistent(),
        }
    }

    pub fn multiline(&self) -> bool {
        match self {
            Options::Enum(value) => value.multiline(),
            Options::Object(value) => value.multiline(),
        }
    }

    pub fn min_items(&self) -> usize {
        match self {
            Options::Enum(value) => value.min_items(),
            Options::Object(value) => value.min_items(),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Object(OptionsObject {
            multiline: Some(true),
            min_items: Some(usize::MAX),
        })
    }
}

fn report_no_beginning_linebreak<'a>(
    node: Node<'a>,
    token: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) {
    context.report(violation! {
        node => node,
        range => token.range(),
        message_id => "unexpected_opening_linebreak",
        fix => |fixer| {
            let next_token = context.get_token_after(
                token,
                Some(SkipOptionsBuilder::<fn(Node) -> bool>::default()
                    .include_comments(true)
                    .build().unwrap()
                )
            );

            if ast_utils::is_comment_token(next_token) {
                return;
            }

            fixer.remove_range(
                range_between_end_and_start(
                    token.range(),
                    next_token.range(),
                )
            );
        }
    });
}

fn report_no_ending_linebreak<'a>(
    node: Node<'a>,
    token: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) {
    context.report(violation! {
        node => node,
        range => token.range(),
        message_id => "unexpected_closing_linebreak",
        fix => |fixer| {
            let previous_token = context.get_token_before(
                token,
                Some(SkipOptionsBuilder::<fn(Node) -> bool>::default()
                    .include_comments(true)
                    .build().unwrap()
                )
            );

            if ast_utils::is_comment_token(previous_token) {
                return;
            }

            fixer.remove_range(
                range_between_end_and_start(
                    previous_token.range(),
                    token.range()
                )
            );
        }
    });
}

fn report_required_beginning_linebreak<'a>(
    node: Node<'a>,
    token: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) {
    context.report(violation! {
        node => node,
        range => token.range(),
        message_id => "missing_opening_linebreak",
        fix => |fixer| {
            fixer.insert_text_after(token, "\n");
        }
    });
}

fn report_required_ending_linebreak<'a>(
    node: Node<'a>,
    token: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) {
    context.report(violation! {
        node => node,
        range => token.range(),
        message_id => "missing_closing_linebreak",
        fix => |fixer| {
            fixer.insert_text_before(token, "\n");
        }
    });
}

pub fn array_bracket_newline_rule() -> Arc<dyn Rule> {
    rule! {
        name => "array-bracket-newline",
        languages => [Javascript],
        messages => [
            unexpected_opening_linebreak => "There should be no linebreak after '['.",
            unexpected_closing_linebreak => "There should be no linebreak before ']'.",
            missing_opening_linebreak => "A linebreak is required after '['.",
            missing_closing_linebreak => "A linebreak is required before ']'.",
        ],
        fixable => true,
        allow_self_conflicting_fixes => true,
        options_type => Options,
        state => {
            [per-config]
            consistent: bool = options.consistent(),
            multiline: bool = options.multiline(),
            min_items: usize = options.min_items(),
        },
        listeners => [
            r#"
              (array) @c
              (array_pattern) @c
            "# => |node, context| {
                let elements = get_comma_separated_optional_non_comment_named_children(node).collect_vec();
                let open_bracket = context.get_first_token(node, Option::<fn(Node) -> bool>::None);
                let close_bracket = context.get_last_token(node, Option::<fn(Node) -> bool>::None);
                let first_inc_comment = context.get_token_after(
                    open_bracket,
                    Some(SkipOptionsBuilder::<fn(Node) -> bool>::default()
                        .include_comments(true)
                        .build()
                        .unwrap())
                );
                let last_inc_comment = context.get_token_before(
                    close_bracket,
                    Some(SkipOptionsBuilder::<fn(Node) -> bool>::default()
                        .include_comments(true)
                        .build()
                        .unwrap())
                );
                let first = context.get_token_after(open_bracket, Option::<fn(Node) -> bool>::None);
                let last = context.get_token_before(close_bracket, Option::<fn(Node) -> bool>::None);

                let needs_linebreaks = elements.len() >= self.min_items ||
                    self.multiline && !elements.is_empty() &&
                        first_inc_comment.range().start_point.row !=
                        last_inc_comment.range().end_point.row ||
                    elements.is_empty() &&
                        first_inc_comment.kind() == Comment &&
                        get_comment_type(first_inc_comment, context) == CommentType::Block &&
                        first_inc_comment.range().start_point.row !=
                        last_inc_comment.range().end_point.row &&
                        first_inc_comment == last_inc_comment ||
                    self.consistent &&
                        open_bracket.range().end_point.row !=
                            first.range().start_point.row;

                if needs_linebreaks {
                    if ast_utils::is_token_on_same_line(open_bracket, first) {
                        report_required_beginning_linebreak(node, open_bracket, context);
                    }
                    if ast_utils::is_token_on_same_line(last, close_bracket) {
                        report_required_ending_linebreak(node, close_bracket, context);
                    }
                } else {
                    if !ast_utils::is_token_on_same_line(open_bracket, first) {
                        report_no_beginning_linebreak(node, open_bracket, context);
                    }
                    if !ast_utils::is_token_on_same_line(last, close_bracket) {
                        report_no_ending_linebreak(node, close_bracket, context);
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::{Array, ArrayPattern};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_array_bracket_newline_rule() {
        RuleTester::run(
            array_bracket_newline_rule(),
            rule_tests! {
                valid => [
                    /*
                     * ArrayExpression
                     * "default" { multiline => true }
                     */
                    "var foo = [];",
                    "var foo = [1];",
                    "var foo = /* any comment */[1];",
                    "var foo = /* any comment */\n[1];",
                    "var foo = [1, 2];",
                    "var foo = [ // any comment\n1, 2\n];",
                    "var foo = [\n// any comment\n1, 2\n];",
                    "var foo = [\n1, 2\n// any comment\n];",
                    "var foo = [\n1,\n2\n];",
                    "var foo = [\nfunction foo() {\nreturn dosomething();\n}\n];",
                    "var foo = [/* \nany comment\n */];",
                    "var foo = [/* single line multiline comment for no real reason */];",
                    "var foo = [[1,2]]",

                    // "always"
                    { code => "var foo = [\n];", options => "always" },
                    { code => "var foo = [\n1\n];", options => "always" },
                    { code => "var foo = [\n// any\n1\n];", options => "always" },
                    { code => "var foo = [\n/* any */\n1\n];", options => "always" },
                    { code => "var foo = [\n1, 2\n];", options => "always" },
                    { code => "var foo = [\n1, 2 // any comment\n];", options => "always" },
                    {
                        code => "var foo = [\n1, 2 /* any comment */\n];",
                        options => "always"
                    },
                    { code => "var foo = [\n1,\n2\n];", options => "always" },
                    {
                        code => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => "always"
                    },
                    {
                        code => "
                        var foo = [
                            [
                                1,2
                            ]
                        ]
                        ",
                        options => "always"
                    },
                    {
                        code => "
                        var foo = [
                            0,
                            [
                                1,2
                            ],
                            3
                        ]
                        ",
                        options => "always"
                    },

                    // "never"
                    { code => "var foo = [];", options => "never" },
                    { code => "var foo = [1];", options => "never" },
                    { code => "var foo = [/* any comment */1];", options => "never" },
                    { code => "var foo = [1, 2];", options => "never" },
                    { code => "var foo = [1,\n2];", options => "never" },
                    { code => "var foo = [1,\n/* any comment */\n2];", options => "never" },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        options => "never"
                    },
                    {
                        code => "var foo = [[1,2],3];",
                        options => "never"
                    },

                    // "consistent"
                    { code => "var a = []", options => "consistent" },
                    { code => "var a = [\n]", options => "consistent" },
                    { code => "var a = [1]", options => "consistent" },
                    { code => "var a = [\n1\n]", options => "consistent" },
                    { code => "var a = [//\n1\n]", options => "consistent" },
                    { code => "var a = [/**/\n1\n]", options => "consistent" },
                    { code => "var a = [/*\n*/1\n]", options => "consistent" },
                    { code => "var a = [//\n]", options => "consistent" },
                    {
                        code => "var a = [
                            [1,2]
                        ]",
                        options => "consistent"
                    },
                    {
                        code => "var a = [
                            [[1,2]]
                        ]",
                        options => "consistent"
                    },

                    // { multiline => true }
                    { code => "var foo = [];", options => { multiline => true } },
                    { code => "var foo = [1];", options => { multiline => true } },
                    {
                        code => "var foo = /* any comment */[1];",
                        options => { multiline => true }
                    },
                    {
                        code => "var foo = /* any comment */\n[1];",
                        options => { multiline => true }
                    },
                    { code => "var foo = [1, 2];", options => { multiline => true } },
                    {
                        code => "var foo = [ // any comment\n1, 2\n];",
                        options => { multiline => true }
                    },
                    {
                        code => "var foo = [\n// any comment\n1, 2\n];",
                        options => { multiline => true }
                    },
                    {
                        code => "var foo = [\n1, 2\n// any comment\n];",
                        options => { multiline => true }
                    },
                    { code => "var foo = [\n1,\n2\n];", options => { multiline => true } },
                    {
                        code => "var foo = [\nfunction foo() {\nreturn dosomething();\n}\n];",
                        options => { multiline => true }
                    },
                    {
                        code => "var foo = [/* \nany comment\n */];",
                        options => { multiline => true }
                    },
                    {
                        code => "var foo = [\n1,\n2,\n[3,4],\n];",
                        options => { multiline => true }
                    },
                    {
                        code => "var foo = [\n1,\n2,\n[\n3,\n4\n],\n];",
                        options => { multiline => true }
                    },

                    // { multiline => false }
                    { code => "var foo = [];", options => { multiline => false } },
                    { code => "var foo = [1];", options => { multiline => false } },
                    {
                        code => "var foo = [1]/* any comment*/;",
                        options => { multiline => false }
                    },
                    {
                        code => "var foo = [1]\n/* any comment*/\n;",
                        options => { multiline => false }
                    },
                    { code => "var foo = [1, 2];", options => { multiline => false } },
                    { code => "var foo = [1,\n2];", options => { multiline => false } },
                    {
                        code => "var foo = [function foo() {\nreturn dosomething();\n}];",
                        options => { multiline => false }
                    },
                    { code => "var foo = [1,\n2,[3,\n4]];", options => { multiline => false } },

                    // { min_items => 2 }
                    { code => "var foo = [];", options => { min_items => 2 } },
                    { code => "var foo = [1];", options => { min_items => 2 } },
                    { code => "var foo = [\n1, 2\n];", options => { min_items => 2 } },
                    { code => "var foo = [\n1,\n2\n];", options => { min_items => 2 } },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        options => { min_items => 2 }
                    },
                    {
                        code => "var foo = [
                            1,[
                                2,3
                            ]
                        ];",
                        options => { min_items => 2 }
                    },
                    {
                        code => "var foo = [[
                            1,2
                        ]]",
                        options => { min_items => 2 }
                    },

                    // { min_items => 0 }
                    { code => "var foo = [\n];", options => { min_items => 0 } },
                    { code => "var foo = [\n1\n];", options => { min_items => 0 } },
                    { code => "var foo = [\n1, 2\n];", options => { min_items => 0 } },
                    { code => "var foo = [\n1,\n2\n];", options => { min_items => 0 } },
                    {
                        code => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => { min_items => 0 }
                    },

                    // { min_items => null }
                    { code => "var foo = [];", options => { min_items => null } },
                    { code => "var foo = [1];", options => { min_items => null } },
                    { code => "var foo = [1, 2];", options => { min_items => null } },
                    { code => "var foo = [1,\n2];", options => { min_items => null } },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        options => { min_items => null }
                    },

                    // { multiline => true, min_items => null }
                    {
                        code => "var foo = [];",
                        options => { multiline => true, min_items => null }
                    },
                    {
                        code => "var foo = [1];",
                        options => { multiline => true, min_items => null }
                    },
                    {
                        code => "var foo = [1, 2];",
                        options => { multiline => true, min_items => null }
                    },
                    {
                        code => "var foo = [\n1,\n2\n];",
                        options => { multiline => true, min_items => null }
                    },
                    {
                        code => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => { multiline => true, min_items => null }
                    },

                    // { multiline => true, min_items => 2 }
                    { code => "var a = [];", options => { multiline => true, min_items => 2 } },
                    { code => "var b = [1];", options => { multiline => true, min_items => 2 } },
                    {
                        code => "var b = [ // any comment\n1\n];",
                        options => { multiline => true, min_items => 2 }
                    },
                    {
                        code => "var b = [ /* any comment */ 1];",
                        options => { multiline => true, min_items => 2 }
                    },
                    {
                        code => "var c = [\n1, 2\n];",
                        options => { multiline => true, min_items => 2 }
                    },
                    {
                        code => "var c = [\n/* any comment */1, 2\n];",
                        options => { multiline => true, min_items => 2 }
                    },
                    {
                        code => "var c = [\n1, /* any comment */ 2\n];",
                        options => { multiline => true, min_items => 2 }
                    },
                    {
                        code => "var d = [\n1,\n2\n];",
                        options => { multiline => true, min_items => 2 }
                    },
                    {
                        code => "var e = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => { multiline => true, min_items => 2 }
                    },

                    /*
                     * ArrayPattern
                     * default { multiline => true }
                     */
                    { code => "var [] = foo", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var [a] = foo;", /*parserOptions: { ecmaVersion: 6 }*/ },
                    {
                        code => "var /* any comment */[a] = foo;",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var /* any comment */\n[a] = foo;",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    { code => "var [a, b] = foo;", /*parserOptions: { ecmaVersion: 6 }*/ },
                    {
                        code => "var [ // any comment\na, b\n] = foo;",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\n// any comment\na, b\n] = foo;",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na, b\n// any comment\n] = foo;",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    { code => "var [\na,\nb\n] = foo;", /*parserOptions: { ecmaVersion: 6 }*/ },

                    // "always"
                    {
                        code => "var [\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\n// any\na\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\n/* any */\na\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na, b\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na, b // any comment\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na, b /* any comment */\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na,\nb\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 }
                    },

                    // "consistent"
                    {
                        code => "var [] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [a] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [//\na\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [/**/\na\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [/*\n*/a\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [//\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 }
                    },

                    // { multiline => true }
                    {
                        code => "var [] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [a] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var /* any comment */[a] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var /* any comment */\n[a] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [a, b] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [ // any comment\na, b\n] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\n// any comment\na, b\n] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na, b\n// any comment\n] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "var [\na,\nb\n] = foo;",
                        options => { multiline => true },
                        // parserOptions: { ecmaVersion: 6 }
                    }
                ],
                invalid => [
                    // default : { multiline : true}
                    {
                        code => "var foo = [
                [1,2]
            ]",
                        output => "var foo = [[1,2]]",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 13,
                                end_line => 3,
                                end_column => 14
                            }
                        ]
                    },
                    {
                        code => "var foo = [[2,\n3]]",
                        output => "var foo = [\n[\n2,\n3\n]\n]",
                        errors => [
                            {
                                line => 1,
                                column => 11,
                                message_id => "missing_opening_linebreak",
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                line => 1,
                                column => 12,
                                message_id => "missing_opening_linebreak",
                                end_line => 1,
                                end_column => 13
                            },
                            {
                                line => 2,
                                column => 2,
                                message_id => "missing_closing_linebreak",
                                end_line => 2,
                                end_column => 3
                            },
                            {
                                line => 2,
                                column => 3,
                                message_id => "missing_closing_linebreak",
                                end_line => 2,
                                end_column => 4
                            }
                        ]
                    },

                    /*
                     * ArrayExpression
                     * "always"
                     */
                    {
                        code => "var foo = [[1,2]]",
                        output => "var foo = [\n[\n1,2\n]\n]",
                        options => "always",
                        errors => [
                            {
                                line => 1,
                                column => 11,
                                message_id => "missing_opening_linebreak",
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                line => 1,
                                column => 12,
                                message_id => "missing_opening_linebreak",
                                end_line => 1,
                                end_column => 13
                            },
                            {
                                line => 1,
                                column => 16,
                                message_id => "missing_closing_linebreak",
                                end_line => 1,
                                end_column => 17
                            },
                            {
                                line => 1,
                                column => 17,
                                message_id => "missing_closing_linebreak",
                                end_line => 1,
                                end_column => 18
                            }
                        ]
                    },
                    {
                        code => "var foo = [];",
                        output => "var foo = [\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 12,
                                end_line => 1,
                                end_column => 13
                            }
                        ]
                    },
                    {
                        code => "var foo = [1];",
                        output => "var foo = [\n1\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 13
                            }
                        ]
                    },
                    {
                        code => "var foo = [ // any comment\n1];",
                        output => "var foo = [ // any comment\n1\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2,
                                end_line => 2,
                                end_column => 3
                            }
                        ]
                    },
                    {
                        code => "var foo = [ /* any comment */\n1];",
                        output => "var foo = [ /* any comment */\n1\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [1, 2];",
                        output => "var foo = [\n1, 2\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 16,
                                end_line => 1,
                                end_column => 17
                            }
                        ]
                    },
                    {
                        code => "var foo = [1, 2 // any comment\n];",
                        output => "var foo = [\n1, 2 // any comment\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            }
                        ]
                    },
                    {
                        code => "var foo = [1, 2 /* any comment */];",
                        output => "var foo = [\n1, 2 /* any comment */\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 34
                            }
                        ]
                    },
                    {
                        code => "var foo = [1,\n2];",
                        output => "var foo = [\n1,\n2\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        output => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 2
                            }
                        ]
                    },

                    // "never"
                    {
                        code => "var foo = [[
                1,2],3];",
                        output => "var foo = [[1,2],3];",
                        options => "never",
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                message_id => "unexpected_opening_linebreak",
                                end_line => 1,
                                end_column => 13
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n];",
                        output => "var foo = [];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 12
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1,
                                end_line => 3,
                                end_column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [ /* any comment */\n1, 2\n];",
                        output => "var foo = [ /* any comment */\n1, 2];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1, 2\n/* any comment */];",
                        output => "var foo = [1, 2\n/* any comment */];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 18
                            }
                        ]
                    },
                    {
                        code => "var foo = [ // any comment\n1, 2\n];",
                        output => "var foo = [ // any comment\n1, 2];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1,\n2\n];",
                        output => "var foo = [1,\n2];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 4,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        output => "var foo = [function foo() {\ndosomething();\n}];",
                        options => "never",
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 5,
                                column => 1
                            }
                        ]
                    },

                    // "consistent"
                    {
                        code => "var a = [[1,2]
            ]",
                        output => "var a = [[1,2]]",
                        options => "consistent",
                        errors => [
                            {
                                line => 2,
                                column => 13,
                                message_id => "unexpected_closing_linebreak",
                                end_line => 2,
                                end_column => 14
                            }
                        ]
                    },
                    {
                        code => "var a = [\n[\n[1,2]]\n]",
                        output => "var a = [\n[\n[1,2]\n]\n]",
                        options => "consistent",
                        errors => [
                            {
                                line => 3,
                                column => 6,
                                message_id => "missing_closing_linebreak",
                                end_line => 3,
                                end_column => 7
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1]",
                        output => "var foo = [\n1\n]",
                        options => "consistent",
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2,
                                end_line => 2,
                                end_column => 3
                            }
                        ]
                    },
                    {
                        code => "var foo = [1\n]",
                        output => "var foo = [1]",
                        options => "consistent",
                        errors => [
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1,
                                end_line => 2,
                                end_column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [//\n1]",
                        output => "var foo = [//\n1\n]",
                        options => "consistent",
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2,
                                end_line => 2,
                                end_column => 3
                            }
                        ]
                    },

                    // { multiline => true }
                    {
                        code => "var foo = [\n];",
                        output => "var foo = [];",
                        options => { multiline => true },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n// any comment\n];",
                        output => None,
                        options => { multiline => true },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => { multiline => true },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1, 2\n];",
                        output => "var foo = [1, 2];",
                        options => { multiline => true },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [1,\n2];",
                        output => "var foo = [\n1,\n2\n];",
                        options => { multiline => true },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        output => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => { multiline => true },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 2
                            }
                        ]
                    },

                    // { min_items => 2 }
                    {
                        code => "var foo = [1,[\n2,3\n]\n];",
                        output => "var foo = [\n1,[\n2,3\n]\n];",
                        options => { min_items => 2 },
                        errors => [
                            {
                                line => 1,
                                column => 11,
                                message_id => "missing_opening_linebreak",
                                end_line => 1,
                                end_column => 12
                            }
                        ]
                    },
                    {
                        code => "var foo = [[1,2\n]]",
                        output => "var foo = [[\n1,2\n]]",
                        options => { min_items => 2 },
                        errors => [
                            {
                                line => 1,
                                column => 12,
                                message_id => "missing_opening_linebreak",
                                end_line => 1,
                                end_column => 13
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n];",
                        output => "var foo = [];",
                        options => { min_items => 2 },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => { min_items => 2 },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [1, 2];",
                        output => "var foo = [\n1, 2\n];",
                        options => { min_items => 2 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 16
                            }
                        ]
                    },
                    {
                        code => "var foo = [1,\n2];",
                        output => "var foo = [\n1,\n2\n];",
                        options => { min_items => 2 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        output => "var foo = [function foo() {\ndosomething();\n}];",
                        options => { min_items => 2 },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 5,
                                column => 1
                            }
                        ]
                    },

                    // { min_items => 0 }
                    {
                        code => "var foo = [];",
                        output => "var foo = [\n];",
                        options => { min_items => 0 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 12
                            }
                        ]
                    },
                    {
                        code => "var foo = [1];",
                        output => "var foo = [\n1\n];",
                        options => { min_items => 0 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 13
                            }
                        ]
                    },
                    {
                        code => "var foo = [1, 2];",
                        output => "var foo = [\n1, 2\n];",
                        options => { min_items => 0 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 16
                            }
                        ]
                    },
                    {
                        code => "var foo = [1,\n2];",
                        output => "var foo = [\n1,\n2\n];",
                        options => { min_items => 0 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        output => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => { min_items => 0 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 2
                            }
                        ]
                    },

                    // { min_items => null }
                    {
                        code => "var foo = [\n];",
                        output => "var foo = [];",
                        options => { min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => { min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1, 2\n];",
                        output => "var foo = [1, 2];",
                        options => { min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1,\n2\n];",
                        output => "var foo = [1,\n2];",
                        options => { min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 4,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        output => "var foo = [function foo() {\ndosomething();\n}];",
                        options => { min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 5,
                                column => 1
                            }
                        ]
                    },

                    // { multiline => true, min_items => null }
                    {
                        code => "var foo = [\n];",
                        output => "var foo = [];",
                        options => { multiline => true, min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => { multiline => true, min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1, 2\n];",
                        output => "var foo = [1, 2];",
                        options => { multiline => true, min_items => null },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [1,\n2];",
                        output => "var foo = [\n1,\n2\n];",
                        options => { multiline => true, min_items => null },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        output => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => { multiline => true, min_items => null },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 2
                            }
                        ]
                    },

                    // { multiline => true, min_items => 2 }
                    {
                        code => "var foo = [\n];",
                        output => "var foo = [];",
                        options => { multiline => true, min_items => 2 },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => { multiline => true, min_items => 2 },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [1, 2];",
                        output => "var foo = [\n1, 2\n];",
                        options => { multiline => true, min_items => 2 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 16
                            }
                        ]
                    },
                    {
                        code => "var foo = [1,\n2];",
                        output => "var foo = [\n1,\n2\n];",
                        options => { multiline => true, min_items => 2 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var foo = [function foo() {\ndosomething();\n}];",
                        output => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        options => { multiline => true, min_items => 2 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 2
                            }
                        ]
                    },

                    /*
                     * extra test cases
                     * "always"
                     */
                    {
                        code => "var foo = [\n1, 2];",
                        output => "var foo = [\n1, 2\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 5
                            }
                        ]
                    },
                    {
                        code => "var foo = [\t1, 2];",
                        output => "var foo = [\n\t1, 2\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => Array,
                                line => 1,
                                column => 17
                            }
                        ]
                    },
                    {
                        code => "var foo = [1,\n2\n];",
                        output => "var foo = [\n1,\n2\n];",
                        options => "always",
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            }
                        ]
                    },

                    //  { multiline => false }
                    {
                        code => "var foo = [\n];",
                        output => "var foo = [];",
                        options => { multiline => false },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1\n];",
                        output => "var foo = [1];",
                        options => { multiline => false },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1, 2\n];",
                        output => "var foo = [1, 2];",
                        options => { multiline => false },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\n1,\n2\n];",
                        output => "var foo = [1,\n2];",
                        options => { multiline => false },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 4,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var foo = [\nfunction foo() {\ndosomething();\n}\n];",
                        output => "var foo = [function foo() {\ndosomething();\n}];",
                        options => { multiline => false },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => Array,
                                line => 1,
                                column => 11
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => Array,
                                line => 5,
                                column => 1
                            }
                        ]
                    },

                    /*
                     * ArrayPattern
                     * "always"
                     */
                    {
                        code => "var [] = foo;",
                        output => "var [\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 6
                            }
                        ]
                    },
                    {
                        code => "var [a] = foo;",
                        output => "var [\na\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 7
                            }
                        ]
                    },
                    {
                        code => "var [ // any comment\na] = foo;",
                        output => "var [ // any comment\na\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var [ /* any comment */\na] = foo;",
                        output => "var [ /* any comment */\na\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 2
                            }
                        ]
                    },
                    {
                        code => "var [a, b] = foo;",
                        output => "var [\na, b\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 10
                            }
                        ]
                    },
                    {
                        code => "var [a, b // any comment\n] = foo;",
                        output => "var [\na, b // any comment\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            }
                        ]
                    },
                    {
                        code => "var [a, b /* any comment */] = foo;",
                        output => "var [\na, b /* any comment */\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 28
                            }
                        ]
                    },
                    {
                        code => "var [a,\nb] = foo;",
                        output => "var [\na,\nb\n] = foo;",
                        options => "always",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 2
                            }
                        ]
                    },

                    // "consistent"
                    {
                        code => "var [\na] = foo",
                        output => "var [\na\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 2,
                                end_line => 2,
                                end_column => 3
                            }
                        ]
                    },
                    {
                        code => "var [a\n] = foo",
                        output => "var [a] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 1,
                                end_line => 2,
                                end_column => 2
                            }
                        ]
                    },
                    {
                        code => "var [//\na] = foo",
                        output => "var [//\na\n] = foo",
                        options => "consistent",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 2,
                                end_line => 2,
                                end_column => 3
                            }
                        ]
                    },

                    // { min_items => 2 }
                    {
                        code => "var [\n] = foo;",
                        output => "var [] = foo;",
                        options => { min_items => 2 },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var [\na\n] = foo;",
                        output => "var [a] = foo;",
                        options => { min_items => 2 },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "unexpected_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "unexpected_closing_linebreak",
                                type => ArrayPattern,
                                line => 3,
                                column => 1
                            }
                        ]
                    },
                    {
                        code => "var [a, b] = foo;",
                        output => "var [\na, b\n] = foo;",
                        options => { min_items => 2 },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 10
                            }
                        ]
                    },
                    {
                        code => "var [a,\nb] = foo;",
                        output => "var [\na,\nb\n] = foo;",
                        options => { min_items => 2 },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [
                            {
                                message_id => "missing_opening_linebreak",
                                type => ArrayPattern,
                                line => 1,
                                column => 5
                            },
                            {
                                message_id => "missing_closing_linebreak",
                                type => ArrayPattern,
                                line => 2,
                                column => 2
                            }
                        ]
                    }
                ]
            },
        )
    }
}
