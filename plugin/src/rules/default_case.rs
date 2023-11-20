use std::{borrow::Cow, sync::Arc};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter_grep::SupportedLanguage, violation, NodeExt, Rule};

use crate::{ast_helpers::get_comment_contents, kind::SwitchDefault};

static DEFAULT_COMMENT_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?i)^no default$"#).unwrap());

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    comment_pattern: Option<String>,
}

pub fn default_case_rule() -> Arc<dyn Rule> {
    rule! {
        name => "default-case",
        languages => [Javascript],
        messages => [
            missing_default_case => "Expected a default case.",
        ],
        options_type => Options,
        state => {
            [per-config]
            comment_pattern: Cow<'static, Regex> = options.comment_pattern.map_or_else(
                || Cow::Borrowed(&*DEFAULT_COMMENT_PATTERN),
                |comment_pattern| Cow::Owned(Regex::new(&comment_pattern).unwrap())
            ),
        },
        listeners => [
            r#"
              (switch_statement) @c
            "# => |node, context| {
                let body = node.field("body");
                let cases = body.non_comment_named_children(SupportedLanguage::Javascript).collect::<Vec<_>>();
                if cases.is_empty() {
                    return;
                }
                let has_default = cases.iter().any(|v| v.kind() == SwitchDefault);
                if has_default {
                    return;
                }

                if context
                    .get_comments_after(*cases.last().unwrap())
                    .last()
                    .filter(|comment| {
                        self.comment_pattern
                            .is_match(get_comment_contents(*comment, context).trim())
                    })
                    .is_none()
                {
                    context.report(violation! {
                        node => node,
                        message_id => "missing_default_case",
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::SwitchStatement;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_default_case_rule() {
        RuleTester::run(
            default_case_rule(),
            rule_tests! {
            valid => [
                "switch (a) { case 1: break; default: break; }",
                "switch (a) { case 1: break; case 2: default: break; }",
                "switch (a) { case 1: break; default: break; \n //no default \n }",
                "switch (a) { \n    case 1: break; \n\n//oh-oh \n // no default\n }",
                "switch (a) { \n    case 1: \n\n// no default\n }",
                "switch (a) { \n    case 1: \n\n// No default\n }",
                "switch (a) { \n    case 1: \n\n// no deFAUlt\n }",
                "switch (a) { \n    case 1: \n\n// NO DEFAULT\n }",
                "switch (a) { \n    case 1: a = 4; \n\n// no default\n }",
                "switch (a) { \n    case 1: a = 4; \n\n/* no default */\n }",
                "switch (a) { \n    case 1: a = 4; break; break; \n\n// no default\n }",
                "switch (a) { // no default\n }",
                "switch (a) { }",
                {
                    code => "switch (a) { case 1: break; default: break; }",
                    options => {
                        comment_pattern => "default case omitted"
                    }
                },
                {
                    code => "switch (a) { case 1: break; \n // skip default case \n }",
                    options => {
                        comment_pattern => "^skip default"
                    }
                },
                {
                    code => "switch (a) { case 1: break; \n /*\nTODO:\n throw error in default case\n*/ \n }",
                    options => {
                        comment_pattern => "default"
                    }
                },
                {
                    code => "switch (a) { case 1: break; \n// \n }",
                    options => {
                        comment_pattern => ".?"
                    }
                }
            ],
            invalid => [
                {
                    code => "switch (a) { case 1: break; }",
                    errors => [{
                        message_id => "missing_default_case",
                        type => SwitchStatement
                    }]
                },
                {
                    code => "switch (a) { \n // no default \n case 1: break;  }",
                    errors => [{
                        message_id => "missing_default_case",
                        type => SwitchStatement
                    }]
                },
                {
                    code => "switch (a) { case 1: break; \n // no default \n // nope \n  }",
                    errors => [{
                        message_id => "missing_default_case",
                        type => SwitchStatement
                    }]
                },
                {
                    code => "switch (a) { case 1: break; \n // no default \n }",
                    options => {
                        comment_pattern => "skipped default case"
                    },
                    errors => [{
                        message_id => "missing_default_case",
                        type => SwitchStatement
                    }]
                },
                {
                    code => "switch (a) {\ncase 1: break; \n// default omitted intentionally \n// TODO: add default case \n}",
                    options => {
                        comment_pattern => "default omitted"
                    },
                    errors => [{
                        message_id => "missing_default_case",
                        type => SwitchStatement
                    }]
                },
                {
                    code => "switch (a) {\ncase 1: break;\n}",
                    options => {
                        comment_pattern => ".?"
                    },
                    errors => [{
                        message_id => "missing_default_case",
                        type => SwitchStatement
                    }]
                }
            ]
            },
        )
    }
}
