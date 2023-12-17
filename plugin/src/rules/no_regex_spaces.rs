use std::{borrow::Cow, cell::RefCell, sync::Arc};

use regexpp_js::{
    id_arena::Id, visit_reg_exp_ast, visitor, AllArenas, NodeInterface, RegExpParser,
    ValidatePatternFlags, Wtf16,
};
use squalid::{regex, CowStrExt, OptionExt};
use tree_sitter_lint::{
    rule,
    tree_sitter::{Node, Point, Range},
    violation, NodeExt, QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::get_call_expression_arguments,
    kind,
    scope::ScopeManager,
    utils::{ast_utils, ast_utils::get_static_string_value},
};

fn check_regex<'a>(
    node_to_report: Node<'a>,
    pattern_node: Node<'a>,
    pattern: Cow<'a, str>,
    raw_pattern: Cow<'a, str>,
    raw_pattern_start_range: usize,
    flags: Option<Cow<'a, str>>,
    context: &QueryMatchContext<'a, '_>,
) {
    if !regex!(r#" {2}"#).is_match(&raw_pattern) {
        return;
    }

    let arena: AllArenas = Default::default();
    let mut reg_exp_parser = RegExpParser::new(&arena, None);
    let pattern_as_wtf16: Wtf16 = (&*pattern).into();
    let Ok(reg_exp_ast) = reg_exp_parser.parse_pattern(
        &pattern_as_wtf16,
        Some(0),
        Some(pattern_as_wtf16.len()),
        Some(ValidatePatternFlags {
            unicode: Some(flags.as_ref().matches(|flags| flags.contains('u'))),
            unicode_sets: Some(flags.as_ref().matches(|flags| flags.contains('v'))),
        }),
    ) else {
        return;
    };

    #[derive(Default)]
    struct Handlers {
        character_class_nodes: RefCell<Vec<Id<regexpp_js::Node>>>,
    }

    impl visitor::Handlers for Handlers {
        fn on_character_class_enter(&self, node: Id<regexpp_js::Node /* CharacterClass */>) {
            self.character_class_nodes.borrow_mut().push(node);
        }
    }

    let handlers = Handlers::default();

    visit_reg_exp_ast(reg_exp_ast, &handlers, &arena);

    let character_class_nodes = handlers.character_class_nodes.borrow();

    for captures in regex!(r#"( {2,})(?: [+*{?]|[^+*{?]|$)"#).captures_iter(&pattern) {
        let index = captures.get(0).unwrap().start();

        if character_class_nodes.iter().all(|&character_class_node| {
            let character_class_node_ref = arena.node(character_class_node);
            index < character_class_node_ref.start() || character_class_node_ref.end() <= index
        }) {
            let length = captures[1].len();
            context.report(violation! {
                node => node_to_report,
                message_id => "multiple_spaces",
                data => {
                    length => length,
                },
                fix => |fixer| {
                    if pattern != raw_pattern {
                        return;
                    }
                    fixer.replace_text_range(
                        Range {
                            start_byte: raw_pattern_start_range + index,
                            end_byte: raw_pattern_start_range + index + length,
                            // TODO: this assumes that there are no preceding newlines
                            // in the regex pattern I believe which is wrong
                            // Probably should have some helpers for converting from
                            // a byte range to a tree_sitter::Range using the
                            // FileRunContext or something?
                            start_point: Point {
                                row: pattern_node.start_position().row,
                                column: pattern_node.start_position().column + index + 1,
                            },
                            end_point: Point {
                                row: pattern_node.start_position().row,
                                column: pattern_node.start_position().column + index + length + 1,
                            },
                        },
                        format!(" {{{length}}}")
                    );
                }
            });

            return;
        }
    }
}

