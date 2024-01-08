use std::sync::Arc;

use regexpp_js::{
    id_arena::Id, visit_reg_exp_ast, visitor, AllArenas, RegExpParser, ValidatePatternFlags, Wtf16,
};
use squalid::regex;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

pub fn no_empty_character_class_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-empty-character-class",
        languages => [Javascript],
        messages => [
            unexpected => "Empty class.",
        ],
        listeners => [
            r#"
              (regex) @c
            "# => |node, context| {
                let pattern = node.field("pattern");
                let flags = node.child_by_field_name("flags");

                let pattern_text = pattern.text(context);
                if !regex!(r#"\[\]"#).is_match(&pattern_text) {
                    return;
                }

                let flags_text = flags.map(|flags| flags.text(context)).unwrap_or_default();

                let arena = AllArenas::default();
                let mut parser = RegExpParser::new(&arena, None);
                let pattern_wtf16: Wtf16 = (&*pattern_text).into();
                let Ok(reg_exp_ast) = parser.parse_pattern(
                    &pattern_wtf16,
                    Some(0),
                    Some(pattern_wtf16.len()),
                    Some(ValidatePatternFlags {
                        unicode: Some(flags_text.contains('u')),
                        unicode_sets: Some(flags_text.contains('v')),
                    }),
                ) else {
                    return;
                };

                struct Handlers<'a, 'b, 'c> {
                    arena: &'c AllArenas,
                    context: &'c QueryMatchContext<'a, 'b>,
                    node: Node<'a>,
                }

                impl<'a, 'b, 'c> visitor::Handlers for Handlers<'a, 'b, 'c> {
                    fn on_character_class_enter(&self, character_class: Id<regexpp_js::Node /*CharacterClass*/>) {
                        let character_class_ref = self.arena.node(character_class);
                        let character_class_as_character_class = character_class_ref.as_character_class();
                        if !character_class_as_character_class.negate && character_class_as_character_class.elements.is_empty() {
                            self.context.report(violation! {
                                node => self.node,
                                message_id => "unexpected",
                            });
                        }
                    }
                }

                let handlers = Handlers {
                    arena: &arena,
                    context,
                    node,
                };

                visit_reg_exp_ast(reg_exp_ast, &handlers, &arena);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind;

    #[test]
    fn test_no_empty_character_class_rule() {
        RuleTester::run(
            no_empty_character_class_rule(),
            rule_tests! {
                valid => [
                    "var foo = /^abc[a-zA-Z]/;",
                    "var regExp = new RegExp(\"^abc[]\");",
                    "var foo = /^abc/;",
                    "var foo = /[\\[]/;",
                    "var foo = /[\\]]/;",
                    "var foo = /\\[][\\]]/;",
                    "var foo = /[a-zA-Z\\[]/;",
                    "var foo = /[[]/;",
                    "var foo = /[\\[a-z[]]/;",
                    "var foo = /[\\-\\[\\]\\/\\{\\}\\(\\)\\*\\+\\?\\.\\\\^\\$\\|]/g;",
                    "var foo = /\\s*:\\s*/gim;",
                    "var foo = /[^]/;", // this rule allows negated empty character classes
                    "var foo = /\\[][^]/;",
                    { code => "var foo = /[\\]]/uy;", environment => { ecma_version => 6 } },
                    { code => "var foo = /[\\]]/s;", environment => { ecma_version => 2018 } },
                    { code => "var foo = /[\\]]/d;", environment => { ecma_version => 2022 } },
                    "var foo = /\\[]/",
                    { code => "var foo = /[[^]]/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[[\\]]]/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[[\\[]]/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[a--b]/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[a&&b]/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[[a][b]]/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[\\q{}]/v;", environment => { ecma_version => 2024 } },
                    { code => "var foo = /[[^]--\\p{ASCII}]/v;", environment => { ecma_version => 2024 } }
                ],
                invalid => [
                    { code => "var foo = /^abc[]/;", errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /foo[]bar/;", errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "if (foo.match(/^abc[]/)) {}", errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "if (/^abc[]/.test(foo)) {}", errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[]]/;", errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /\\[[]/;", errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /\\[\\[\\]a-z[]/;", errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[]]/d;", environment => { ecma_version => 2022 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[(]\\u{0}*[]/u;", environment => { ecma_version => 2015 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[[]]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[[a][]]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[a[[b[]c]]d]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[a--[]]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[[]--b]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[a&&[]]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] },
                    { code => "var foo = /[[]&&b]/v;", environment => { ecma_version => 2024 }, errors => [{ message_id => "unexpected", type => kind::Regex }] }
                ]
            },
        )
    }
}
