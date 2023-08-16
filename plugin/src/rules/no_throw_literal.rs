use std::sync::Arc;

use tree_sitter_lint::{rule, violation, Rule};

use crate::{ast_helpers::NodeExtJs, kind::Undefined, utils::ast_utils};

pub fn no_throw_literal_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-throw-literal",
        languages => [Javascript],
        messages => [
            object => "Expected an error object to be thrown.",
            undef => "Do not throw undefined.",
        ],
        listeners => [
            r#"
              (throw_statement) @c
            "# => |node, context| {
                let argument = node.first_non_comment_named_child();
                if !ast_utils::could_be_error(argument, context) {
                    context.report(violation! {
                        node => node,
                        message_id => "object",
                    });
                } else if argument.kind() == Undefined {
                    context.report(violation! {
                        node => node,
                        message_id => "undef",
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::ThrowStatement;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_throw_literal_rule() {
        RuleTester::run(
            no_throw_literal_rule(),
            rule_tests! {
                valid => [
                    "throw new Error();",
                    "throw new Error('error');",
                    "throw Error('error');",
                    "var e = new Error(); throw e;",
                    "try {throw new Error();} catch (e) {throw e;};",
                    "throw a;", // Identifier
                    "throw foo();", // CallExpression
                    "throw new foo();", // NewExpression
                    "throw foo.bar;", // MemberExpression
                    "throw foo[bar];", // MemberExpression
                    { code => "class C { #field; foo() { throw foo.#field; } }", /*parserOptions: { ecmaVersion: 2022 }*/ }, // MemberExpression
                    "throw foo = new Error();", // AssignmentExpression with the `=` operator
                    { code => "throw foo.bar ||= 'literal'", /*parserOptions: { ecmaVersion: 2021 }*/ }, // AssignmentExpression with a logical operator
                    { code => "throw foo[bar] ??= 'literal'", /*parserOptions: { ecmaVersion: 2021 }*/ }, // AssignmentExpression with a logical operator
                    "throw 1, 2, new Error();", // SequenceExpression
                    "throw 'literal' && new Error();", // LogicalExpression (right)
                    "throw new Error() || 'literal';", // LogicalExpression (left)
                    "throw foo ? new Error() : 'literal';", // ConditionalExpression (consequent)
                    "throw foo ? 'literal' : new Error();", // ConditionalExpression (alternate)
                    { code => "throw tag `${foo}`;", /*parserOptions: { ecmaVersion: 6 }*/ }, // TaggedTemplateExpression
                    { code => "function* foo() { var index = 0; throw yield index++; }", /*parserOptions: { ecmaVersion: 6 }*/ }, // YieldExpression
                    { code => "async function foo() { throw await bar; }", /*parserOptions: { ecmaVersion: 8 }*/ }, // AwaitExpression
                    { code => "throw obj?.foo", /*parserOptions: { ecmaVersion: 2020 }*/ }, // ChainExpression
                    { code => "throw obj?.foo()", /*parserOptions: { ecmaVersion: 2020 }*/ } // ChainExpression
                ],
                invalid => [
                    {
                        code => "throw 'error';",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw 0;",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw false;",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw null;",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw {};",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw undefined;",
                        errors => [{
                            message_id => "undef",
                            type => ThrowStatement
                        }]
                    },

                    // String concatenation
                    {
                        code => "throw 'a' + 'b';",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "var b = new Error(); throw 'a' + b;",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },

                    // AssignmentExpression
                    {
                        code => "throw foo = 'error';", // RHS is a literal
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw foo += new Error();", // evaluates to a primitive value, or throws while evaluating
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw foo &= new Error();", // evaluates to a primitive value, or throws while evaluating
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw foo &&= 'literal'", // evaluates either to a falsy value of `foo` (which, then, cannot be an Error object), or to 'literal'
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },

                    // SequenceExpression
                    {
                        code => "throw new Error(), 1, 2, 3;",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },

                    // LogicalExpression
                    {
                        code => "throw 'literal' && 'not an Error';",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },
                    {
                        code => "throw foo && 'literal'", // evaluates either to a falsy value of `foo` (which, then, cannot be an Error object), or to 'literal'
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },

                    // ConditionalExpression
                    {
                        code => "throw foo ? 'not an Error' : 'literal';",
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    },

                    // TemplateLiteral
                    {
                        code => "throw `${err}`;",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "object",
                            type => ThrowStatement
                        }]
                    }
                ]
            },
        )
    }
}
