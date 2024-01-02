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
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_sort_imports_rule() {
        RuleTester::run(
            sort_imports_rule(),
            rule_tests! {
                valid => [
                    "var test = { debugger: 1 }; test.debugger;"
                ],
                invalid => [
                    {
                        code => "if (foo) debugger",
                        output => None,
                        errors => [{ message_id => "unexpected", type => "debugger_statement" }]
                    }
                ]
            },
        )
    }
}