pub fn no_regex_spaces_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-regex-spaces",
        languages => [Javascript],
        messages => [
            multiple_spaces => "Spaces are hard to count. Use {{{length}}}.",
        ],
        fixable => true,
        listeners => [
            r#"
              (regex) @c
            "# => |node, context| {
                let pattern_node = node.field("pattern");
                let raw_pattern = pattern_node.text(context);
                let pattern = raw_pattern.clone();
                let raw_pattern_start_range = pattern_node.start_byte();
                let flags = node.child_by_field_name("flags").map(|flags| flags.text(context));

                check_regex(
                    node,
                    pattern_node,
                    pattern,
                    raw_pattern,
                    raw_pattern_start_range,
                    flags,
                    context,
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
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let node = captures["call_expression"];
                let scope = scope_manager.get_scope(node);
                let reg_exp_var = ast_utils::get_variable_by_name(scope, "RegExp");
                let shadowed = reg_exp_var.matches(|reg_exp_var| reg_exp_var.defs().next().is_some());
                if shadowed {
                    return;
                }
                let pattern_node = captures["pattern"];

                let raw_pattern = pattern_node.text(context).sliced(|len| 1..len - 1);
                let pattern = get_static_string_value(pattern_node, context).unwrap();
                let raw_pattern_start_range = pattern_node.start_byte() + 1;
                let flags_node = get_call_expression_arguments(node).unwrap().nth(1);
                let flags = match flags_node {
                    Some(flags_node) => {
                        if flags_node.kind() != kind::String {
                            return;
                        }
                        get_static_string_value(flags_node, context)
                    }
                    None => None,
                };

                check_regex(
                    node,
                    pattern_node,
                    pattern,
                    raw_pattern,
                    raw_pattern_start_range,
                    flags,
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
    use crate::{get_instance_provider_factory, kind, kind::{CallExpression, NewExpression}};

    #[test]
    fn test_no_regex_spaces_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_regex_spaces_rule(),
            rule_tests! {
                valid => [
                    "var foo = /foo/;",
                    "var foo = RegExp('foo')",
                    "var foo = / /;",
                    "var foo = RegExp(' ')",
                    "var foo = / a b c d /;",
                    "var foo = /bar {3}baz/g;",
                    "var foo = RegExp('bar {3}baz', 'g')",
                    "var foo = new RegExp('bar {3}baz')",
                    "var foo = /bar\t\t\tbaz/;",
                    "var foo = RegExp('bar\t\t\tbaz');",
                    "var foo = new RegExp('bar\t\t\tbaz');",
                    "var RegExp = function() {}; var foo = new RegExp('bar   baz');",
                    "var RegExp = function() {}; var foo = RegExp('bar   baz');",
                    "var foo = /  +/;",
                    "var foo = /  ?/;",
                    "var foo = /  */;",
                    "var foo = /  {2}/;",

                    // don't report if there are no consecutive spaces in the source code
                    "var foo = /bar \\ baz/;",
                    "var foo = /bar\\ \\ baz/;",
                    "var foo = /bar \\u0020 baz/;",
                    "var foo = /bar\\u0020\\u0020baz/;",
                    "var foo = new RegExp('bar \\ baz')",
                    "var foo = new RegExp('bar\\ \\ baz')",
                    "var foo = new RegExp('bar \\\\ baz')",
                    "var foo = new RegExp('bar \\u0020 baz')",
                    "var foo = new RegExp('bar\\u0020\\u0020baz')",
                    "var foo = new RegExp('bar \\\\u0020 baz')",

                    // don't report spaces in character classes
                    "var foo = /[  ]/;",
                    "var foo = /[   ]/;",
                    "var foo = / [  ] /;",
                    "var foo = / [  ] [  ] /;",
                    "var foo = new RegExp('[  ]');",
                    "var foo = new RegExp('[   ]');",
                    "var foo = new RegExp(' [  ] ');",
                    "var foo = RegExp(' [  ] [  ] ');",
                    "var foo = new RegExp(' \\[   ');",
                    "var foo = new RegExp(' \\[   \\] ');",

                    // ES2024
                    { code => "var foo = /  {2}/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[\\q{    }]/v;", environment => { ecma_version => 2024 } },

                    // don't report invalid regex
                    "var foo = new RegExp('[  ');",
                    "var foo = new RegExp('{  ', 'u');",

                    // don't report if flags cannot be determined
                    "new RegExp('  ', flags)",
                    "new RegExp('[[abc]  ]', flags + 'v')",
                    "new RegExp('[[abc]\\\\q{  }]', flags + 'v')"
                ],
                invalid => [
                    {
                        code => "var foo = /bar  baz/;",
                        output => "var foo = /bar {2}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ],
                    },
                    {
                        code => "var foo = /bar    baz/;",
                        output => "var foo = /bar {4}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = / a b  c d /;",
                        output => "var foo = / a b {2}c d /;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp(' a b c d  ');",
                        output => "var foo = RegExp(' a b c d {2}');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => CallExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('bar    baz');",
                        output => "var foo = RegExp('bar {4}baz');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => CallExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('bar    baz');",
                        output => "var foo = new RegExp('bar {4}baz');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => NewExpression
                            }
                        ]
                    },
                    {

                        // `RegExp` is not shadowed in the scope where it's called
                        code => "{ let RegExp = function() {}; } var foo = RegExp('bar    baz');",
                        output => "{ let RegExp = function() {}; } var foo = RegExp('bar {4}baz');",
                        environment => { ecma_version => 6 },
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => CallExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = /bar   {3}baz/;",
                        output => "var foo = /bar {2} {3}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /bar    ?baz/;",
                        output => "var foo = /bar {3} ?baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "3" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('bar   *baz')",
                        output => "var foo = new RegExp('bar {2} *baz')",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('bar   +baz')",
                        output => "var foo = RegExp('bar {2} +baz')",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => CallExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('bar    ');",
                        output => "var foo = new RegExp('bar {4}');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = /bar\\  baz/;",
                        output => "var foo = /bar\\ {2}baz/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ],
                    },
                    {
                        code => "var foo = /[   ]  /;",
                        output => "var foo = /[   ] {2}/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /  [   ] /;",
                        output => "var foo = / {2}[   ] /;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('[   ]  ');",
                        output => "var foo = new RegExp('[   ] {2}');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('  [ ]');",
                        output => "var foo = RegExp(' {2}[ ]');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => CallExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = /\\[  /;",
                        output => "var foo = /\\[ {2}/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /\\[  \\]/;",
                        output => "var foo = /\\[ {2}\\]/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = /(?:  )/;",
                        output => "var foo = /(?: {2})/;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('^foo(?=   )');",
                        output => "var foo = RegExp('^foo(?= {3})');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "3" },
                                type => CallExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = /\\  /",
                        output => "var foo = /\\ {2}/",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = / \\  /",
                        output => "var foo = / \\ {2}/",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },

                    // report only the first occurrence of consecutive spaces
                    {
                        code => "var foo = /  foo   /;",
                        output => "var foo = / {2}foo   /;",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => kind::Regex
                            }
                        ]
                    },

                    // don't fix strings with escape sequences
                    {
                        code => "var foo = new RegExp('\\\\d  ')",
                        output => None,
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = RegExp('\\u0041   ')",
                        output => None,
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "3" },
                                type => CallExpression
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('\\\\[  \\\\]');",
                        output => None,
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "2" },
                                type => NewExpression
                            }
                        ]
                    },

                    // ES2024
                    {
                        code => "var foo = /[[    ]    ]    /v;",
                        output => "var foo = /[[    ]    ] {4}/v;",
                        environment => {
                            ecma_version => 2024
                        },
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => kind::Regex
                            }
                        ]
                    },
                    {
                        code => "var foo = new RegExp('[[    ]    ]    ', 'v');",
                        output => "var foo = new RegExp('[[    ]    ] {4}', 'v');",
                        errors => [
                            {
                                message_id => "multiple_spaces",
                                data => { length => "4" },
                                type => NewExpression
                            }
                        ]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
