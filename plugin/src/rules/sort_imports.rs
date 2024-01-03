use std::{borrow::Cow, sync::Arc};

use grouped_ordering::grouped_ordering;
use itertools::Itertools;
use serde::Deserialize;
use squalid::{EverythingExt, OptionExt};
use tree_sitter_lint::{
    range_between_start_and_end, rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage,
    violation, NodeExt, QueryMatchContext, Rule, SourceTextProvider,
};

use crate::{
    assert_kind,
    ast_helpers::get_num_import_specifiers,
    kind::{
        Identifier, ImportClause, ImportSpecifier, ImportStatement, NamedImports, NamespaceImport,
    },
};

grouped_ordering!(MemberSyntaxSortOrder, [None, All, Multiple, Single,]);

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    ignore_case: bool,
    member_syntax_sort_order: MemberSyntaxSortOrder,
    ignore_declaration_sort: bool,
    ignore_member_sort: bool,
    allow_separated_groups: bool,
}

fn used_member_syntax(node: Node) -> MemberSyntaxSortOrderGroup {
    let Some(import_clause) = node.maybe_first_child_of_kind(ImportClause) else {
        return MemberSyntaxSortOrderGroup::None;
    };
    let first_child = import_clause.first_non_comment_named_child(SupportedLanguage::Javascript);
    if first_child.kind() == NamespaceImport {
        return MemberSyntaxSortOrderGroup::All;
    }
    match get_num_import_specifiers(import_clause) {
        1 => MemberSyntaxSortOrderGroup::Single,
        num_specifiers if num_specifiers > 1 => MemberSyntaxSortOrderGroup::Multiple,
        _ => unreachable!(),
    }
}

fn get_import_specifier_local_name<'a>(
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> Cow<'a, str> {
    assert_kind!(node, ImportSpecifier);

    node.child_by_field_name("alias").map_or_else(
        || node.field("name").text(context),
        |alias| alias.text(context),
    )
}

fn get_first_local_member_name<'a>(
    node: Node<'a>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<Cow<'a, str>> {
    assert_kind!(node, ImportStatement);
    let import_clause = node.maybe_first_child_of_kind(ImportClause)?;
    let first_child = import_clause.first_non_comment_named_child(SupportedLanguage::Javascript);
    match first_child.kind() {
        Identifier => Some(first_child.text(context)),
        NamespaceImport => Some(
            first_child
                .first_non_comment_named_child(SupportedLanguage::Javascript)
                .text(context),
        ),
        NamedImports => first_child
            .maybe_first_non_comment_named_child(SupportedLanguage::Javascript)
            .map(|first_named_import| get_import_specifier_local_name(first_named_import, context)),
        _ => unreachable!(),
    }
}

fn get_number_of_lines_between(left: Node, right: Node) -> usize {
    match right.end_position().row - left.end_position().row {
        0 => 0,
        num_lines => num_lines - 1,
    }
}

