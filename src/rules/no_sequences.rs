use std::{collections::HashMap, sync::Arc};

use once_cell::sync::Lazy;
use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, Rule};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{
        ArrowFunction, DoStatement, ExpressionStatement, ForStatement, IfStatement,
        ParenthesizedExpression, SwitchStatement, WhileStatement, WithStatement,
    },
    utils::ast_utils,
};

#[derive(Default, Deserialize)]
struct Options {
    allow_in_parentheses: Option<bool>,
}

impl Options {
    pub fn allow_in_parentheses(&self) -> bool {
        self.allow_in_parentheses.unwrap_or(true)
    }
}

static PARENTHESIZED: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    [
        (DoStatement, "condition"),
        (IfStatement, "condition"),
        (SwitchStatement, "value"),
        (WhileStatement, "condition"),
        (WithStatement, "object"),
        (ArrowFunction, "body"),
    ]
    .into()
});

fn requires_extra_parens(node: Node, parent: Node) -> bool {
    PARENTHESIZED
        .get(parent.kind())
        .copied()
        .matches(|field_name| {
            node == parent
                .field(field_name)
                .skip_nodes_of_types(&[ExpressionStatement, ParenthesizedExpression])
        })
}

fn is_parenthesised_twice(node: Node) -> bool {
    node.parent().matches(|parent| {
        parent.kind() == ParenthesizedExpression
            && parent.parent().unwrap().kind() == ParenthesizedExpression
    })
}

pub fn no_sequences_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-sequences",
        languages => [Javascript],
        messages => [
            unexpected_comma_expression => "Unexpected use of comma operator.",
        ],
        options_type => Option<Options>,
        state => {
            [per-run]
            allow_in_parentheses: bool = options.unwrap_or_default().allow_in_parentheses(),
        },
        listeners => [
            r#"
              (sequence_expression) @c
            "# => |node, context| {
                let parent = node.next_ancestor_not_of_types(&[
                    ExpressionStatement,
                    ParenthesizedExpression,
                ]);
                if parent.kind() == ForStatement && (
                    node == parent.field("initializer").skip_nodes_of_types(&[
                        ExpressionStatement,
                        ParenthesizedExpression,
                    ]) ||
                    parent.child_by_field_name("increment").matches(|increment| {
                        node == increment.skip_parentheses()
                    })
                ) {
                    return;
                }

                if self.allow_in_parentheses {
                    #[allow(clippy::collapsible_else_if)]
                    if requires_extra_parens(node, parent) {
                        if is_parenthesised_twice(node) {
                            return;
                        }
                    } else {
                        if ast_utils::is_parenthesised(node) {
                            return;
                        }
                    }
                }

                let first_comma_token = context.get_token_after(
                    node.field("left"),
                    Some(|node: Node| ast_utils::is_comma_token(node, context)),
                );

                context.report(violation! {
                    node => node,
                    range => first_comma_token.range(),
                    message_id => "unexpected_comma_expression",
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::SequenceExpression;

    use super::*;

    use tree_sitter_lint::{
        rule_tests, RuleTestExpectedError, RuleTestExpectedErrorBuilder, RuleTester,
    };

    fn errors(column: usize) -> Vec<RuleTestExpectedError> {
        vec![RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected_comma_expression")
            .type_(SequenceExpression)
            .line(1_usize)
            .column(column)
            .build()
            .unwrap()]
    }

    #[test]
    fn test_no_sequences_rule() {
        RuleTester::run(
            no_sequences_rule(),
            rule_tests! {
            // Examples of code that should not trigger the rule
            valid => [
                "var arr = [1, 2];",
                "var obj = {a: 1, b: 2};",
                "var a = 1, b = 2;",
                "var foo = (1, 2);",
                "(0,eval)(\"foo()\");",
                "for (i = 1, j = 2;; i++, j++);",
                "foo(a, (b, c), d);",
                "do {} while ((doSomething(), !!test));",
                "for ((doSomething(), somethingElse()); (doSomething(), !!test); );",
                "if ((doSomething(), !!test));",
                "switch ((doSomething(), val)) {}",
                "while ((doSomething(), !!test));",
                "with ((doSomething(), val)) {}",
                { code => "a => ((doSomething(), a))", /*env: { es6: true }*/ },

                // options object without "allowInParentheses" property
                { code => "var foo = (1, 2);", options => {} },

                // explicitly set option "allowInParentheses" to default value
                { code => "var foo = (1, 2);", options => { allow_in_parentheses => true } },

                // valid code with "allowInParentheses" set to `false`
                { code => "for ((i = 0, j = 0); test; );", options => { allow_in_parentheses => false } },
                { code => "for (; test; (i++, j++));", options => { allow_in_parentheses => false } },

                // https://github.com/eslint/eslint/issues/14572
                { code => "const foo = () => { return ((bar = 123), 10) }", /*env: { es6: true }*/ },
                { code => "const foo = () => (((bar = 123), 10));", /*env: { es6: true }*/ }
            ],

            // Examples of code that should trigger the rule
            invalid => [
                {
                    code => "1, 2;",
                    errors => [{
                        message_id => "unexpected_comma_expression",
                        type => SequenceExpression,
                        line => 1,
                        column => 2,
                        end_line => 1,
                        end_column => 3
                    }]
                },
                { code => "a = 1, 2", errors => errors(6) },
                { code => "do {} while (doSomething(), !!test);", errors => errors(27) },
                { code => "for (; doSomething(), !!test; );", errors => errors(21) },
                { code => "if (doSomething(), !!test);", errors => errors(18) },
                { code => "switch (doSomething(), val) {}", errors => errors(22) },
                { code => "while (doSomething(), !!test);", errors => errors(21) },
                { code => "with (doSomething(), val) {}", errors => errors(20) },
                { code => "a => (doSomething(), a)", /*env: { es6: true }*/ errors => errors(20) },
                { code => "(1), 2", errors => errors(4) },
                { code => "((1)) , (2)", errors => errors(7) },
                { code => "while((1) , 2);", errors => errors(11) },

                // option "allowInParentheses": do not allow sequence in parentheses
                { code => "var foo = (1, 2);", options => { allow_in_parentheses => false }, errors => errors(13) },
                { code => "(0,eval)(\"foo()\");", options => { allow_in_parentheses => false }, errors => errors(3) },
                { code => "foo(a, (b, c), d);", options => { allow_in_parentheses => false }, errors => errors(10) },
                { code => "do {} while ((doSomething(), !!test));", options => { allow_in_parentheses => false }, errors => errors(28) },
                { code => "for (; (doSomething(), !!test); );", options => { allow_in_parentheses => false }, errors => errors(22) },
                { code => "if ((doSomething(), !!test));", options => { allow_in_parentheses => false }, errors => errors(19) },
                { code => "switch ((doSomething(), val)) {}", options => { allow_in_parentheses => false }, errors => errors(23) },
                { code => "while ((doSomething(), !!test));", options => { allow_in_parentheses => false }, errors => errors(22) },
                { code => "with ((doSomething(), val)) {}", options => { allow_in_parentheses => false }, errors => errors(21) },
                { code => "a => ((doSomething(), a))", options => { allow_in_parentheses => false }, /*env: { es6: true }*/ errors => errors(21) }
            ]
            },
        )
    }
}
