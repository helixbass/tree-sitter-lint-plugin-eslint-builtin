use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, Rule};

use crate::utils::ast_utils;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Options {
    #[default]
    Object,
    Property,
}

pub fn dot_location_rule() -> Arc<dyn Rule> {
    rule! {
        name => "dot-location",
        languages => [Javascript],
        messages => [
            expected_dot_after_object => "Expected dot to be on same line as object.",
            expected_dot_before_property => "Expected dot to be on same line as property.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            on_object: bool = options == Options::Object,
        },
        listeners => [
            r#"
              (member_expression) @c
            "# => |node, context| {
                let property = node.field("property");
                let dot_token = context.get_token_before(property, Option::<fn(Node) -> bool>::None);

                if self.on_object {
                    let token_before_dot = context.get_token_before(dot_token, Option::<fn(Node) -> bool>::None);

                    if !ast_utils::is_token_on_same_line(token_before_dot, dot_token) {
                        context.report(violation! {
                            node => node,
                            range => dot_token.range(),
                            message_id => "expected_dot_after_object",
                            fix => |fixer| {
                                if dot_token.text(context).starts_with('.') &&
                                    ast_utils::is_decimal_integer_numeric_token(token_before_dot, context) {
                                    fixer.insert_text_after(
                                        token_before_dot,
                                        format!(" {}", dot_token.text(context)),
                                    );
                                } else {
                                    fixer.insert_text_after(
                                        token_before_dot,
                                        dot_token.text(context),
                                    );
                                }
                                fixer.remove(dot_token);
                            }
                        });
                    }
                } else if !ast_utils::is_token_on_same_line(dot_token, property) {
                    context.report(violation! {
                        node => node,
                        range => dot_token.range(),
                        message_id => "expected_dot_before_property",
                        fix => |fixer| {
                            fixer.remove(dot_token);
                            fixer.insert_text_before(property, dot_token.text(context));
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::MemberExpression;

    #[test]
    fn test_dot_location_rule() {
        RuleTester::run(
            dot_location_rule(),
            rule_tests! {
                valid => [
                    "obj.\nprop",
                    "obj. \nprop",
                    "obj.\n prop",
                    "(obj).\nprop",
                    "obj\n['prop']",
                    "obj['prop']",
                    {
                        code => "obj.\nprop",
                        options => "object"
                    },
                    {
                        code => "obj\n.prop",
                        options => "property"
                    },
                    {
                        code => "(obj)\n.prop",
                        options => "property"
                    },
                    {
                        code => "obj . prop",
                        options => "object"
                    },
                    {
                        code => "obj /* a */ . prop",
                        options => "object"
                    },
                    {
                        code => "obj . \nprop",
                        options => "object"
                    },
                    {
                        code => "obj . prop",
                        options => "property"
                    },
                    {
                        code => "obj . /* a */ prop",
                        options => "property"
                    },
                    {
                        code => "obj\n. prop",
                        options => "property"
                    },
                    {
                        code => "f(a\n).prop",
                        options => "object"
                    },
                    {
                        code => "`\n`.prop",
                        options => "object",
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "obj[prop]",
                        options => "object"
                    },
                    {
                        code => "obj\n[prop]",
                        options => "object"
                    },
                    {
                        code => "obj[\nprop]",
                        options => "object"
                    },
                    {
                        code => "obj\n[\nprop\n]",
                        options => "object"
                    },
                    {
                        code => "obj[prop]",
                        options => "property"
                    },
                    {
                        code => "obj\n[prop]",
                        options => "property"
                    },
                    {
                        code => "obj[\nprop]",
                        options => "property"
                    },
                    {
                        code => "obj\n[\nprop\n]",
                        options => "property"
                    },

                    // https://github.com/eslint/eslint/issues/11868 (also in invalid)
                    {
                        code => "(obj).prop",
                        options => "object"
                    },
                    {
                        code => "(obj).\nprop",
                        options => "object"
                    },
                    {
                        code => "(obj\n).\nprop",
                        options => "object"
                    },
                    {
                        code => "(\nobj\n).\nprop",
                        options => "object"
                    },
                    {
                        code => "((obj\n)).\nprop",
                        options => "object"
                    },
                    {
                        code => "(f(a)\n).\nprop",
                        options => "object"
                    },
                    {
                        code => "((obj\n)\n).\nprop",
                        options => "object"
                    },
                    {
                        code => "(\na &&\nb()\n).toString()",
                        options => "object"
                    },

                    // Optional chaining
                    {
                        code => "obj?.prop",
                        options => "object",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.[key]",
                        options => "object",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.\nprop",
                        options => "object",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj\n?.[key]",
                        options => "object",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.\n[key]",
                        options => "object",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.[\nkey]",
                        options => "object",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.prop",
                        options => "property",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.[key]",
                        options => "property",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj\n?.prop",
                        options => "property",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj\n?.[key]",
                        options => "property",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.\n[key]",
                        options => "property",
                        environment => { ecma_version => 2020 }
                    },
                    {
                        code => "obj?.[\nkey]",
                        options => "property",
                        environment => { ecma_version => 2020 }
                    },

                    // Private properties
                    {
                        code => "class C { #a; foo() { this.\n#a; } }",
                        options => "object",
                        environment => { ecma_version => 2022 }
                    },
                    {
                        code => "class C { #a; foo() { this\n.#a; } }",
                        options => "property",
                        environment => { ecma_version => 2022 }
                    }
                ],
                invalid => [
                    {
                        code => "obj\n.property",
                        output => "obj.\nproperty",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1, end_line => 2, end_column => 2 }]
                    },
                    {
                        code => "obj.\nproperty",
                        output => "obj\n.property",
                        options => "property",
                        errors => [{ message_id => "expected_dot_before_property", type => MemberExpression, line => 1, column => 4, end_line => 1, end_column => 5 }]
                    },
                    {
                        code => "(obj).\nproperty",
                        output => "(obj)\n.property",
                        options => "property",
                        errors => [{ message_id => "expected_dot_before_property", type => MemberExpression, line => 1, column => 6 }]
                    },
                    {
                        code => "5\n.toExponential()",
                        output => "5 .\ntoExponential()",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "-5\n.toExponential()",
                        output => "-5 .\ntoExponential()",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "01\n.toExponential()",
                        output => "01.\ntoExponential()",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "08\n.toExponential()",
                        output => "08 .\ntoExponential()",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "0190\n.toExponential()",
                        output => "0190 .\ntoExponential()",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "5_000\n.toExponential()",
                        output => "5_000 .\ntoExponential()",
                        options => "object",
                        environment => { ecma_version => 2021 },
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "5_000_00\n.toExponential()",
                        output => "5_000_00 .\ntoExponential()",
                        options => "object",
                        environment => { ecma_version => 2021 },
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "5.000_000\n.toExponential()",
                        output => "5.000_000.\ntoExponential()",
                        options => "object",
                        environment => { ecma_version => 2021 },
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "0b1010_1010\n.toExponential()",
                        output => "0b1010_1010.\ntoExponential()",
                        options => "object",
                        environment => { ecma_version => 2021 },
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },
                    {
                        code => "foo /* a */ . /* b */ \n /* c */ bar",
                        output => "foo /* a */  /* b */ \n /* c */ .bar",
                        options => "property",
                        errors => [{ message_id => "expected_dot_before_property", type => MemberExpression, line => 1, column => 13 }]
                    },
                    {
                        code => "foo /* a */ \n /* b */ . /* c */ bar",
                        output => "foo. /* a */ \n /* b */  /* c */ bar",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 10 }]
                    },
                    {
                        code => "f(a\n)\n.prop",
                        output => "f(a\n).\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 1 }]
                    },
                    {
                        code => "`\n`\n.prop",
                        output => "`\n`.\nprop",
                        options => "object",
                        environment => { ecma_version => 6 },
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 1 }]
                    },

                    // https://github.com/eslint/eslint/issues/11868 (also in valid)
                    {
                        code => "(a\n)\n.prop",
                        output => "(a\n).\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 1 }]
                    },
                    {
                        code => "(a\n)\n.\nprop",
                        output => "(a\n).\n\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 1 }]
                    },
                    {
                        code => "(f(a)\n)\n.prop",
                        output => "(f(a)\n).\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 1 }]
                    },
                    {
                        code => "(f(a\n)\n)\n.prop",
                        output => "(f(a\n)\n).\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 4, column => 1 }]
                    },
                    {
                        code => "((obj\n))\n.prop",
                        output => "((obj\n)).\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 1 }]
                    },
                    {
                        code => "((obj\n)\n)\n.prop",
                        output => "((obj\n)\n).\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 4, column => 1 }]
                    },
                    {
                        code => "(a\n) /* a */ \n.prop",
                        output => "(a\n). /* a */ \nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 1 }]
                    },
                    {
                        code => "(a\n)\n/* a */\n.prop",
                        output => "(a\n).\n/* a */\nprop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 4, column => 1 }]
                    },
                    {
                        code => "(a\n)\n/* a */.prop",
                        output => "(a\n).\n/* a */prop",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 3, column => 8 }]
                    },
                    {
                        code => "(5)\n.toExponential()",
                        output => "(5).\ntoExponential()",
                        options => "object",
                        errors => [{ message_id => "expected_dot_after_object", type => MemberExpression, line => 2, column => 1 }]
                    },

                    // Optional chaining
                    {
                        code => "obj\n?.prop",
                        output => "obj?.\nprop",
                        options => "object",
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "expected_dot_after_object" }]
                    },
                    {
                        code => "10\n?.prop",
                        output => "10?.\nprop",
                        options => "object",
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "expected_dot_after_object" }]
                    },
                    {
                        code => "obj?.\nprop",
                        output => "obj\n?.prop",
                        options => "property",
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "expected_dot_before_property" }]
                    },

                    // Private properties
                    {
                        code => "class C { #a; foo() { this\n.#a; } }",
                        output => "class C { #a; foo() { this.\n#a; } }",
                        options => "object",
                        environment => { ecma_version => 2022 },
                        errors => [{ message_id => "expected_dot_after_object" }]
                    },
                    {
                        code => "class C { #a; foo() { this.\n#a; } }",
                        output => "class C { #a; foo() { this\n.#a; } }",
                        options => "property",
                        environment => { ecma_version => 2022 },
                        errors => [{ message_id => "expected_dot_before_property" }]
                    }
                ]
            },
        )
    }
}
