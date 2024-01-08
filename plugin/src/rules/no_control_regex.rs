use std::{
    borrow::Cow,
    cell::{Ref, RefCell},
    rc::Rc,
    sync::Arc,
};

use itertools::Itertools;
use once_cell::sync::Lazy;
use regexpp_js::{validator, CodePoint, RegExpValidator, ValidatePatternFlags, Wtf16};
use squalid::{CowExt, OptionExt};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{get_call_expression_arguments, get_cooked_value},
    kind,
};

struct RegExp<'a> {
    pattern: Cow<'a, str>,
    flags: Option<Cow<'a, str>>,
}

struct Collector<'a> {
    _source: RefCell<Wtf16>,
    _control_chars: RefCell<Vec<String>>,
    _validator: RefCell<Option<RegExpValidator<'a>>>,
}

impl<'a> Collector<'a> {
    pub fn new() -> Rc<Self> {
        let ret = Rc::new(Self {
            _source: Default::default(),
            _control_chars: Default::default(),
            _validator: Default::default(),
        });

        *ret._validator.borrow_mut() = Some(RegExpValidator::new(Some(ret.clone())));
        ret
    }

    pub fn collect_control_chars(
        &self,
        regexp_str: &str,
        flags: Option<&str>,
    ) -> Ref<'_, Vec<String>> {
        let u_flag = flags.matches(|flags| flags.contains('u'));
        let v_flag = flags.matches(|flags| flags.contains('v'));

        self._control_chars.borrow_mut().clear();
        *self._source.borrow_mut() = regexp_str.into();

        let _ = self
            ._validator
            .borrow_mut()
            .as_mut()
            .unwrap()
            .validate_pattern(
                &self._source.borrow(),
                None,
                None,
                Some(ValidatePatternFlags {
                    unicode: Some(u_flag),
                    unicode_sets: Some(v_flag),
                }),
            );
        self._control_chars.borrow()
    }
}

impl<'a> validator::Options for Collector<'a> {
    fn strict(&self) -> Option<bool> {
        None
    }

    fn ecma_version(&self) -> Option<regexpp_js::EcmaVersion> {
        None
    }

    // TODO: these should really take &mut self?
    // But that doesn't play well with passing an
    // Rc<dyn Options> to RegExpValidator::new()?
    fn on_pattern_enter(&self, _start: usize) {
        self._control_chars.borrow_mut().clear();
    }

