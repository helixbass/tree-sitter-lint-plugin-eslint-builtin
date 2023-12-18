use std::{collections::HashMap, sync::Arc};

use itertools::Itertools;
use serde::Deserialize;
use squalid::EverythingExt;
use tree_sitter_lint::{
    range_between_end_and_start, rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext,
    Rule,
};

use crate::{
    ast_helpers::is_postfix_update_expression,
    kind::{NewExpression, UnaryExpression},
    utils::ast_utils,
};

type Overrides = HashMap<String, bool>;

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    words: bool,
    nonwords: bool,
    overrides: Overrides,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            words: true,
            nonwords: Default::default(),
            overrides: Default::default(),
        }
    }
}

fn is_first_bang_in_bang_bang_expression(node: Node) -> bool {
    // TODO: looks like the ESLint version says node.argument.operator here when
    // it means node.operator, upstream?
    node.kind() == UnaryExpression
        && node.field("operator").kind() == "!"
        && node.field("argument").thrush(|argument| {
            argument.kind() == UnaryExpression && argument.field("operator").kind() == "!"
        })
}

fn override_exists_for_operator(operator: &str, overrides: &Overrides) -> bool {
    overrides.contains_key(operator)
}

fn override_enforces_spaces(operator: &str, overrides: &Overrides) -> bool {
    overrides.get(operator).copied() == Some(true)
}

fn verify_word_has_spaces(
    node: Node,
    first_token: Node,
    second_token: Node,
    word: &str,
    context: &QueryMatchContext,
) {
    if second_token.range().start_byte == first_token.range().end_byte {
        context.report(violation! {
            node => node,
            message_id => "word_operator",
            data => {
                word => word,
            },
            fix => |fixer| {
                fixer.insert_text_after(first_token, " ");
            }
        });
    }
}

fn verify_word_doesnt_have_spaces(
    node: Node,
    first_token: Node,
    second_token: Node,
    word: &str,
    context: &QueryMatchContext,
) {
    #[allow(clippy::collapsible_if)]
    if ast_utils::can_tokens_be_adjacent(first_token, second_token, context) {
        if second_token.range().start_byte > first_token.range().end_byte {
            context.report(violation! {
                node => node,
                message_id => "unexpected_after_word",
                data => {
                    word => word,
                },
                fix => |fixer| {
                    fixer.remove_range(range_between_end_and_start(first_token.range(), second_token.range()));
                }
            });
        }
    }
}

fn check_unary_word_operator_for_spaces(
    node: Node,
    first_token: Node,
    second_token: Node,
    word: &str,
    overrides: &Overrides,
    words: bool,
    context: &QueryMatchContext,
) {
    if override_exists_for_operator(word, overrides) {
        if override_enforces_spaces(word, overrides) {
            verify_word_has_spaces(node, first_token, second_token, word, context);
        } else {
            verify_word_doesnt_have_spaces(node, first_token, second_token, word, context);
        }
    } else if words {
        verify_word_has_spaces(node, first_token, second_token, word, context);
    } else {
        verify_word_doesnt_have_spaces(node, first_token, second_token, word, context);
    }
}

fn verify_non_words_have_spaces(
    node: Node,
    first_token: Node,
    second_token: Node,
    context: &QueryMatchContext,
) {
    #[allow(clippy::collapsible_else_if)]
    if !is_postfix_update_expression(node, context) {
        if is_first_bang_in_bang_bang_expression(node) {
            return;
        }
        if first_token.range().end_byte == second_token.range().start_byte {
            context.report(violation! {
                node => node,
                message_id => "operator",
                data => {
                    operator => first_token.kind(),
                },
                fix => |fixer| {
                    fixer.insert_text_after(first_token, " ");
                }
            });
        }
    } else {
        if first_token.range().end_byte == second_token.range().start_byte {
            context.report(violation! {
                node => node,
                message_id => "before_unary_expressions",
                data => {
                    token => second_token.kind(),
                },
                fix => |fixer| {
                    fixer.insert_text_before(second_token, " ");
                }
            });
        }
    }
}

