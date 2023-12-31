use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, NodeExt, Rule};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    enforce_for_ordering_relations: bool,
}

pub fn no_unsafe_negation_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-unsafe-negation",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected negating the left operand of '{{operator}}' operator.",
        ],
        options_type => Options,
        state => {
            [per-config]
            enforce_for_ordering_relations: bool = options.enforce_for_ordering_relations,
        },
        listeners => [
            r#"
              (binary_expression
                left: (unary_expression
                  operator: "!"
                )
                operator: [
                  "in"
                  "instanceof"
                  "<"
                  ">"
                  ">="
                  "<="
                ]
              ) @c
            "# => |node, context| {
                let operator = node.field("operator").kind();
                if !self.enforce_for_ordering_relations && ["<", ">", "<=", ">="].contains(&operator) {
                    return;
                }

                context.report(violation! {
                    node => node,
                    range => node.field("left").range(),
                    message_id => "unexpected",
                    data => {
                        operator => operator,
                    },
                    // suggest: [
                    //     {
                    //         messageId: "suggestNegatedExpression",
                    //         data: { operator },
                    //         fix(fixer) {
                    //             const negationToken = sourceCode.getFirstToken(node.left);
                    //             const fixRange = [negationToken.range[1], node.range[1]];
                    //             const text = sourceCode.text.slice(fixRange[0], fixRange[1]);

                    //             return fixer.replaceTextRange(fixRange, `(${text})`);
                    //         },
                    //     },
                    //     {
                    //         messageId: "suggestParenthesisedNegation",
                    //         fix(fixer) {
                    //             return fixer.replaceTextRange(node.left, `(${sourceCode.getText(node.left)})`);
                    //         },
                    //     },
                    // ]
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
    fn test_no_unsafe_negation_rule() {
        RuleTester::run(
            no_unsafe_negation_rule(),
            rule_tests! {
                valid => [
                    "a in b",
                    "a in b === false",
                    "!(a in b)",
                    "(!a) in b",
                    "a instanceof b",
                    "a instanceof b === false",
                    "!(a instanceof b)",
                    "(!a) instanceof b",

                    // tests cases for enforceForOrderingRelations option:
                    "if (! a < b) {}",
                    "while (! a > b) {}",
                    "foo = ! a <= b;",
                    "foo = ! a >= b;",
                    {
                        code => "! a <= b",
                        options => {}
                    },
                    {
                        code => "foo = ! a >= b;",
                        options => { enforce_for_ordering_relations => false }
                    },
                    {
                        code => "foo = (!a) >= b;",
                        options => { enforce_for_ordering_relations => true }
                    },
                    {
                        code => "a <= b",
                        options => { enforce_for_ordering_relations => true }
                    },
                    {
                        code => "!(a < b)",
                        options => { enforce_for_ordering_relations => true }
                    },
                    {
                        code => "foo = a > b;",
                        options => { enforce_for_ordering_relations => true }
                    }
                ],
                invalid => [
                    {
                        code => "!a in b",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "in" },
                            // suggestions: [
                            //     {
                            //         desc: "Negate 'in' expression instead of its left operand. This changes the current behavior.",
                            //         output => "!(a in b)"
                            //     },
                            //     {
                            //         desc: "Wrap negation in '()' to make the intention explicit. This preserves the current behavior.",
                            //         output => "(!a) in b"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "(!a in b)",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "in" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "(!(a in b))"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "((!a) in b)"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "!(a) in b",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "in" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "!((a) in b)"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "(!(a)) in b"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "!a instanceof b",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "instanceof" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "!(a instanceof b)"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "(!a) instanceof b"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "(!a instanceof b)",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "instanceof" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "(!(a instanceof b))"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "((!a) instanceof b)"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "!(a) instanceof b",
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "instanceof" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "!((a) instanceof b)"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "(!(a)) instanceof b"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "if (! a < b) {}",
                        options => { enforce_for_ordering_relations => true },
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "<" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "if (!( a < b)) {}"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "if ((! a) < b) {}"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "while (! a > b) {}",
                        options => { enforce_for_ordering_relations => true },
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => ">" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "while (!( a > b)) {}"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "while ((! a) > b) {}"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "foo = ! a <= b;",
                        options => { enforce_for_ordering_relations => true },
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "<=" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "foo = !( a <= b);"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "foo = (! a) <= b;"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "foo = ! a >= b;",
                        options => { enforce_for_ordering_relations => true },
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => ">=" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "foo = !( a >= b);"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "foo = (! a) >= b;"
                            //     }
                            // ]
                        }]
                    },
                    {
                        code => "! a <= b",
                        options => { enforce_for_ordering_relations => true },
                        errors => [{
                            message_id => "unexpected",
                            data => { operator => "<=" },
                            // suggestions: [
                            //     {
                            //         message_id => "suggestNegatedExpression",
                            //         output => "!( a <= b)"
                            //     },
                            //     {
                            //         message_id => "suggestParenthesisedNegation",
                            //         output => "(! a) <= b"
                            //     }
                            // ]
                        }]
                    }
                ]
            },
        )
    }
}
