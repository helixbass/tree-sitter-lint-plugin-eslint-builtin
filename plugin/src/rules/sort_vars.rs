use std::sync::Arc;

use itertools::Itertools;
use serde::Deserialize;
use squalid::OptionExt;
use tree_sitter_lint::{
    range_between_start_and_end, rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage,
    violation, NodeExt, Rule, SourceTextProvider,
};

use crate::kind::{is_literal_kind, Identifier};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    ignore_case: Option<bool>,
}

impl Options {
    fn ignore_case(&self) -> bool {
        self.ignore_case.unwrap_or_default()
    }
}

pub fn sort_vars_rule() -> Arc<dyn Rule> {
    rule! {
        name => "sort-vars",
        languages => [Javascript],
        messages => [
            sort_vars => "Variables within the same declaration block should be sorted alphabetically.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            ignore_case: bool = options.ignore_case(),
        },
        listeners => [
            r#"
              (variable_declaration) @c
            "# => |node, context| {
                let id_declarations = node.non_comment_named_children(SupportedLanguage::Javascript)
                    .filter(|decl| decl.field("name").kind() == Identifier)
                    .collect_vec();
                let get_sortable_name = |decl: &Node| {
                    if self.ignore_case {
                        decl.field("name").text(context)
                    } else {
                        decl.field("name").text(context).to_lowercase().into()
                    }
                };
                let unfixable = id_declarations.iter().any(|decl| {
                    decl.child_by_field_name("value").matches(|init| {
                        !is_literal_kind(init.kind())
                    })
                });
                let mut fixed = false;

                id_declarations.iter().reduce(|memo, decl| {
                    let last_variable_name = get_sortable_name(memo);
                    let current_variable_name = get_sortable_name(decl);

                    if current_variable_name < last_variable_name {
                        context.report(violation! {
                            node => *decl,
                            message_id => "sort_vars",
                            fix => |fixer| {
                                if unfixable || fixed {
                                    return;
                                }
                                fixer.replace_text_range(
                                    range_between_start_and_end(
                                        id_declarations[0].range(),
                                        id_declarations.last().unwrap().range(),
                                    ),
                                    id_declarations
                                        .iter()
                                        .sorted_by_key(|node| get_sortable_name(node))
                                        .enumerate()
                                        .fold("".to_owned(), |mut source_text, (index, identifier)| {
                                            let text_after_identifier = if index == id_declarations.len() - 1 {
                                                "".into()
                                            } else {
                                                context.file_run_context.file_contents.slice(
                                                    id_declarations[index].end_byte()..id_declarations[index + 1].start_byte()
                                                )
                                            };

                                            source_text.push_str(&format!("{}{text_after_identifier}", identifier.text(context)));
                                            source_text
                                        })
                                );
                            }
                        });
                        fixed = true;
                        return memo;
                    }
                    decl
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::kind::VariableDeclarator;

    #[test]
    fn test_sort_vars_rule() {
        let expected_error = RuleTestExpectedErrorBuilder::default()
            .message_id("sort_vars")
            .type_(VariableDeclarator)
            .build()
            .unwrap();
        let ignore_case_args = json!({ "ignore_case": true });

        RuleTester::run(
            sort_vars_rule(),
            rule_tests! {
                valid => [
                    "var a=10, b=4, c='abc'",
                    "var a, b, c, d",
                    "var b; var a; var d;",
                    "var _a, a",
                    "var A, a",
                    "var A, b",
                    { code => "var a, A;", options => ignore_case_args },
                    { code => "var A, a;", options => ignore_case_args },
                    { code => "var a, B, c;", options => ignore_case_args },
                    { code => "var A, b, C;", options => ignore_case_args },
                    { code => "var {a, b, c} = x;", options => ignore_case_args, environment => { ecma_version => 6 } },
                    { code => "var {A, b, C} = x;", options => ignore_case_args, environment => { ecma_version => 6 } },
                    { code => "var test = [1,2,3];", environment => { ecma_version => 6 } },
                    { code => "var {a,b} = [1,2];", environment => { ecma_version => 6 } },
                    {
                        code => "var [a, B, c] = [1, 2, 3];",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var [A, B, c] = [1, 2, 3];",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var [A, b, C] = [1, 2, 3];",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    },
                    { code => "let {a, b, c} = x;", environment => { ecma_version => 6 } },
                    {
                        code => "let [a, b, c] = [1, 2, 3];",
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "const {a, b, c} = {a: 1, b: true, c: \"Moo\"};",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "const [a, b, c] = [1, true, \"Moo\"];",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "const [c, a, b] = [1, true, \"Moo\"];",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    },
                    { code => "var {a, x: {b, c}} = {};", environment => { ecma_version => 6 } },
                    { code => "var {c, x: {a, c}} = {};", environment => { ecma_version => 6 } },
                    { code => "var {a, x: [b, c]} = {};", environment => { ecma_version => 6 } },
                    { code => "var [a, {b, c}] = {};", environment => { ecma_version => 6 } },
                    { code => "var [a, {x: {b, c}}] = {};", environment => { ecma_version => 6 } },
                    { code => "var a = 42, {b, c } = {};", environment => { ecma_version => 6 } },
                    { code => "var b = 42, {a, c } = {};", environment => { ecma_version => 6 } },
                    { code => "var [b, {x: {a, c}}] = {};", environment => { ecma_version => 6 } },
                    { code => "var [b, d, a, c] = {};", environment => { ecma_version => 6 } },
                    { code => "var e, [a, c, d] = {};", environment => { ecma_version => 6 } },
                    {
                        code => "var a, [E, c, D] = [];",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    },
                    { code => "var a, f, [e, c, d] = [1,2,3];", environment => { ecma_version => 6 } },
                    {
                        code =>
                            "export default class {
                                render () {
                                    let {
                                        b
                                    } = this,
                                        a,
                                        c;
                                }
                            }",
                        environment => { ecma_version => 6, source_type => "module" }
                    },

                    {
                        code => "var {} = 1, a",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 }
                    }
                ],
                invalid => [
                    {
                        code => "var b, a",
                        output => "var a, b",
                        errors => [expected_error]
                    },
                    {
                        code => "var b , a",
                        output => "var a , b",
                        errors => [expected_error]
                    },
                    {
                        code => [
                            "var b,",
                            "    a;"
                        ].join("\n"),
                        output => [
                            "var a,",
                            "    b;"
                        ].join("\n"),
                        errors => [expected_error]
                    },
                    {
                        code => "var b=10, a=20;",
                        output => "var a=20, b=10;",
                        errors => [expected_error]
                    },
                    {
                        code => "var b=10, a=20, c=30;",
                        output => "var a=20, b=10, c=30;",
                        errors => [expected_error]
                    },
                    {
                        code => "var all=10, a = 1",
                        output => "var a = 1, all=10",
                        errors => [expected_error]
                    },
                    {
                        code => "var b, c, a, d",
                        output => "var a, b, c, d",
                        errors => [expected_error]
                    },
                    {
                        code => "var c, d, a, b",
                        output => "var a, b, c, d",
                        errors => 2
                    },
                    {
                        code => "var a, A;",
                        output => "var A, a;",
                        errors => [expected_error]
                    },
                    {
                        code => "var a, B;",
                        output => "var B, a;",
                        errors => [expected_error]
                    },
                    {
                        code => "var a, B, c;",
                        output => "var B, a, c;",
                        errors => [expected_error]
                    },
                    {
                        code => "var B, a;",
                        output => "var a, B;",
                        options => ignore_case_args,
                        errors => [expected_error]
                    },
                    {
                        code => "var B, A, c;",
                        output => "var A, B, c;",
                        options => ignore_case_args,
                        errors => [expected_error]
                    },
                    {
                        code => "var d, a, [b, c] = {};",
                        output => "var a, d, [b, c] = {};",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 },
                        errors => [expected_error]
                    },
                    {
                        code => "var d, a, [b, {x: {c, e}}] = {};",
                        output => "var a, d, [b, {x: {c, e}}] = {};",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 },
                        errors => [expected_error]
                    },
                    {
                        code => "var {} = 1, b, a",
                        output => "var {} = 1, a, b",
                        options => ignore_case_args,
                        environment => { ecma_version => 6 },
                        errors => [expected_error]
                    },
                    {
                        code => "var b=10, a=f();",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code => "var b=10, a=b;",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code => "var b = 0, a = `${b}`;",
                        output => None,
                        environment => { ecma_version => 6 },
                        errors => [expected_error]
                    },
                    {
                        code => "var b = 0, a = `${f()}`",
                        output => None,
                        environment => { ecma_version => 6 },
                        errors => [expected_error]
                    },
                    {
                        code => "var b = 0, c = b, a;",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code => "var b = 0, c = 0, a = b + c;",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code => "var b = f(), c, d, a;",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code => "var b = `${f()}`, c, d, a;",
                        output => None,
                        environment => { ecma_version => 6 },
                        errors => [expected_error]
                    },
                    {
                        code => "var c, a = b = 0",
                        output => None,
                        errors => [expected_error]
                    }
                ]
            },
        )
    }
}
