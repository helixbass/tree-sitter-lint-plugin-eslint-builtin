use std::sync::Arc;

use regex::Regex;
use serde::Deserialize;
use tree_sitter_lint::{rule, violation, Rule};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
enum Vars {
    #[default]
    All,
    Local,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Args {
    All,
    #[default]
    AfterUsed,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
enum CaughtErrors {
    All,
    #[default]
    None,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct OptionsObject {
    vars: Vars,
    #[serde(with = "serde_regex")]
    vars_ignore_pattern: Option<Regex>,
    args: Args,
    ignore_rest_siblings: bool,
    #[serde(with = "serde_regex")]
    args_ignore_pattern: Option<Regex>,
    caught_errors: CaughtErrors,
    #[serde(with = "serde_regex")]
    caught_errors_ignore_pattern: Option<Regex>,
    #[serde(with = "serde_regex")]
    destructured_array_ignore_pattern: Option<Regex>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Vars(Vars),
    Object(OptionsObject),
}

impl Options {
    pub fn vars(&self) -> Vars {
        match self {
            Self::Vars(value) => *value,
            Self::Object(OptionsObject { vars, .. }) => *vars,
        }
    }

    pub fn vars_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                vars_ignore_pattern,
                ..
            }) => vars_ignore_pattern.clone(),
            _ => None,
        }
    }

    pub fn args(&self) -> Args {
        match self {
            Self::Object(OptionsObject { args, .. }) => *args,
            _ => Default::default(),
        }
    }

    pub fn ignore_rest_siblings(&self) -> bool {
        match self {
            Self::Object(OptionsObject {
                ignore_rest_siblings,
                ..
            }) => *ignore_rest_siblings,
            _ => Default::default(),
        }
    }

    pub fn args_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                args_ignore_pattern,
                ..
            }) => args_ignore_pattern.clone(),
            _ => None,
        }
    }

    pub fn caught_errors(&self) -> CaughtErrors {
        match self {
            Self::Object(OptionsObject { caught_errors, .. }) => *caught_errors,
            _ => Default::default(),
        }
    }

    pub fn caught_errors_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                caught_errors_ignore_pattern,
                ..
            }) => caught_errors_ignore_pattern.clone(),
            _ => None,
        }
    }

    pub fn destructured_array_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                destructured_array_ignore_pattern,
                ..
            }) => destructured_array_ignore_pattern.clone(),
            _ => None,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Object(Default::default())
    }
}

pub fn no_unused_vars_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-unused-vars",
        languages => [Javascript],
        messages => [
            unused_var => "'{{var_name}}' is {{action}} but never used{{additional}}.",
        ],
        options_type => Options,
        state => {
            [per-run]
            vars: Vars = options.vars(),
            vars_ignore_pattern: Option<Regex> = options.vars_ignore_pattern(),
            args: Args = options.args(),
            ignore_rest_siblings: bool = options.ignore_rest_siblings(),
            args_ignore_pattern: Option<Regex> = options.args_ignore_pattern(),
            caught_errors: CaughtErrors = options.caught_errors(),
            caught_errors_ignore_pattern: Option<Regex> = options.caught_errors_ignore_pattern(),
            destructured_array_ignore_pattern: Option<Regex> = options.destructured_array_ignore_pattern(),
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
    fn test_no_unused_vars_rule() {
        RuleTester::run(
            no_unused_vars_rule(),
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