pub fn sort_imports_rule() -> Arc<dyn Rule> {
    rule! {
        name => "sort-imports",
        languages => [Javascript],
        messages => [
            sort_imports_alphabetically => "Imports should be sorted alphabetically.",
            sort_members_alphabetically => "Member '{{member_name}}' of the import declaration should be sorted alphabetically.",
            unexpected_syntax_order => "Expected '{{syntax_a}}' syntax before '{{syntax_b}}' syntax.",
        ],
        fixable => true,
        options_type => Options,
        state => {
            [per-config]
            ignore_case: bool = options.ignore_case,
            member_syntax_sort_order: MemberSyntaxSortOrder = options.member_syntax_sort_order,
            ignore_declaration_sort: bool = options.ignore_declaration_sort,
            ignore_member_sort: bool = options.ignore_member_sort,
            allow_separated_groups: bool = options.allow_separated_groups,

            [per-file-run]
            previous_declaration: Option<Node<'a>>,
        },
        methods => {
            fn get_member_parameter_group_index(&self, node: Node) -> usize {
                self.member_syntax_sort_order[used_member_syntax(node)]
            }

            fn get_sortable_name(&self, specifier: Node<'a>, context: &QueryMatchContext<'a, '_>) -> Cow<'a, str> {
                get_import_specifier_local_name(specifier, context).thrush(|name| {
                    if self.ignore_case {
                        name.to_lowercase().into()
                    } else {
                        name
                    }
                })
            }
        },
        listeners => [
            r#"
              (import_statement) @c
            "# => |node, context| {
                if !self.ignore_declaration_sort {
                    if matches!(
                        self.previous_declaration,
                        // TODO: shouldn't have to say self.rule_instance, this is presumably
                        // because we're inside matches!() macro
                        Some(previous_declaration) if self.rule_instance.allow_separated_groups
                            && get_number_of_lines_between(previous_declaration, node) > 0
                    ) {
                        self.previous_declaration = None;
                    }

                    if let Some(previous_declaration) = self.previous_declaration {
                        let current_member_syntax_group_index = self.get_member_parameter_group_index(node);
                        let previous_member_syntax_group_index = self.get_member_parameter_group_index(previous_declaration);
                        let mut current_local_member_name = get_first_local_member_name(node, context);
                        let mut previous_local_member_name = get_first_local_member_name(previous_declaration, context);

                        if self.ignore_case {
                            previous_local_member_name = previous_local_member_name.map(|previous_local_member_name| {
                                previous_local_member_name.to_lowercase().into()
                            });
                            current_local_member_name = current_local_member_name.map(|current_local_member_name| {
                                current_local_member_name.to_lowercase().into()
                            });
                        }

                        #[allow(clippy::collapsible_else_if)]
                        if current_member_syntax_group_index != previous_member_syntax_group_index {
                            if current_member_syntax_group_index < previous_member_syntax_group_index {
                                context.report(violation! {
                                    node => node,
                                    message_id => "unexpected_syntax_order",
                                    data => {
                                        syntax_a => format!("{:?}", self.member_syntax_sort_order[current_member_syntax_group_index]).to_lowercase(),
                                        syntax_b => format!("{:?}", self.member_syntax_sort_order[previous_member_syntax_group_index]).to_lowercase(),
                                    }
                                });
                            }
                        } else {
                            if matches!(
                                (previous_local_member_name, current_local_member_name),
                                (Some(previous_local_member_name), Some(current_local_member_name)) if
                                    current_local_member_name < previous_local_member_name
                            ) {
                                context.report(violation! {
                                    node => node,
                                    message_id => "sort_imports_alphabetically",
                                });
                            }
                        }
                    }

                    self.previous_declaration = Some(node);
                }

                if !self.ignore_member_sort {
                    let import_specifiers = node.maybe_first_child_of_kind(ImportClause)
                        .and_then(|import_clause| import_clause.maybe_first_child_of_kind(NamedImports))
                        .map_or_default(|named_imports| {
                            named_imports.non_comment_named_children(SupportedLanguage::Javascript)
                                .collect_vec()
                        });
                    let import_specifier_names = import_specifiers.iter().map(|&import_specifier| {
                        self.get_sortable_name(import_specifier, context)
                    }).collect_vec();
                    let Some(first_unsorted_index) = import_specifier_names.iter().enumerate().position(|(index, name)| {
                        index > 0 && &import_specifier_names[index - 1] > name
                    }) else {
                        return
                    };

                    context.report(violation! {
                        node => import_specifiers[first_unsorted_index],
                        message_id => "sort_members_alphabetically",
                        data => {
                            member_name => get_import_specifier_local_name(import_specifiers[first_unsorted_index], context),
                        },
                        fix => |fixer| {
                            if import_specifiers.iter().any(|&specifier| {
                                context.get_comments_before(specifier).next().is_some() ||
                                    context.get_comments_after(specifier).next().is_some()
                            }) {
                                return;
                            }

                            fixer.replace_text_range(
                                range_between_start_and_end(
                                    import_specifiers[0].range(),
                                    import_specifiers.last().unwrap().range()
                                ),
                                import_specifiers
                                    .iter()
                                    .sorted_by_key(|&&specifier| self.get_sortable_name(specifier, context))
                                    .enumerate()
                                    .fold("".to_owned(), |mut source_text, (index, &specifier)| {
                                        let text_after_specifier = if index == import_specifiers.len() - 1 {
                                            "".into()
                                        } else {
                                            context.slice(import_specifiers[index].end_byte()..import_specifiers[index + 1].start_byte())
                                        };

                                        source_text.push_str(&specifier.text(context));
                                        source_text.push_str(&text_after_specifier);
                                        source_text
                                    })
                            );
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;

    #[test]
    fn test_sort_imports_rule() {
        let expected_error = RuleTestExpectedErrorBuilder::default()
            .message_id("sort_imports_alphabetically")
            .type_(ImportStatement)
            .build()
            .unwrap();
        let ignore_case_args = json!({"ignore_case": true});

        RuleTester::run(
            sort_imports_rule(),
            rule_tests! {
                valid => [
                    "import a from 'foo.js';
                    import b from 'bar.js';
                    import c from 'baz.js';",
                    "import * as B from 'foo.js';
                    import A from 'bar.js';",
                    "import * as B from 'foo.js';
                    import {a, b} from 'bar.js';",
                    "import {b, c} from 'bar.js';
                    import A from 'foo.js';",
                    {
                        code =>
                            "import A from 'bar.js';
                            import {b, c} from 'foo.js';",
                        options => {
                            member_syntax_sort_order => ["single", "multiple", "none", "all"]
                        },
                    },
                    "import {a, b} from 'bar.js';
                    import {c, d} from 'foo.js';",
                    "import A from 'foo.js';
                    import B from 'bar.js';",
                    "import A from 'foo.js';
                    import a from 'bar.js';",
                    "import a, * as b from 'foo.js';
                    import c from 'bar.js';",
                    "import 'foo.js';
                     import a from 'bar.js';",
                    "import B from 'foo.js';
                    import a from 'bar.js';",
                    {
                        code =>
                            "import a from 'foo.js';
                            import B from 'bar.js';",
                        options => ignore_case_args
                    },
                    "import {a, b, c, d} from 'foo.js';",
                    {
                        code =>
                            "import a from 'foo.js';
                            import B from 'bar.js';",
                        options => {
                            ignore_declaration_sort => true
                        }
                    },
                    {
                        code => "import {b, A, C, d} from 'foo.js';",
                        options => {
                            ignore_member_sort => true
                        }
                    },
                    {
                        code => "import {B, a, C, d} from 'foo.js';",
                        options => {
                            ignore_member_sort => true
                        }
                    },
                    {
                        code => "import {a, B, c, D} from 'foo.js';",
                        options => ignore_case_args
                    },
                    "import a, * as b from 'foo.js';",
                    "import * as a from 'foo.js';

                    import b from 'bar.js';",
                    "import * as bar from 'bar.js';
                    import * as foo from 'foo.js';",

                    // https://github.com/eslint/eslint/issues/5130
                    {
                        code =>
                            "import 'foo';
                            import bar from 'bar';",
                        options => ignore_case_args
                    },

                    // https://github.com/eslint/eslint/issues/5305
                    "import React, {Component} from 'react';",

                    // allowSeparatedGroups
                    {
                        code => "import b from 'b';\n\nimport a from 'a';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import a from 'a';\n\nimport 'b';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import { b } from 'b';\n\n\nimport { a } from 'a';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import b from 'b';\n// comment\nimport a from 'a';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import b from 'b';\nfoo();\nimport a from 'a';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import { b } from 'b';/*\n comment \n*/import { a } from 'a';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import b from\n'b';\n\nimport\n a from 'a';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import c from 'c';\n\nimport a from 'a';\nimport b from 'b';",
                        options => { allow_separated_groups => true }
                    },
                    {
                        code => "import c from 'c';\n\nimport b from 'b';\n\nimport a from 'a';",
                        options => { allow_separated_groups => true }
                    }
                ],
                invalid => [
                    {
                        code =>
                            "import a from 'foo.js';
                            import A from 'bar.js';",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code =>
                            "import b from 'foo.js';
                            import a from 'bar.js';",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code =>
                            "import {b, c} from 'foo.js';
                            import {a, d} from 'bar.js';",
                        output => None,
                        errors => [expected_error]
                    },
                    {
                        code =>
                            "import * as foo from 'foo.js';
                            import * as bar from 'bar.js';",
                        output => None,
                        errors => [expected_error],
                    },
                    {
                        code =>
                            "import a from 'foo.js';
                            import {b, c} from 'bar.js';",
                        output => None,
                        errors => [{
                            message_id => "unexpected_syntax_order",
                            data => {
                                syntax_a => "multiple",
                                syntax_b => "single"
                            },
                            type => ImportStatement
                        }],
                    },
                    {
                        code =>
                            "import a from 'foo.js';
                            import * as b from 'bar.js';",
                        output => None,
                        errors => [{
                            message_id => "unexpected_syntax_order",
                            data => {
                                syntax_a => "all",
                                syntax_b => "single"
                            },
                            type => ImportStatement
                        }]
                    },
                    {
                        code =>
                            "import a from 'foo.js';
                            import 'bar.js';",
                        output => None,
                        errors => [{
                            message_id => "unexpected_syntax_order",
                            data => {
                                syntax_a => "none",
                                syntax_b => "single"
                            },
                            type => ImportStatement
                        }]
                    },
                    {
                        code =>
                            "import b from 'bar.js';
                            import * as a from 'foo.js';",
                        output => None,
                        options => {
                            member_syntax_sort_order => ["all", "single", "multiple", "none"]
                        },
                        errors => [{
                            message_id => "unexpected_syntax_order",
                            data => {
                                syntax_a => "all",
                                syntax_b => "single"
                            },
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import {b, a, d, c} from 'foo.js';",
                        output => "import {a, b, c, d} from 'foo.js';",
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "a" },
                            type => ImportSpecifier
                        }]
                    },
                    {
                        code =>
                            "import {b, a, d, c} from 'foo.js';
            import {e, f, g, h} from 'bar.js';",
                        output =>
                            "import {a, b, c, d} from 'foo.js';
            import {e, f, g, h} from 'bar.js';",
                        options => {
                            ignore_declaration_sort => true
                        },
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "a" },
                            type => ImportSpecifier
                        }]
                    },
                    {
                        code => "import {a, B, c, D} from 'foo.js';",
                        output => "import {B, D, a, c} from 'foo.js';",
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "B" },
                            type => ImportSpecifier
                        }]
                    },
                    {
                        code => "import {zzzzz, /* comment */ aaaaa} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => ImportSpecifier
                        }],
                    },
                    {
                        code => "import {zzzzz /* comment */, aaaaa} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => ImportSpecifier
                        }]
                    },
                    {
                        code => "import {/* comment */ zzzzz, aaaaa} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => ImportSpecifier
                        }]
                    },
                    {
                        code => "import {zzzzz, aaaaa /* comment */} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => ImportSpecifier
                        }]
                    },
                    {
                        code => r#"
                          import {
                            boop,
                            foo,
                            zoo,
                            baz as qux,
                            bar,
                            beep
                          } from 'foo.js';
                        "#,
                        output => r#"
                          import {
                            bar,
                            beep,
                            boop,
                            foo,
                            baz as qux,
                            zoo
                          } from 'foo.js';
                        "#,
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "qux" },
                            type => ImportSpecifier
                        }]
                    },

                    // allowSeparatedGroups
                    {
                        code => "import b from 'b';\nimport a from 'a';",
                        output => None,
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import b from 'b';\nimport a from 'a';",
                        output => None,
                        options => {},
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import b from 'b';\nimport a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import b from 'b';import a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import b from 'b'; /* comment */ import a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import b from 'b'; // comment\nimport a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import b from 'b'; // comment 1\n/* comment 2 */import a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import { b } from 'b'; /* comment line 1 \n comment line 2 */ import { a } from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    {
                        code => "import b\nfrom 'b'; import a\nfrom 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement
                        }]
                    },
                    // TODO: uncomment these if https://github.com/tree-sitter/tree-sitter-javascript/issues/283 is resolved
                    // {
                    //     code => "import { b } from \n'b'; /* comment */ import\n { a } from 'a';",
                    //     output => None,
                    //     options => { allow_separated_groups => false },
                    //     errors => [{
                    //         message_id => "sort_imports_alphabetically",
                    //         type => ImportStatement
                    //     }],
                    // },
                    // {
                    //     code => "import { b } from \n'b';\nimport\n { a } from 'a';",
                    //     output => None,
                    //     options => { allow_separated_groups => false },
                    //     errors => [{
                    //         message_id => "sort_imports_alphabetically",
                    //         type => ImportStatement
                    //     }]
                    // },
                    {
                        code => "import c from 'c';\n\nimport b from 'b';\nimport a from 'a';",
                        output => None,
                        options => { allow_separated_groups => true },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => ImportStatement,
                            line => 4
                        }]
                    },
                    {
                        code => "import b from 'b';\n\nimport { c, a } from 'c';",
                        output => "import b from 'b';\n\nimport { a, c } from 'c';",
                        options => { allow_separated_groups => true },
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            type => ImportSpecifier
                        }]
                    }
                ]
            },
        )
    }
}
