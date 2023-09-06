use std::sync::Arc;

use regex::Regex;
use serde::Deserialize;
use squalid::{regex, OptionExt};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, Rule, SkipOptionsBuilder};

use crate::{
    ast_helpers::{get_comment_contents, get_comment_type, CommentType},
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum AboveOrBeside {
    #[default]
    Above,
    Beside,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    SingleValue(AboveOrBeside),
    Object(OptionsObject),
}

impl Options {
    fn above(&self) -> bool {
        match self {
            Self::SingleValue(value) => *value == AboveOrBeside::Above,
            Self::Object(value) => value.position == AboveOrBeside::Above,
        }
    }

    fn ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(value) => value.ignore_pattern.clone(),
            _ => None,
        }
    }

    fn apply_default_ignore_patterns(&self) -> bool {
        match self {
            Self::Object(value) => value.apply_default_ignore_patterns,
            _ => true,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::SingleValue(Default::default())
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct OptionsObject {
    position: AboveOrBeside,
    #[serde(with = "serde_regex")]
    ignore_pattern: Option<Regex>,
    #[serde(alias = "apply_default_patterns")]
    apply_default_ignore_patterns: bool,
}

impl Default for OptionsObject {
    fn default() -> Self {
        Self {
            position: Default::default(),
            ignore_pattern: Default::default(),
            apply_default_ignore_patterns: true,
        }
    }
}

pub fn line_comment_position_rule() -> Arc<dyn Rule> {
    rule! {
        name => "line-comment-position",
        languages => [Javascript],
        messages => [
            above => "Expected comment to be above code.",
            beside => "Expected comment to be beside code.",
        ],
        options_type => Options,
        state => {
            [per-run]
            above: bool = options.above(),
            ignore_pattern: Option<Regex> = options.ignore_pattern(),
            apply_default_ignore_patterns: bool = options.apply_default_ignore_patterns(),
        },
        listeners => [
            r#"
              (comment) @c
            "# => |node, context| {
                if get_comment_type(node, context) != CommentType::Line {
                    return;
                }

                let comment_text = get_comment_contents(node, context);
                let fall_through_reg_exp = regex!(r#"^\s*falls?\s?through"#);
                if self.apply_default_ignore_patterns && (
                    ast_utils::COMMENTS_IGNORE_PATTERN.is_match(&comment_text) ||
                    fall_through_reg_exp.is_match(&comment_text)
                ) {
                    return;
                }

                if let Some(ignore_pattern) = self.ignore_pattern.as_ref() {
                    if ignore_pattern.is_match(&comment_text) {
                        return;
                    }
                }

                let previous = context.maybe_get_token_before(node, Some(SkipOptionsBuilder::<fn(Node) -> bool>::default()
                    .include_comments(true)
                    .build().unwrap()));
                let is_on_same_line = previous.matches(|previous| previous.range().end_point.row == node.range().start_point.row);

                match (self.above, is_on_same_line) {
                    (true, true) => {
                        context.report(violation! {
                            node => node,
                            message_id => "above",
                        });
                    }
                    (false, false) => {
                        context.report(violation! {
                            node => node,
                            message_id => "beside",
                        });
                    }
                    _ => ()
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::Comment;

    #[test]
    fn test_line_comment_position_rule() {
        RuleTester::run(
            line_comment_position_rule(),
            rule_tests! {
                valid => [
                    "// valid comment\n1 + 1;",
                    "/* block comments are skipped */\n1 + 1;",
                    "1 + 1; /* block comments are skipped */",
                    "1 + 1; /* eslint eqeqeq: 'error' */",
                    "1 + 1; /* eslint-disable */",
                    "1 + 1; /* eslint-enable */",
                    "1 + 1; // eslint-disable-line",
                    "// eslint-disable-next-line\n1 + 1;",
                    "1 + 1; // global MY_GLOBAL, ANOTHER",
                    "1 + 1; // globals MY_GLOBAL: true",
                    "1 + 1; // exported MY_GLOBAL, ANOTHER",
                    "1 + 1; // fallthrough",
                    "1 + 1; // fall through",
                    "1 + 1; // falls through",
                    "1 + 1; // jslint vars: true",
                    "1 + 1; // jshint ignore:line",
                    "1 + 1; // istanbul ignore next",
                    {
                        code => "1 + 1; // linter excepted comment",
                        options => { position => "above", ignore_pattern => "linter" }
                    },
                    {
                        code => "// Meep\nconsole.log('Meep');",
                        options => "above"
                    },
                    {
                        code => "1 + 1; // valid comment",
                        options => "beside"
                    },
                    {
                        code => "// jscs: disable\n1 + 1;",
                        options => "beside"
                    },
                    {
                        code => "// jscs: enable\n1 + 1;",
                        options => "beside"
                    },
                    {
                        code => "/* block comments are skipped */\n1 + 1;",
                        options => "beside"
                    },
                    {
                        code => "/*block comment*/\n/*block comment*/\n1 + 1;",
                        options => "beside"
                    },
                    {
                        code => "1 + 1; /* block comment are skipped */",
                        options => "beside"
                    },
                    {
                        code => "1 + 1; // jshint strict: true",
                        options => "beside"
                    },
                    {
                        code => "// pragma valid comment\n1 + 1;",
                        options => { position => "beside", ignore_pattern => "pragma|linter" }
                    },
                    {
                        code => "// above\n1 + 1; // ignored",
                        options => { ignore_pattern => "ignored" }
                    },
                    {
                        code => "foo; // eslint-disable-line no-alert",
                        options => { position => "above" }
                    }
                ],
                invalid => [
                    {
                        code => "1 + 1; // invalid comment",
                        errors => [{
                            message_id => "above",
                            type => Comment,
                            line => 1,
                            column => 8
                        }]
                    },
                    {
                        code => "1 + 1; // globalization is a word",
                        errors => [{
                            message_id => "above",
                            type => Comment,
                            line => 1,
                            column => 8
                        }]
                    },
                    {
                        code => "// jscs: disable\n1 + 1;",
                        options => { position => "beside", apply_default_ignore_patterns => false },
                        errors => [{
                            message_id => "beside",
                            type => Comment,
                            line => 1,
                            column => 1
                        }]
                    },
                    { // deprecated option still works
                        code => "// jscs: disable\n1 + 1;",
                        options => { position => "beside", apply_default_patterns => false },
                        errors => [{
                            message_id => "beside",
                            type => Comment,
                            line => 1,
                            column => 1
                        }]
                    },
                    // { // new option name takes precedence
                    //     code => "// jscs: disable\n1 + 1;",
                    //     options => { position => "beside", apply_default_ignore_patterns => false, apply_default_patterns => true },
                    //     errors => [{
                    //         message_id => "beside",
                    //         type => Comment,
                    //         line => 1,
                    //         column => 1
                    //     }]
                    // },
                    {
                        code => "1 + 1; // mentioning falls through",
                        errors => [{
                            message_id => "above",
                            type => Comment,
                            line => 1,
                            column => 8
                        }]
                    },
                    {
                        code => "// invalid comment\n1 + 1;",
                        options => "beside",
                        errors => [{
                            message_id => "beside",
                            type => Comment,
                            line => 1,
                            column => 1
                        }]
                    },
                    {
                        code => "// pragma\n// invalid\n1 + 1;",
                        options => { position => "beside", ignore_pattern => "pragma" },
                        errors => [{
                            message_id => "beside",
                            type => Comment,
                            line => 2,
                            column => 1
                        }]
                    },
                    {
                        code => "1 + 1; // linter\n2 + 2; // invalid comment",
                        options => { position => "above", ignore_pattern => "linter" },
                        errors => [{
                            message_id => "above",
                            type => Comment,
                            line => 2,
                            column => 8
                        }]
                    }
                ]
            },
        )
    }
}