fn verify_non_words_dont_have_spaces(
    node: Node,
    first_token: Node,
    second_token: Node,
    context: &QueryMatchContext,
) {
    #[allow(clippy::collapsible_else_if)]
    if !is_postfix_update_expression(node, context) {
        if second_token.range().start_byte > first_token.range().end_byte {
            context.report(violation! {
                node => node,
                message_id => "unexpected_after",
                data => {
                    operator => first_token.kind(),
                },
                fix => |fixer| {
                    if ast_utils::can_tokens_be_adjacent(first_token, second_token, context) {
                        fixer.remove_range(range_between_end_and_start(first_token.range(), second_token.range()));
                    }
                }
            });
        }
    } else {
        if second_token.range().start_byte > first_token.range().end_byte {
            context.report(violation! {
                node => node,
                message_id => "unexpected_before",
                data => {
                    operator => second_token.kind(),
                },
                fix => |fixer| {
                    if ast_utils::can_tokens_be_adjacent(first_token, second_token, context) {
                        fixer.remove_range(range_between_end_and_start(first_token.range(), second_token.range()));
                    }
                }
            });
        }
    }
}

pub fn space_unary_ops_rule() -> Arc<dyn Rule> {
    rule! {
        name => "space-unary-ops",
        languages => [Javascript],
        messages => [
            unexpected_before => "Unexpected space before unary operator '{{operator}}'.",
            unexpected_after => "Unexpected space after unary operator '{{operator}}'.",
            unexpected_after_word => "Unexpected space after unary word operator '{{word}}'.",
            word_operator => "Unary word operator '{{word}}' must be followed by whitespace.",
            operator => "Unary operator '{{operator}}' must be followed by whitespace.",
            before_unary_expressions => "Space is required before unary expressions '{{token}}'.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            words: bool = options.words,
            nonwords: bool = options.nonwords,
            overrides: Overrides = options.overrides,
        },
        listeners => [
            r#"
              (unary_expression) @c
              (update_expression) @c
              (new_expression) @c
            "# => |node, context| {
                let is_postfix_update_expression = is_postfix_update_expression(node, context);
                let tokens = if is_postfix_update_expression {
                    context.get_last_tokens(node, Some(2)).collect_vec()
                } else {
                    context.get_first_tokens(node, Some(2)).collect_vec()
                };
                let first_token = tokens[0];
                let second_token = tokens[1];

                if node.kind() == NewExpression ||
                    node.kind() == UnaryExpression && first_token.kind().len() > 1 {
                    check_unary_word_operator_for_spaces(node, first_token, second_token, first_token.kind(), &self.overrides, self.words, context);
                    return;
                }

                let operator = if is_postfix_update_expression {
                    tokens[1].kind()
                } else {
                    tokens[0].kind()
                };

                if override_exists_for_operator(operator, &self.overrides) {
                    if override_enforces_spaces(operator, &self.overrides) {
                        verify_non_words_have_spaces(node, first_token, second_token, context);
                    } else {
                        verify_non_words_dont_have_spaces(node, first_token, second_token, context);
                    }
                } else if self.nonwords {
                    verify_non_words_have_spaces(node, first_token, second_token, context);
                } else {
                    verify_non_words_dont_have_spaces(node, first_token, second_token, context);
                }
            },
            "
              (yield_expression) @c
            " => |node, context| {
                if node.has_child_of_kind("*") {
                    return;
                }
                if !node.has_non_comment_named_children(context) {
                    return;
                }

                let tokens = context.get_first_tokens(node, Some(3)).collect_vec();
                check_unary_word_operator_for_spaces(
                    node,
                    tokens[0],
                    tokens[1],
                    "yield",
                    &self.overrides,
                    self.words,
                    context,
                );
            },
            "
              (await_expression) @c
            " => |node, context| {
                let tokens = context.get_first_tokens(node, Some(3)).collect_vec();
                check_unary_word_operator_for_spaces(
                    node,
                    tokens[0],
                    tokens[1],
                    "await",
                    &self.overrides,
                    self.words,
                    context,
                );
            }
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::{AwaitExpression, YieldExpression};

    #[test]
    fn test_space_unary_ops_rule() {
        RuleTester::run(
            space_unary_ops_rule(),
            rule_tests! {
                valid => [
                    {
                        code => "++this.a",
                        options => { words => true }
                    },
                    {
                        code => "--this.a",
                        options => { words => true }
                    },
                    {
                        code => "this.a++",
                        options => { words => true }
                    },
                    {
                        code => "this.a--",
                        options => { words => true }
                    },
                    "foo .bar++",
                    {
                        code => "foo.bar --",
                        options => { nonwords => true }
                    },

                    {
                        code => "delete foo.bar",
                        options => { words => true }
                    },
                    {
                        code => "delete foo[\"bar\"]",
                        options => { words => true }
                    },
                    {
                        code => "delete foo.bar",
                        options => { words => false }
                    },
                    {
                        code => "delete(foo.bar)",
                        options => { words => false }
                    },

                    {
                        code => "new Foo",
                        options => { words => true }
                    },
                    {
                        code => "new Foo()",
                        options => { words => true }
                    },
                    {
                        code => "new [foo][0]",
                        options => { words => true }
                    },
                    {
                        code => "new[foo][0]",
                        options => { words => false }
                    },

                    {
                        code => "typeof foo",
                        options => { words => true }
                    },
                    {
                        code => "typeof{foo:true}",
                        options => { words => false }
                    },
                    {
                        code => "typeof {foo:true}",
                        options => { words => true }
                    },
                    {
                        code => "typeof (foo)",
                        options => { words => true }
                    },
                    {
                        code => "typeof(foo)",
                        options => { words => false }
                    },
                    {
                        code => "typeof!foo",
                        options => { words => false }
                    },

                    {
                        code => "void 0",
                        options => { words => true }
                    },
                    {
                        code => "(void 0)",
                        options => { words => true }
                    },
                    {
                        code => "(void (0))",
                        options => { words => true }
                    },
                    {
                        code => "void foo",
                        options => { words => true }
                    },
                    {
                        code => "void foo",
                        options => { words => false }
                    },
                    {
                        code => "void(foo)",
                        options => { words => false }
                    },

                    {
                        code => "-1",
                        options => { nonwords => false }
                    },
                    {
                        code => "!foo",
                        options => { nonwords => false }
                    },
                    {
                        code => "!!foo",
                        options => { nonwords => false }
                    },
                    {
                        code => "foo++",
                        options => { nonwords => false }
                    },
                    {
                        code => "foo ++",
                        options => { nonwords => true }
                    },
                    {
                        code => "++foo",
                        options => { nonwords => false }
                    },
                    {
                        code => "++ foo",
                        options => { nonwords => true }
                    },
                    {
                        code => "function *foo () { yield (0) }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield +1 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield* 0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield * 0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { (yield)*0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { (yield) * 0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield*0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo() { yield *0 }",
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "async function foo() { await {foo: 1} }",
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await {bar: 2} }",
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await{baz: 3} }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await {qux: 4} }",
                        options => { words => false, overrides => { "await" => true } },
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "async function foo() { await{foo: 5} }",
                        options => { words => true, overrides => { "await" => false } },
                        // parserOptions: { ecmaVersion: 8 }
                    },
                    {
                        code => "foo++",
                        options => { nonwords => true, overrides => { "++" => false } }
                    },
                    {
                        code => "foo++",
                        options => { nonwords => false, overrides => { "++" => false } }
                    },
                    {
                        code => "++foo",
                        options => { nonwords => true, overrides => { "++" => false } }
                    },
                    {
                        code => "++foo",
                        options => { nonwords => false, overrides => { "++" => false } }
                    },
                    {
                        code => "!foo",
                        options => { nonwords => true, overrides => { "!" => false } }
                    },
                    {
                        code => "!foo",
                        options => { nonwords => false, overrides => { "!" => false } }
                    },
                    {
                        code => "new foo",
                        options => { words => true, overrides => { new => false } }
                    },
                    {
                        code => "new foo",
                        options => { words => false, overrides => { new => false } }
                    },
                    {
                        code => "function *foo () { yield(0) }",
                        options => { words => true, overrides => { "yield" => false } },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "function *foo () { yield(0) }",
                        options => { words => false, overrides => { "yield" => false } },
                        // parserOptions: { ecmaVersion: 6 }
                    },
                    {
                        code => "class C { #x; *foo(bar) { yield#x in bar; } }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 2022 }
                    }
                ],
                invalid => [
                    {
                        code => "delete(foo.bar)",
                        output => "delete (foo.bar)",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "delete" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "delete(foo[\"bar\"]);",
                        output => "delete (foo[\"bar\"]);",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "delete" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "delete (foo.bar)",
                        output => "delete(foo.bar)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "delete" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "new(Foo)",
                        output => "new (Foo)",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "new" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new (Foo)",
                        output => "new(Foo)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "new" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new(Foo())",
                        output => "new (Foo())",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "new" },
                            type => NewExpression
                        }]
                    },
                    {
                        code => "new [foo][0]",
                        output => "new[foo][0]",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "new" },
                            type => NewExpression
                        }]
                    },

                    {
                        code => "typeof(foo)",
                        output => "typeof (foo)",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "typeof" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "typeof (foo)",
                        output => "typeof(foo)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "typeof" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "typeof[foo]",
                        output => "typeof [foo]",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "typeof" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "typeof [foo]",
                        output => "typeof[foo]",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "typeof" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "typeof{foo:true}",
                        output => "typeof {foo:true}",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "typeof" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "typeof {foo:true}",
                        output => "typeof{foo:true}",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "typeof" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "typeof!foo",
                        output => "typeof !foo",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "typeof" },
                            type => UnaryExpression
                        }]
                    },

                    {
                        code => "void(0);",
                        output => "void (0);",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "void" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "void(foo);",
                        output => "void (foo);",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "void" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "void[foo];",
                        output => "void [foo];",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "void" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "void{a:0};",
                        output => "void {a:0};",
                        options => { words => true },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "void" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "void (foo)",
                        output => "void(foo)",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "void" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "void [foo]",
                        output => "void[foo]",
                        options => { words => false },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "void" },
                            type => UnaryExpression
                        }]
                    },

                    {
                        code => "! foo",
                        output => "!foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "!" }
                        }]
                    },
                    {
                        code => "!foo",
                        output => "! foo",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" }
                        }]
                    },

                    {
                        code => "!! foo",
                        output => "!!foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "!" },
                            type => UnaryExpression,
                            line => 1,
                            column => 2
                        }]
                    },
                    {
                        code => "!!foo",
                        output => "!! foo",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" },
                            type => UnaryExpression,
                            line => 1,
                            column => 2
                        }]
                    },

                    {
                        code => "- 1",
                        output => "-1",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "-" },
                            type => UnaryExpression
                        }]
                    },
                    {
                        code => "-1",
                        output => "- 1",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "-" },
                            type => UnaryExpression
                        }]
                    },

                    {
                        code => "foo++",
                        output => "foo ++",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "before_unary_expressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "foo ++",
                        output => "foo++",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_before",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "++ foo",
                        output => "++foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "++foo",
                        output => "++ foo",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "foo .bar++",
                        output => "foo .bar ++",
                        options => { nonwords => true },
                        errors => [{
                            message_id => "before_unary_expressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "foo.bar --",
                        output => "foo.bar--",
                        errors => [{
                            message_id => "unexpected_before",
                            data => { operator => "--" }
                        }]
                    },
                    {
                        code => "+ +foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "+" }
                        }]
                    },
                    {
                        code => "+ ++foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "+" }
                        }]
                    },
                    {
                        code => "- -foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "-" }
                        }]
                    },
                    {
                        code => "- --foo",
                        output => None,
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "-" }
                        }]
                    },
                    {
                        code => "+ -foo",
                        output => "+-foo",
                        options => { nonwords => false },
                        errors => [{
                            message_id => "unexpected_after",
                            data => { operator => "+" }
                        }]
                    },
                    {
                        code => "function *foo() { yield(0) }",
                        output => "function *foo() { yield (0) }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "function *foo() { yield (0) }",
                        output => "function *foo() { yield(0) }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "function *foo() { yield+0 }",
                        output => "function *foo() { yield +0 }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "foo++",
                        output => "foo ++",
                        options => { nonwords => true, overrides => { "++" => true } },
                        errors => [{
                            message_id => "before_unary_expressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "foo++",
                        output => "foo ++",
                        options => { nonwords => false, overrides => { "++" => true } },
                        errors => [{
                            message_id => "before_unary_expressions",
                            data => { token => "++" }
                        }]
                    },
                    {
                        code => "++foo",
                        output => "++ foo",
                        options => { nonwords => true, overrides => { "++" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "++foo",
                        output => "++ foo",
                        options => { nonwords => false, overrides => { "++" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "++" }
                        }]
                    },
                    {
                        code => "!foo",
                        output => "! foo",
                        options => { nonwords => true, overrides => { "!" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" }
                        }]
                    },
                    {
                        code => "!foo",
                        output => "! foo",
                        options => { nonwords => false, overrides => { "!" => true } },
                        errors => [{
                            message_id => "operator",
                            data => { operator => "!" }
                        }]
                    },
                    {
                        code => "new(Foo)",
                        output => "new (Foo)",
                        options => { words => true, overrides => { new => true } },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "new" }
                        }]
                    },
                    {
                        code => "new(Foo)",
                        output => "new (Foo)",
                        options => { words => false, overrides => { new => true } },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "new" }
                        }]
                    },
                    {
                        code => "function *foo() { yield(0) }",
                        output => "function *foo() { yield (0) }",
                        options => { words => true, overrides => { "yield" => true } },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "function *foo() { yield(0) }",
                        output => "function *foo() { yield (0) }",
                        options => { words => false, overrides => { "yield" => true } },
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 19
                        }]
                    },
                    {
                        code => "async function foo() { await{foo: 'bar'} }",
                        output => "async function foo() { await {foo: 'bar'} }",
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "async function foo() { await{baz: 'qux'} }",
                        output => "async function foo() { await {baz: 'qux'} }",
                        options => { words => false, overrides => { "await" => true } },
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "word_operator",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "async function foo() { await {foo: 1} }",
                        output => "async function foo() { await{foo: 1} }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "async function foo() { await {bar: 2} }",
                        output => "async function foo() { await{bar: 2} }",
                        options => { words => true, overrides => { "await" => false } },
                        // parserOptions: { ecmaVersion: 8 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "await" },
                            type => AwaitExpression,
                            line => 1,
                            column => 24
                        }]
                    },
                    {
                        code => "class C { #x; *foo(bar) { yield #x in bar; } }",
                        output => "class C { #x; *foo(bar) { yield#x in bar; } }",
                        options => { words => false },
                        // parserOptions: { ecmaVersion: 2022 },
                        errors => [{
                            message_id => "unexpected_after_word",
                            data => { word => "yield" },
                            type => YieldExpression,
                            line => 1,
                            column => 27
                        }]
                    }
                ]
            },
        )
    }
}
