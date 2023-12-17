use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{
    range_between_end_and_start, rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext,
    Rule,
};

#[derive(Copy, Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum BeforeOrAfter {
    Before,
    #[default]
    After,
    Both,
    Neither,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Single(BeforeOrAfter),
    Object(OptionsObject),
}

impl Options {
    pub fn before(&self) -> bool {
        match self {
            Options::Single(value) => matches!(*value, BeforeOrAfter::Before | BeforeOrAfter::Both),
            Options::Object(value) => value.before,
        }
    }

    pub fn after(&self) -> bool {
        match self {
            Options::Single(value) => matches!(*value, BeforeOrAfter::After | BeforeOrAfter::Both),
            Options::Object(value) => value.after,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Single(Default::default())
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct OptionsObject {
    before: bool,
    after: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum BeforeAfter {
    Before,
    After,
}

fn check_spacing<'a>(
    side: BeforeAfter,
    left_token: Node<'a>,
    right_token: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
    before: bool,
    after: bool,
) {
    let expected = match side {
        BeforeAfter::Before => before,
        BeforeAfter::After => after,
    };
    if context.is_space_between(left_token, right_token) != expected {
        let after = left_token.kind() == "*";
        let space_required = expected;
        let node = if after { left_token } else { right_token };

        let message_id = if space_required {
            match side {
                BeforeAfter::Before => "missing_before",
                BeforeAfter::After => "missing_after",
            }
        } else {
            match side {
                BeforeAfter::Before => "unexpected_before",
                BeforeAfter::After => "unexpected_after",
            }
        };

        context.report(violation! {
            node => node,
            message_id => message_id,
            fix => |fixer| {
                match space_required {
                    true => match after {
                        true => fixer.insert_text_after(node, " "),
                        false => fixer.insert_text_before(node, " "),
                    }
                    false => fixer.remove_range(range_between_end_and_start(left_token.range(), right_token.range())),
                }
            }
        });
    }
}

pub fn yield_star_spacing_rule() -> Arc<dyn Rule> {
    rule! {
        name => "yield-star-spacing",
        languages => [Javascript],
        messages => [
            missing_before => "Missing space before *.",
            missing_after => "Missing space after *.",
            unexpected_before => "Unexpected space before *.",
            unexpected_after => "Unexpected space after *.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            before: bool = options.before(),
            after: bool = options.after(),
        },
        listeners => [
            r#"
              (yield_expression) @c
            "# => |node, context| {
                if !node.has_child_of_kind("*") {
                    return;
                }

                let mut tokens = context.get_first_tokens(node, Some(3));
                let yield_token = tokens.next().unwrap();
                let star_token = tokens.next().unwrap();
                let next_token = tokens.next().unwrap();

                check_spacing(BeforeAfter::Before, yield_token, star_token, context, self.before, self.after);
                check_spacing(BeforeAfter::After, star_token, next_token, context, self.before, self.after);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;

    #[test]
    fn test_yield_star_spacing_rule() {
        let missing_before_error = RuleTestExpectedErrorBuilder::default()
            .message_id("missing_before")
            .type_("*")
            .build()
            .unwrap();
        let missing_after_error = RuleTestExpectedErrorBuilder::default()
            .message_id("missing_after")
            .type_("*")
            .build()
            .unwrap();
        let unexpected_before_error = RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected_before")
            .type_("*")
            .build()
            .unwrap();
        let unexpected_after_error = RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected_after")
            .type_("*")
            .build()
            .unwrap();

        RuleTester::run(
            yield_star_spacing_rule(),
            rule_tests! {
                valid => [
                    // default (after)
                    "function *foo(){ yield foo; }",
                    "function *foo(){ yield* foo; }",

                    // after
                    {
                        code => "function *foo(){ yield foo; }",
                        options => "after"
                    },
                    {
                        code => "function *foo(){ yield* foo; }",
                        options => "after"
                    },
                    {
                        code => "function *foo(){ yield* foo(); }",
                        options => "after"
                    },
                    {
                        code => "function *foo(){ yield* 0 }",
                        options => "after"
                    },
                    {
                        code => "function *foo(){ yield* []; }",
                        options => "after"
                    },
                    {
                        code => "function *foo(){ var result = yield* foo(); }",
                        options => "after"
                    },
                    {
                        code => "function *foo(){ var result = yield* (foo()); }",
                        options => "after"
                    },

                    // before
                    {
                        code => "function *foo(){ yield foo; }",
                        options => "before"
                    },
                    {
                        code => "function *foo(){ yield *foo; }",
                        options => "before"
                    },
                    {
                        code => "function *foo(){ yield *foo(); }",
                        options => "before"
                    },
                    {
                        code => "function *foo(){ yield *0 }",
                        options => "before"
                    },
                    {
                        code => "function *foo(){ yield *[]; }",
                        options => "before"
                    },
                    {
                        code => "function *foo(){ var result = yield *foo(); }",
                        options => "before"
                    },

                    // both
                    {
                        code => "function *foo(){ yield foo; }",
                        options => "both"
                    },
                    {
                        code => "function *foo(){ yield * foo; }",
                        options => "both"
                    },
                    {
                        code => "function *foo(){ yield * foo(); }",
                        options => "both"
                    },
                    {
                        code => "function *foo(){ yield * 0 }",
                        options => "both"
                    },
                    {
                        code => "function *foo(){ yield * []; }",
                        options => "both"
                    },
                    {
                        code => "function *foo(){ var result = yield * foo(); }",
                        options => "both"
                    },

                    // neither
                    {
                        code => "function *foo(){ yield foo; }",
                        options => "neither"
                    },
                    {
                        code => "function *foo(){ yield*foo; }",
                        options => "neither"
                    },
                    {
                        code => "function *foo(){ yield*foo(); }",
                        options => "neither"
                    },
                    {
                        code => "function *foo(){ yield*0 }",
                        options => "neither"
                    },
                    {
                        code => "function *foo(){ yield*[]; }",
                        options => "neither"
                    },
                    {
                        code => "function *foo(){ var result = yield*foo(); }",
                        options => "neither"
                    },

                    // object option
                    {
                        code => "function *foo(){ yield* foo; }",
                        options => { before => false, after => true }
                    },
                    {
                        code => "function *foo(){ yield *foo; }",
                        options => { before => true, after => false }
                    },
                    {
                        code => "function *foo(){ yield * foo; }",
                        options => { before => true, after => true }
                    },
                    {
                        code => "function *foo(){ yield*foo; }",
                        options => { before => false, after => false }
                    }
                ],
                invalid => [
                    // default (after)
                    {
                        code => "function *foo(){ yield *foo1; }",
                        output => "function *foo(){ yield* foo1; }",
                        errors => [unexpected_before_error, missing_after_error]
                    },

                    // after
                    {
                        code => "function *foo(){ yield *foo1; }",
                        output => "function *foo(){ yield* foo1; }",
                        options => "after",
                        errors => [unexpected_before_error, missing_after_error]
                    },
                    {
                        code => "function *foo(){ yield * foo; }",
                        output => "function *foo(){ yield* foo; }",
                        options => "after",
                        errors => [unexpected_before_error]
                    },
                    {
                        code => "function *foo(){ yield*foo2; }",
                        output => "function *foo(){ yield* foo2; }",
                        options => "after",
                        errors => [missing_after_error]
                    },

                    // before
                    {
                        code => "function *foo(){ yield* foo; }",
                        output => "function *foo(){ yield *foo; }",
                        options => "before",
                        errors => [missing_before_error, unexpected_after_error]
                    },
                    {
                        code => "function *foo(){ yield * foo; }",
                        output => "function *foo(){ yield *foo; }",
                        options => "before",
                        errors => [unexpected_after_error]
                    },
                    {
                        code => "function *foo(){ yield*foo; }",
                        output => "function *foo(){ yield *foo; }",
                        options => "before",
                        errors => [missing_before_error]
                    },

                    // both
                    {
                        code => "function *foo(){ yield* foo; }",
                        output => "function *foo(){ yield * foo; }",
                        options => "both",
                        errors => [missing_before_error]
                    },
                    {
                        code => "function *foo(){ yield *foo3; }",
                        output => "function *foo(){ yield * foo3; }",
                        options => "both",
                        errors => [missing_after_error]
                    },
                    {
                        code => "function *foo(){ yield*foo4; }",
                        output => "function *foo(){ yield * foo4; }",
                        options => "both",
                        errors => [missing_before_error, missing_after_error]
                    },

                    // neither
                    {
                        code => "function *foo(){ yield* foo; }",
                        output => "function *foo(){ yield*foo; }",
                        options => "neither",
                        errors => [unexpected_after_error]
                    },
                    {
                        code => "function *foo(){ yield *foo; }",
                        output => "function *foo(){ yield*foo; }",
                        options => "neither",
                        errors => [unexpected_before_error]
                    },
                    {
                        code => "function *foo(){ yield * foo; }",
                        output => "function *foo(){ yield*foo; }",
                        options => "neither",
                        errors => [unexpected_before_error, unexpected_after_error]
                    },

                    // object option
                    {
                        code => "function *foo(){ yield*foo; }",
                        output => "function *foo(){ yield* foo; }",
                        options => { before => false, after => true },
                        errors => [missing_after_error]
                    },
                    {
                        code => "function *foo(){ yield * foo; }",
                        output => "function *foo(){ yield *foo; }",
                        options => { before => true, after => false },
                        errors => [unexpected_after_error]
                    },
                    {
                        code => "function *foo(){ yield*foo; }",
                        output => "function *foo(){ yield * foo; }",
                        options => { before => true, after => true },
                        errors => [missing_before_error, missing_after_error]
                    },
                    {
                        code => "function *foo(){ yield * foo; }",
                        output => "function *foo(){ yield*foo; }",
                        options => { before => false, after => false },
                        errors => [unexpected_before_error, unexpected_after_error]
                    }
                ]
            },
        )
    }
}
