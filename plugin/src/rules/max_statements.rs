use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::{kind::ClassStaticBlock, string_utils::upper_case_first, utils::ast_utils};

const DEFAULT_MAX: usize = 10;

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(untagged)]
enum Max {
    Usize(usize),
    Object(MaxObject),
}

impl Default for Max {
    fn default() -> Self {
        Self::Usize(DEFAULT_MAX)
    }
}

impl From<Max> for usize {
    fn from(value: Max) -> Self {
        match value {
            Max::Usize(value) => value,
            Max::Object(value) => value.max,
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize)]
#[serde(default)]
struct MaxObject {
    #[serde(alias = "maximum")]
    max: usize,
}

impl Default for MaxObject {
    fn default() -> Self {
        Self { max: DEFAULT_MAX }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionsVariants {
    EmptyList(),
    JustMax([Max; 1]),
    MaxAndOptionsObject(Max, OptionsObject),
}

impl Default for OptionsVariants {
    fn default() -> Self {
        Self::EmptyList()
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct OptionsObject {
    ignore_top_level_functions: bool,
}

struct Options {
    max: usize,
    ignore_top_level_functions: bool,
}

impl Options {
    pub fn from_max_and_options_object(max: Max, options_object: OptionsObject) -> Self {
        Self {
            max: max.into(),
            ignore_top_level_functions: options_object.ignore_top_level_functions,
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
                Self::from_max_and_options_object(Default::default(), Default::default())
            }
            OptionsVariants::JustMax(max) => {
                Self::from_max_and_options_object(max[0], Default::default())
            }
            OptionsVariants::MaxAndOptionsObject(max, options_object) => {
                Self::from_max_and_options_object(max, options_object)
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

struct TopLevelFunction<'a> {
    node: Node<'a>,
    count: usize,
}

fn report_if_too_many_statements(
    node: Node,
    count: usize,
    max: usize,
    context: &QueryMatchContext,
) {
    if count <= max {
        return;
    }

    let name = upper_case_first(&ast_utils::get_function_name_with_kind(node, context));

    context.report(violation! {
        node => node,
        message_id => "exceed",
        data => {
            name => name,
            count => count,
            max => max,
        }
    });
}

pub fn max_statements_rule() -> Arc<dyn Rule> {
    rule! {
        name => "max-statements",
        languages => [Javascript],
        messages => [
            exceed => "{{name}} has too many statements ({{count}}). Maximum allowed is {{max}}.",
        ],
        options_type => Options,
        state => {
            [per-config]
            max_statements: usize = options.max,
            ignore_top_level_functions: bool = options.ignore_top_level_functions,

            [per-file-run]
            function_stack: Vec<usize>,
            top_level_functions: Vec<TopLevelFunction<'a>>,
        },
        listeners => [
            r#"
              (function_declaration) @c
              (function) @c
              (arrow_function) @c
              (class_static_block) @c
              (generator_function_declaration) @c
              (generator_function) @c
              (method_definition) @c
            "# => |node, context| {
                self.function_stack.push(0);
            },
            r#"
              (statement_block) @c
            "# => |node, context| {
                *self.function_stack.last_mut().unwrap() += node.num_non_comment_named_children(SupportedLanguage::Javascript);
            },
            r#"
              function_declaration:exit,
              function:exit,
              arrow_function:exit,
              class_static_block:exit,
              generator_function_declaration:exit,
              generator_function:exit,
              method_definition:exit
            "# => |node, context| {
                let count = self.function_stack.pop().unwrap();

                if node.kind() == ClassStaticBlock {
                    return;
                }

                if self.ignore_top_level_functions && self.function_stack.is_empty() {
                    self.top_level_functions.push(TopLevelFunction {
                        node,
                        count,
                    });
                } else {
                    report_if_too_many_statements(node, count, self.max_statements, context);
                }
            },
            r#"
              program:exit
            "# => |node, context| {
                if self.top_level_functions.len() == 1 {
                    return;
                }

                self.top_level_functions.iter().for_each(|element| {
                    let count = element.count;
                    let node = element.node;

                    report_if_too_many_statements(node, count, self.max_statements, context);
                });
            }
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_max_statements_rule() {
        RuleTester::run(
            max_statements_rule(),
            rule_tests! {
                valid => [
                    { code => "function foo() { var bar = 1; function qux () { var noCount = 2; } return 3; }", options => [3] },
                    { code => "function foo() { var bar = 1; if (true) { for (;;) { var qux = null; } } else { quxx(); } return 3; }", options => [6] },
                    { code => "function foo() { var x = 5; function bar() { var y = 6; } bar(); z = 10; baz(); }", options => [5] },
                    "function foo() { var a; var b; var c; var x; var y; var z; bar(); baz(); qux(); quxx(); }",
                    { code => "(function() { var bar = 1; return function () { return 42; }; })()", options => [1, { ignore_top_level_functions => true }] },
                    { code => "function foo() { var bar = 1; var baz = 2; }", options => [1, { ignore_top_level_functions => true }] },
                    { code => "define(['foo', 'qux'], function(foo, qux) { var bar = 1; var baz = 2; })", options => [1, { ignore_top_level_functions => true }] },

                    // object property options
                    { code => "var foo = { thing: function() { var bar = 1; var baz = 2; } }", options => [2] },
                    { code => "var foo = { thing() { var bar = 1; var baz = 2; } }", options => [2], environment => { ecma_version => 6 } },
                    { code => "var foo = { ['thing']() { var bar = 1; var baz = 2; } }", options => [2], environment => { ecma_version => 6 } },
                    { code => "var foo = { thing: () => { var bar = 1; var baz = 2; } }", options => [2], environment => { ecma_version => 6 } },
                    { code => "var foo = { thing: function() { var bar = 1; var baz = 2; } }", options => [{ max => 2 }] },

                    // this rule does not apply to class static blocks, and statements in them should not count as statements in the enclosing function
                    { code => "class C { static { one; two; three; { four; five; six; } } }", options => [2], environment => { ecma_version => 2022 } },
                    { code => "function foo() { class C { static { one; two; three; { four; five; six; } } } }", options => [2], environment => { ecma_version => 2022 } },
                    { code => "class C { static { one; two; three; function foo() { 1; 2; } four; five; six; } }", options => [2], environment => { ecma_version => 2022 } },
                    { code => "class C { static { { one; two; three; function foo() { 1; 2; } four; five; six; } } }", options => [2], environment => { ecma_version => 2022 } },
                    {
                        code => "function top_level() { 1; /* 2 */ class C { static { one; two; three; { four; five; six; } } } 3;}",
                        options => [2, { ignore_top_level_functions => true }],
                        environment => { ecma_version => 2022 }
                    },
                    {
                        code => "function top_level() { 1; 2; } class C { static { one; two; three; { four; five; six; } } }",
                        options => [1, { ignore_top_level_functions => true }],
                        environment => { ecma_version => 2022 }
                    },
                    {
                        code => "class C { static { one; two; three; { four; five; six; } } } function top_level() { 1; 2; } ",
                        options => [1, { ignore_top_level_functions => true }],
                        environment => { ecma_version => 2022 }
                    },
                    {
                        code => "function foo() { let one; let two = class { static { let three; let four; let five; if (six) { let seven; let eight; let nine; } } }; }",
                        options => [2],
                        environment => { ecma_version => 2022 }
                    }
                ],
                invalid => [
                    {
                        code => "function foo() { var bar = 1; var baz = 2; var qux = 3; }",
                        options => [2],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => "3", max => 2 } }]
                    },
                    {
                        code => "var foo = () => { var bar = 1; var baz = 2; var qux = 3; };",
                        options => [2],
                        environment => { ecma_version => 6 },
                        errors => [{ message_id => "exceed", data => { name => "Arrow function", count => "3", max => 2 } }]
                    },
                    {
                        code => "var foo = function() { var bar = 1; var baz = 2; var qux = 3; };",
                        options => [2],
                        errors => [{ message_id => "exceed", data => { name => "Function", count => "3", max => 2 } }]
                    },
                    {
                        code => "function foo() { var bar = 1; if (true) { while (false) { var qux = null; } } return 3; }",
                        options => [4],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => "5", max => 4 } }]
                    },
                    {
                        code => "function foo() { var bar = 1; if (true) { for (;;) { var qux = null; } } return 3; }",
                        options => [4],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => "5", max => 4 } }]
                    },
                    {
                        code => "function foo() { var bar = 1; if (true) { for (;;) { var qux = null; } } else { quxx(); } return 3; }",
                        options => [5],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => "6", max => 5 } }]
                    },
                    {
                        code => "function foo() { var x = 5; function bar() { var y = 6; } bar(); z = 10; baz(); }",
                        options => [3],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => "5", max => 3 } }]
                    },
                    {
                        code => "function foo() { var x = 5; function bar() { var y = 6; } bar(); z = 10; baz(); }",
                        options => [4],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => "5", max => 4 } }]
                    },
                    {
                        code => ";(function() { var bar = 1; return function () { var z; return 42; }; })()",
                        options => [1, { ignore_top_level_functions => true }],
                        errors => [{ message_id => "exceed", data => { name => "Function", count => "2", max => 1 } }]
                    },
                    {
                        code => ";(function() { var bar = 1; var baz = 2; })(); (function() { var bar = 1; var baz = 2; })()",
                        options => [1, { ignore_top_level_functions => true }],
                        errors => [
                            { message_id => "exceed", data => { name => "Function", count => "2", max => 1 } },
                            { message_id => "exceed", data => { name => "Function", count => "2", max => 1 } }
                        ]
                    },
                    {
                        code => "define(['foo', 'qux'], function(foo, qux) { var bar = 1; var baz = 2; return function () { var z; return 42; }; })",
                        options => [1, { ignore_top_level_functions => true }],
                        errors => [{ message_id => "exceed", data => { name => "Function", count => "2", max => 1 } }]
                    },
                    {
                        code => "function foo() { var a; var b; var c; var x; var y; var z; bar(); baz(); qux(); quxx(); foo(); }",
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => "11", max => 10 } }]
                    },

                    // object property options
                    {
                        code => "var foo = { thing: function() { var bar = 1; var baz = 2; var baz2; } }",
                        options => [2],
                        errors => [{ message_id => "exceed", data => { name => "Method 'thing'", count => "3", max => 2 } }]
                    },
                    {
                        code => "var foo = { thing() { var bar = 1; var baz = 2; var baz2; } }",
                        options => [2],
                        environment => { ecma_version => 6 },
                        errors => [{ message_id => "exceed", data => { name => "Method 'thing'", count => "3", max => 2 } }]
                    },

                    /*
                     * TODO decide if we want this or not
                     * {
                     *     code => "var foo = { ['thing']() { var bar = 1; var baz = 2; var baz2; } }",
                     *     options => [2],
                     *     environment => { ecma_version => 6 },
                     *     errors => [{ message_id => "exceed", data => {name => "Method ''thing''", count => "3", max => 2} }]
                     * },
                     */

                    {
                        code => "var foo = { thing: () => { var bar = 1; var baz = 2; var baz2; } }",
                        options => [2],
                        environment => { ecma_version => 6 },
                        errors => [{ message_id => "exceed", data => { name => "Method 'thing'", count => "3", max => 2 } }]
                    },
                    {
                        code => "var foo = { thing: function() { var bar = 1; var baz = 2; var baz2; } }",
                        options => [{ max => 2 }],
                        errors => [{ message_id => "exceed", data => { name => "Method 'thing'", count => "3", max => 2 } }]
                    },
                    {
                        code => "function foo() { 1; 2; 3; 4; 5; 6; 7; 8; 9; 10; 11; }",
                        options => [{}],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => 11, max => 10 } }]
                    },
                    {
                        code => "function foo() { 1; }",
                        options => [{ max => 0 }],
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => 1, max => 0 } }]
                    },
                    {
                        code => "function foo() { foo_1; /* foo_ 2 */ class C { static { one; two; three; four; { five; six; seven; eight; } } } foo_3 }",
                        options => [2],
                        environment => { ecma_version => 2022 },
                        errors => [{ message_id => "exceed", data => { name => "Function 'foo'", count => 3, max => 2 } }]
                    },
                    {
                        code => "class C { static { one; two; three; four; function not_top_level() { 1; 2; 3; } five; six; seven; eight; } }",
                        options => [2, { ignore_top_level_functions => true }],
                        environment => { ecma_version => 2022 },
                        errors => [{ message_id => "exceed", data => { name => "Function 'not_top_level'", count => 3, max => 2 } }]
                    },
                    {
                        code => "class C { static { { one; two; three; four; function not_top_level() { 1; 2; 3; } five; six; seven; eight; } } }",
                        options => [2, { ignore_top_level_functions => true }],
                        environment => { ecma_version => 2022 },
                        errors => [{ message_id => "exceed", data => { name => "Function 'not_top_level'", count => 3, max => 2 } }]
                    },
                    {
                        code => "class C { static { { one; two; three; four; } function not_top_level() { 1; 2; 3; } { five; six; seven; eight; } } }",
                        options => [2, { ignore_top_level_functions => true }],
                        environment => { ecma_version => 2022 },
                        errors => [{ message_id => "exceed", data => { name => "Function 'not_top_level'", count => 3, max => 2 } }]
                    }
                ]
            },
        )
    }
}
