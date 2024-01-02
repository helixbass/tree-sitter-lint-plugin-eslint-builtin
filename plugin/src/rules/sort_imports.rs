use std::sync::Arc;

use serde::{de::Error, Deserialize};
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum MemberSyntaxSortOrderItem {
    None,
    All,
    Multiple,
    Single,
}

type MemberSyntaxSortOrderArray = [MemberSyntaxSortOrderItem; 4];

struct MemberSyntaxSortOrder(MemberSyntaxSortOrderArray);

impl<'de> Deserialize<'de> for MemberSyntaxSortOrder {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let array = MemberSyntaxSortOrderArray::deserialize(deserializer)?;
        for variant in [
            MemberSyntaxSortOrderItem::None,
            MemberSyntaxSortOrderItem::All,
            MemberSyntaxSortOrderItem::Multiple,
            MemberSyntaxSortOrderItem::Single,
        ] {
            if !array.contains(&variant) {
                return Err(D::Error::custom("Expected all variants"));
            }
        }
        Ok(Self(array))
    }
}

impl Default for MemberSyntaxSortOrder {
    fn default() -> Self {
        Self([
            MemberSyntaxSortOrderItem::None,
            MemberSyntaxSortOrderItem::All,
            MemberSyntaxSortOrderItem::Multiple,
            MemberSyntaxSortOrderItem::Single,
        ])
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    ignore_case: bool,
    member_syntax_sort_order: MemberSyntaxSortOrder,
    ignore_declaration_sort: bool,
    ignore_member_sort: bool,
    allow_separated_groups: bool,
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
    use serde_json::json;
    use tree_sitter_lint::{rule_tests, RuleTester, RuleTestExpectedErrorBuilder};

    use super::*;

    #[test]
    fn test_sort_imports_rule() {
        let expected_error = RuleTestExpectedErrorBuilder::default()
            .message_id("sort_imports_alphabetically")
            .type_("ImportDeclaration")
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
                        errors => [expected_error]
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
                            type => "ImportDeclaration"
                        }]
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
                            type => "ImportDeclaration"
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
                            type => "ImportDeclaration"
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
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import {b, a, d, c} from 'foo.js';",
                        output => "import {a, b, c, d} from 'foo.js';",
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "a" },
                            type => "ImportSpecifier"
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
                            type => "ImportSpecifier"
                        }]
                    },
                    {
                        code => "import {a, B, c, D} from 'foo.js';",
                        output => "import {B, D, a, c} from 'foo.js';",
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "B" },
                            type => "ImportSpecifier"
                        }]
                    },
                    {
                        code => "import {zzzzz, /* comment */ aaaaa} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => "ImportSpecifier"
                        }]
                    },
                    {
                        code => "import {zzzzz /* comment */, aaaaa} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => "ImportSpecifier"
                        }]
                    },
                    {
                        code => "import {/* comment */ zzzzz, aaaaa} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => "ImportSpecifier"
                        }]
                    },
                    {
                        code => "import {zzzzz, aaaaa /* comment */} from 'foo.js';",
                        output => None, // not fixed due to comment
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            data => { member_name => "aaaaa" },
                            type => "ImportSpecifier"
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
                            type => "ImportSpecifier"
                        }]
                    },

                    // allowSeparatedGroups
                    {
                        code => "import b from 'b';\nimport a from 'a';",
                        output => None,
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import b from 'b';\nimport a from 'a';",
                        output => None,
                        options => {},
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import b from 'b';\nimport a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import b from 'b';import a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import b from 'b'; /* comment */ import a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import b from 'b'; // comment\nimport a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import b from 'b'; // comment 1\n/* comment 2 */import a from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import { b } from 'b'; /* comment line 1 \n comment line 2 */ import { a } from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import b\nfrom 'b'; import a\nfrom 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import { b } from \n'b'; /* comment */ import\n { a } from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import { b } from \n'b';\nimport\n { a } from 'a';",
                        output => None,
                        options => { allow_separated_groups => false },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration"
                        }]
                    },
                    {
                        code => "import c from 'c';\n\nimport b from 'b';\nimport a from 'a';",
                        output => None,
                        options => { allow_separated_groups => true },
                        errors => [{
                            message_id => "sort_imports_alphabetically",
                            type => "ImportDeclaration",
                            line => 4
                        }]
                    },
                    {
                        code => "import b from 'b';\n\nimport { c, a } from 'c';",
                        output => "import b from 'b';\n\nimport { a, c } from 'c';",
                        options => { allow_separated_groups => true },
                        errors => [{
                            message_id => "sort_members_alphabetically",
                            type => "ImportSpecifier"
                        }]
                    }
                ]
            },
        )
    }
}
