use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Copy, Clone, Default, Deserialize)]
#[serde(default)]
struct ArrayAndObject {
    array: bool,
    object: bool,
}

#[derive(Copy, Clone, Default, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
struct ByNodeType {
    variable_declarator: Option<ArrayAndObject>,
    assignment_expression: Option<ArrayAndObject>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FirstOption {
    ByNodeType(ByNodeType),
    ArrayAndObject(ArrayAndObject),
}

impl FirstOption {
    fn normalized(&self) -> ByNodeType {
        match self {
            Self::ByNodeType(value) => *value,
            Self::ArrayAndObject(value) => ByNodeType {
                variable_declarator: Some(*value),
                assignment_expression: Some(*value),
            },
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SecondOption {
    enforce_for_renamed_properties: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Single(FirstOption),
    Multiple((FirstOption, SecondOption)),
}

impl Options {
    fn enforce_for_renamed_properties(&self) -> bool {
        match self {
            Options::Multiple((_, second_option)) => second_option.enforce_for_renamed_properties,
            _ => false,
        }
    }

    fn normalized_options(&self) -> ByNodeType {
        match self {
            Self::Single(first_option) => first_option.normalized(),
            Self::Multiple((first_option, _)) => first_option.normalized(),
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Single(FirstOption::ByNodeType(ByNodeType {
            variable_declarator: Some(ArrayAndObject {
                array: true,
                object: true,
            }),
            assignment_expression: Some(ArrayAndObject {
                array: true,
                object: true,
            }),
        }))
    }
}

pub fn prefer_destructuring_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-debugger",
        languages => [Javascript],
        messages => [
            prefer_destructuring => "Use {{type}} destructuring.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            enforce_for_renamed_properties: bool = options.enforce_for_renamed_properties(),
            normalized_options: ByNodeType = options.normalized_options(),
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
    use crate::kind::VariableDeclarator;

    #[test]
    fn test_prefer_destructuring_rule() {
        RuleTester::run(
            prefer_destructuring_rule(),
            rule_tests! {
                valid => [
                    "var [foo] = array;",
                    "var { foo } = object;",
                    "var foo;",
                    {

                        // Ensure that the default behavior does not require destructuring when renaming
                        code => "var foo = object.bar;",
                        options => [{ VariableDeclarator => { object => true } }]
                    },

                    // Non-array options
                    {

                        // Ensure that the default behavior does not require destructuring when renaming
                        code => "var foo = object.bar;",
                        options => { VariableDeclarator => { object => true } }
                    },

                    {

                        // Ensure that the default behavior does not require destructuring when renaming
                        code => "var foo = object.bar;",
                        options => [{ object => true }]
                    },
                    {
                        code => "var foo = object.bar;",
                        options => [{ VariableDeclarator => { object => true } }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "var foo = object.bar;",
                        options => [{ object => true }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "var foo = object['bar'];",
                        options => [{ VariableDeclarator => { object => true } }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "var foo = object[bar];",
                        options => [{ object => true }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "var { bar: foo } = object;",
                        options => [{ VariableDeclarator => { object => true } }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "var { bar: foo } = object;",
                        options => [{ object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "var { [bar]: foo } = object;",
                        options => [{ VariableDeclarator => { object => true } }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "var { [bar]: foo } = object;",
                        options => [{ object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "var foo = array[0];",
                        options => [{ VariableDeclarator => { array => false } }]
                    },
                    {
                        code => "var foo = array[0];",
                        options => [{ array => false }]
                    },
                    {
                        code => "var foo = object.foo;",
                        options => [{ VariableDeclarator => { object => false } }]
                    },
                    {
                        code => "var foo = object['foo'];",
                        options => [{ VariableDeclarator => { object => false } }]
                    },
                    "({ foo } = object);",
                    {

                        // Fix #8654
                        code => "var foo = array[0];",
                        options => [{ VariableDeclarator => { array => false } }, { enforce_for_renamed_properties => true }]
                    },
                    {

                        // Fix #8654
                        code => "var foo = array[0];",
                        options => [{ array => false }, { enforce_for_renamed_properties => true }]
                    },
                    "[foo] = array;",
                    "foo += array[0]",
                    {
                        code => "foo &&= array[0]",
                        environment => { ecma_version => 2021 }
                    },
                    "foo += bar.foo",
                    {
                        code => "foo ||= bar.foo",
                        environment => { ecma_version => 2021 }
                    },
                    {
                        code => "foo ??= bar['foo']",
                        environment => { ecma_version => 2021 }
                    },
                    {
                        code => "foo = object.foo;",
                        options => [{ AssignmentExpression => { object => false } }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "foo = object.foo;",
                        options => [{ AssignmentExpression => { object => false } }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "foo = array[0];",
                        options => [{ AssignmentExpression => { array => false } }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "foo = array[0];",
                        options => [{ AssignmentExpression => { array => false } }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "foo = array[0];",
                        options => [{ VariableDeclarator => { array => true }, AssignmentExpression => { array => false } }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "var foo = array[0];",
                        options => [{ VariableDeclarator => { array => false }, AssignmentExpression => { array => true } }, { enforce_for_renamed_properties => false }]
                    },
                    {
                        code => "foo = object.foo;",
                        options => [{ VariableDeclarator => { object => true }, AssignmentExpression => { object => false } }]
                    },
                    {
                        code => "var foo = object.foo;",
                        options => [{ VariableDeclarator => { object => false }, AssignmentExpression => { object => true } }]
                    },
                    "class Foo extends Bar { static foo() {var foo = super.foo} }",
                    "foo = bar[foo];",
                    "var foo = bar[foo];",
                    {
                        code => "var {foo: {bar}} = object;",
                        options => [{ object => true }]
                    },
                    {
                        code => "var {bar} = object.foo;",
                        options => [{ object => true }]
                    },

                    // Optional chaining
                    "var foo = array?.[0];", // because the fixed code can throw TypeError.
                    "var foo = object?.foo;",

                    // Private identifiers
                    "class C { #x; foo() { const x = this.#x; } }",
                    "class C { #x; foo() { x = this.#x; } }",
                    "class C { #x; foo(a) { x = a.#x; } }",
                    {
                        code => "class C { #x; foo() { const x = this.#x; } }",
                        options => [{ array => true, object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "class C { #x; foo() { const y = this.#x; } }",
                        options => [{ array => true, object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "class C { #x; foo() { x = this.#x; } }",
                        options => [{ array => true, object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "class C { #x; foo() { y = this.#x; } }",
                        options => [{ array => true, object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "class C { #x; foo(a) { x = a.#x; } }",
                        options => [{ array => true, object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "class C { #x; foo(a) { y = a.#x; } }",
                        options => [{ array => true, object => true }, { enforce_for_renamed_properties => true }]
                    },
                    {
                        code => "class C { #x; foo() { x = this.a.#x; } }",
                        options => [{ array => true, object => true }, { enforce_for_renamed_properties => true }]
                    }
                ],
                invalid => [
                    {
                        code => "var foo = array[0];",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "array" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "foo = array[0];",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "array" },
                            type => "AssignmentExpression"
                        }]
                    },
                    {
                        code => "var foo = object.foo;",
                        output => "var {foo} = object;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (a, b).foo;",
                        output => "var {foo} = (a, b);",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var length = (() => {}).length;",
                        output => "var {length} = () => {};",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (a = b).foo;",
                        output => "var {foo} = a = b;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (a || b).foo;",
                        output => "var {foo} = a || b;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (f()).foo;",
                        output => "var {foo} = f();",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object.bar.foo;",
                        output => "var {foo} = object.bar;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foobar = object.bar;",
                        output => None,
                        options => [{ VariableDeclarator => { object => true } }, { enforce_for_renamed_properties => true }],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foobar = object.bar;",
                        output => None,
                        options => [{ object => true }, { enforce_for_renamed_properties => true }],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object[bar];",
                        output => None,
                        options => [{ VariableDeclarator => { object => true } }, { enforce_for_renamed_properties => true }],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object[bar];",
                        output => None,
                        options => [{ object => true }, { enforce_for_renamed_properties => true }],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object[foo];",
                        output => None,
                        options => [{ object => true }, { enforce_for_renamed_properties => true }],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object['foo'];",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "foo = object.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => "AssignmentExpression"
                        }]
                    },
                    {
                        code => "foo = object['foo'];",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => "AssignmentExpression"
                        }]
                    },
                    {
                        code => "var foo = array[0];",
                        output => None,
                        options => [{ VariableDeclarator => { array => true } }, { enforce_for_renamed_properties => true }],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "array" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "foo = array[0];",
                        output => None,
                        options => [{ AssignmentExpression => { array => true } }],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "array" },
                            type => "AssignmentExpression"
                        }]
                    },
                    {
                        code => "var foo = array[0];",
                        output => None,
                        options => [
                            {
                                VariableDeclarator => { array => true },
                                AssignmentExpression => { array => false }
                            },
                            { enforce_for_renamed_properties => true }
                        ],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "array" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = array[0];",
                        output => None,
                        options => [
                            {
                                VariableDeclarator => { array => true },
                                AssignmentExpression => { array => false }
                            }
                        ],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "array" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "foo = array[0];",
                        output => None,
                        options => [
                            {
                                VariableDeclarator => { array => false },
                                AssignmentExpression => { array => true }
                            }
                        ],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "array" },
                            type => "AssignmentExpression"
                        }]
                    },
                    {
                        code => "foo = object.foo;",
                        output => None,
                        options => [
                            {
                                VariableDeclarator => { array => true, object => false },
                                AssignmentExpression => { object => true }
                            }
                        ],
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => "AssignmentExpression"
                        }]
                    },
                    {
                        code => "class Foo extends Bar { static foo() {var bar = super.foo.bar} }",
                        output => "class Foo extends Bar { static foo() {var {bar} = super.foo} }",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },

                    // comments
                    {
                        code => "var /* comment */ foo = object.foo;",
                        output => "var /* comment */ {foo} = object;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var a, /* comment */foo = object.foo;",
                        output => "var a, /* comment */{foo} = object;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo /* comment */ = object.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var a, foo /* comment */ = object.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo /* comment */ = object.foo, a;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo // comment\n = object.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = /* comment */ object.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = // comment\n object.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (/* comment */ object).foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (object /* comment */).foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = bar(/* comment */).foo;",
                        output => "var {foo} = bar(/* comment */);",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = bar/* comment */.baz.foo;",
                        output => "var {foo} = bar/* comment */.baz;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = bar[// comment\nbaz].foo;",
                        output => "var {foo} = bar[// comment\nbaz];",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo // comment\n = bar(/* comment */).foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = bar/* comment */.baz/* comment */.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object// comment\n.foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object./* comment */foo;",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (/* comment */ object.foo);",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = (object.foo /* comment */);",
                        output => None,
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object.foo/* comment */;",
                        output => "var {foo} = object/* comment */;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object.foo// comment",
                        output => "var {foo} = object// comment",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object.foo/* comment */, a;",
                        output => "var {foo} = object/* comment */, a;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object.foo// comment\n, a;",
                        output => "var {foo} = object// comment\n, a;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    },
                    {
                        code => "var foo = object.foo, /* comment */ a;",
                        output => "var {foo} = object, /* comment */ a;",
                        errors => [{
                            message_id => "prefer_destructuring",
                            data => { type => "object" },
                            type => VariableDeclarator
                        }]
                    }
                ]
            },
        )
    }
}