    fn on_character(&self, start: usize, end: usize, cp: CodePoint) {
        let _source = self._source.borrow();
        if
        /* cp >= 0x00 && */
        cp <= 0x1f
            && (_source.code_point_at(start).unwrap() == cp
                || _source[start..end].starts_with({
                    static BACKSLASH_X: Lazy<Wtf16> = Lazy::new(|| r#"\x"#.into());
                    &BACKSLASH_X
                })
                || _source[start..end].starts_with({
                    static BACKSLASH_U: Lazy<Wtf16> = Lazy::new(|| r#"\u"#.into());
                    &BACKSLASH_U
                }))
        {
            let cp_as_hex = format!("0{:x}", cp);
            self._control_chars
                .borrow_mut()
                .push(format!(r#"\x{}"#, &cp_as_hex[cp_as_hex.len() - 2..]));
        }
    }
}

pub fn no_control_regex_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-control-regex",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected control character(s) in regular expression: {{control_chars}}.",
        ],
        state => {
            // TODO: make this rule-static? Couldn't do that because regexpp-js
            // is currently single-threaded I guess
            // I think as-is it will be leaking a circular reference (on each file run)
            [per-file-run]
            collector: Rc<Collector<'a>> = Collector::new(),
        },
        methods => {
            fn check(&self, RegExp { pattern, flags }: RegExp, node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
                let control_characters = self.collector.collect_control_chars(&pattern, flags.as_deref());

                if !control_characters.is_empty() {
                    context.report(violation! {
                        node => node,
                        message_id => "unexpected",
                        data => {
                            control_chars => control_characters.join(", "),
                        }
                    });
                }
            }
        },
        listeners => [
            r#"
              (regex) @c
            "# => |node, context| {
                self.check(
                    RegExp {
                        pattern: node.field("pattern").text(context),
                        flags: node.child_by_field_name("flags").map(|flags| flags.text(context)),
                    },
                    node,
                    context
                );
            },
            r#"
              (call_expression
                function: (identifier) @regexp (#eq? @regexp "RegExp")
                arguments: (arguments
                  (string) @pattern
                )
              ) @call_expression
              (new_expression
                constructor: (identifier) @regexp (#eq? @regexp "RegExp")
                arguments: (arguments
                  (string) @pattern
                )
              ) @call_expression
            "# => |captures, context| {
                let pattern = captures["pattern"].text(context).map_cow(get_cooked_value);
                let args = get_call_expression_arguments(
                    captures["call_expression"]
                ).unwrap().collect_vec();
                let flags = args.get(1).copied().filter(|&arg| arg.kind() == kind::String).map(|flags| {
                    flags.text(context).map_cow(get_cooked_value)
                });
                self.check(
                    RegExp {
                        pattern,
                        flags,
                    },
                    captures["pattern"],
                    context,
                );
            }
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind;

    #[test]
    fn test_no_control_regex_rule() {
        RuleTester::run(
            no_control_regex_rule(),
            rule_tests! {
                valid => [
                    "var regex = /x1f/",
                    r#"var regex = /\\x1f/"#,
                    "var regex = new RegExp('x1f')",
                    "var regex = RegExp('x1f')",
                    "new RegExp('[')",
                    "RegExp('[')",
                    "new (function foo(){})('\\x1f')",
                    { code => r#""/\u{20}/u"#, environment => { ecma_version => 2015 } },
                    r#"/\u{1F}/"#,
                    r#"/\u{1F}/g"#,
                    r#"new RegExp("\\u{20}", "u")"#,
                    r#"new RegExp("\\u{1F}")"#,
                    r#"new RegExp("\\u{1F}", "g")"#,
                    r#"new RegExp("\\u{1F}", flags)"#, // when flags are unknown, this rule assumes there's no `u` flag
                    r#"new RegExp("[\\q{\\u{20}}]", "v")"#,
                    { code => r#""/[\u{20}--B]/v"#, environment => { ecma_version => 2024 } }
                ],
                invalid => [
                    { code => r#"var regex = /\x1f/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => r#"var regex = /\\\x1f\\x1e/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => r#"var regex = /\\\x1fFOO\\x00/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => r#"var regex = /FOO\\\x1fFOO\\x1f/"#, errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }] },
                    { code => "var regex = new RegExp('\\x1f\\x1e')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f, \\x1e" }, type => kind::String }] },
                    { code => "var regex = new RegExp('\\x1fFOO\\x00')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f, \\x00" }, type => kind::String }] },
                    { code => "var regex = new RegExp('FOO\\x1fFOO\\x1f')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f, \\x1f" }, type => kind::String }] },
                    { code => "var regex = RegExp('\\x1f')", errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::String }] },
                    {
                        code => "var regex = /(?<a>\\x1f)/",
                        environment => { ecma_version => 2018 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"var regex = /(?<\u{1d49c}>.)\x1f/"#,
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"new RegExp("\\u001F", flags)"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::String }]
                    },
                    {
                        code => r#"/\u{1111}*\x1F/u"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"new RegExp("\\u{1111}*\\x1F", "u")"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::String }]
                    },
                    {
                        code => r#"/\u{1F}/u"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"/\u{1F}/gui"#,
                        environment => { ecma_version => 2015 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"new RegExp("\\u{1F}", "u")"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::String }]
                    },
                    {
                        code => r#"new RegExp("\\u{1F}", "gui")"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::String }]
                    },
                    {
                        code => r#"new RegExp("[\\q{\\u{1F}}]", "v")"#,
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::String }]
                    },
                    {
                        code => r#"/[\u{1F}--B]/v"#,
                        environment => { ecma_version => 2024 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x1f" }, type => kind::Regex }]
                    },
                    {
                        code => r#"/\x11/; RegExp("foo", "uv");"#,
                        environment => { ecma_version => 2024 },
                        errors => [{ message_id => "unexpected", data => { control_chars => "\\x11" }, type => kind::Regex, column => 1 }]
                    }
                ]
            },
        )
    }
}
