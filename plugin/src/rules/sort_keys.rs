use std::{borrow::Cow, cmp::Ordering, sync::Arc};

use serde::Deserialize;
use squalid::{continue_if_none, regex, EverythingExt};
use tree_sitter_lint::{
    rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule, SkipOptionsBuilder,
};

use crate::{
    ast_helpers::{get_object_property_computed_property_name, get_object_property_key},
    kind::{Identifier, SpreadElement},
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
enum Direction {
    #[default]
    #[serde(rename = "asc")]
    Ascending,
    #[serde(rename = "desc")]
    Descending,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OptionsVariants {
    EmptyList(),
    JustDirection([Direction; 1]),
    DirectionAndOptionsObject(Direction, OptionsObject),
}

impl Default for OptionsVariants {
    fn default() -> Self {
        Self::EmptyList()
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct OptionsObject {
    case_sensitive: bool,
    natural: bool,
    min_keys: usize,
    allow_line_separated_groups: bool,
}

impl Default for OptionsObject {
    fn default() -> Self {
        Self {
            case_sensitive: true,
            natural: false,
            min_keys: 2,
            allow_line_separated_groups: false,
        }
    }
}

struct Options {
    direction: Direction,
    case_sensitive: bool,
    natural: bool,
    min_keys: usize,
    allow_line_separated_groups: bool,
}

impl Options {
    pub fn from_direction_and_options_object(
        direction: Direction,
        options_object: OptionsObject,
    ) -> Self {
        Self {
            direction,
            case_sensitive: options_object.case_sensitive,
            natural: options_object.natural,
            min_keys: options_object.min_keys,
            allow_line_separated_groups: options_object.allow_line_separated_groups,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        OptionsVariants::default().into()
    }
}

impl From<OptionsVariants> for Options {
    fn from(value: OptionsVariants) -> Self {
        match value {
            OptionsVariants::EmptyList() => {
                Self::from_direction_and_options_object(Default::default(), Default::default())
            }
            OptionsVariants::JustDirection(direction) => {
                Self::from_direction_and_options_object(direction[0], Default::default())
            }
            OptionsVariants::DirectionAndOptionsObject(direction, options_object) => {
                Self::from_direction_and_options_object(direction, options_object)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Options {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(OptionsVariants::deserialize(deserializer)?.into())
    }
}

fn get_property_name<'a>(node: Node, context: &QueryMatchContext<'a, '_>) -> Option<Cow<'a, str>> {
    ast_utils::get_static_property_name(node, context).or_else(|| {
        get_object_property_computed_property_name(node)?
            .first_non_comment_named_child(context)
            .when(|child| child.kind() == Identifier)
            .map(|child| child.text(context))
    })
}

fn natural_compare(a: &str, b: &str) -> Ordering {
    let starts_with_letter = regex!(r#"^[a-zA-Z]"#);
    if a.starts_with('_') && starts_with_letter.is_match(b) {
        return Ordering::Less;
    }
    if b.starts_with('_') && starts_with_letter.is_match(a) {
        return Ordering::Greater;
    }
    natord::compare(a, b)
}

fn is_valid_order(order: Direction, insensitive: bool, natural: bool, a: &str, b: &str) -> bool {
    match (order, insensitive, natural) {
        (Direction::Ascending, false, false) => a <= b,
        (Direction::Ascending, true, false) => a.to_lowercase() <= b.to_lowercase(),
        (Direction::Ascending, false, true) => natural_compare(a, b) != Ordering::Greater,
        (Direction::Ascending, true, true) => {
            natord::compare(&a.to_lowercase(), &b.to_lowercase()) != Ordering::Greater
        }
        (Direction::Descending, insensitive, natural) => {
            is_valid_order(Direction::Ascending, insensitive, natural, b, a)
        }
    }
}

pub fn sort_keys_rule() -> Arc<dyn Rule> {
    rule! {
        name => "sort-keys",
        languages => [Javascript],
        messages => [
            sort_keys => "Expected object keys to be in {{natural}}{{insensitive}}{{order}}ending order. '{{this_name}}' should be before '{{prev_name}}'.",
        ],
        options_type => Options,
        state => {
            [per-config]
            order: Direction = options.direction,
            insensitive: bool = !options.case_sensitive,
            natural: bool = options.natural,
            min_keys: usize = options.min_keys,
            allow_line_separated_groups: bool = options.allow_line_separated_groups,
        },
        listeners => [
            r#"
              (object) @c
            "# => |node, context| {
                let properties = node.non_comment_named_children(context).collect::<Vec<_>>();
                if properties.len() < self.min_keys {
                    return;
                }
                let mut prev_node: Option<Node<'a>> = None;
                let mut prev_blank_line = false;
                let mut prev_name: Option<Cow<'a, str>> = None;

                for &property in &properties {
                    match property.kind() {
                        SpreadElement => prev_name = None,
                        _ => {
                            let this_name = get_property_name(property, context);

                            let tokens = prev_node.map(|prev_node| {
                                context.get_tokens_between(
                                    prev_node,
                                    property,
                                    Some(SkipOptionsBuilder::<fn(Node) -> bool>::default()
                                        .include_comments(true)
                                        .build().unwrap())
                                )
                            });

                            let mut is_blank_line_between_nodes = prev_blank_line;

                            if let Some(tokens) = tokens {
                                let tokens = tokens.collect::<Vec<_>>();
                                tokens.iter().enumerate().skip(1).for_each(|(index, token)| {
                                    let previous_token = tokens[index - 1];

                                    if token.start_position().row - previous_token.end_position().row > 1 {
                                        is_blank_line_between_nodes = true;
                                    }
                                });

                                if !is_blank_line_between_nodes &&
                                    property.start_position().row - tokens.last().unwrap().end_position().row > 1 {
                                    is_blank_line_between_nodes = true;
                                }

                                if !is_blank_line_between_nodes &&
                                    tokens.first().unwrap().start_position().row - prev_node.unwrap().end_position().row > 1 {
                                    is_blank_line_between_nodes = true;
                                }
                            }

                            prev_node = Some(property);

                            let prev_name_local = prev_name.clone();

                            if this_name.is_some() {
                                prev_name = this_name.clone();
                            }

                            if self.allow_line_separated_groups && is_blank_line_between_nodes {
                                prev_blank_line = this_name.is_none();
                                continue;
                            }

                            let prev_name = continue_if_none!(prev_name_local);
                            let this_name = continue_if_none!(this_name);

                            if !is_valid_order(
                                self.order,
                                self.insensitive,
                                self.natural,
                                &prev_name,
                                &this_name,
                            ) {
                                context.report(violation! {
                                    node => property,
                                    range => get_object_property_key(property).range(),
                                    message_id => "sort_keys",
                                    data => {
                                        this_name => this_name,
                                        prev_name => prev_name,
                                        order => match self.order {
                                            Direction::Ascending => "asc",
                                            Direction::Descending => "desc",
                                        },
                                        insensitive => self.insensitive.then_some("insensitive ").unwrap_or(""),
                                        natural => self.natural.then_some("natural ").unwrap_or(""),
                                    }
                                });
                            }
                        }
                    }
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_sort_keys_rule() {
        RuleTester::run(
            sort_keys_rule(),
            rule_tests! {
            valid => [
                // default (asc)
                { code => "var obj = {'':1, [``]:2}", options => [], /*parserOptions: { ecmaVersion: 6 }*/ },
                { code => "var obj = {[``]:1, '':2}", options => [], /*parserOptions: { ecmaVersion: 6 }*/ },
                { code => "var obj = {'':1, a:2}", options => [] },
                { code => "var obj = {[``]:1, a:2}", options => [], /*parserOptions: { ecmaVersion: 6 }*/ },
                { code => "var obj = {_:2, a:1, b:3} // default", options => [] },
                { code => "var obj = {a:1, b:3, c:2}", options => [] },
                { code => "var obj = {a:2, b:3, b_:1}", options => [] },
                { code => "var obj = {C:3, b_:1, c:2}", options => [] },
                { code => "var obj = {$:1, A:3, _:2, a:4}", options => [] },
                { code => "var obj = {1:1, '11':2, 2:4, A:3}", options => [] },
                { code => "var obj = {'#':1, 'Z':2, À:3, è:4}", options => [] },
                { code => "var obj = { [/(?<zero>0)/]: 1, '/(?<zero>0)/': 2 }", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },

                // ignore non-simple computed properties.
                { code => "var obj = {a:1, b:3, [a + b]: -1, c:2}", options => [], /*parserOptions: { ecmaVersion: 6 }*/ },
                { code => "var obj = {'':1, [f()]:2, a:3}", options => [], /*parserOptions: { ecmaVersion: 6 }*/ },
                { code => "var obj = {a:1, [b++]:2, '':3}", options => ["desc"], /*parserOptions: { ecmaVersion: 6 }*/ },

                // ignore properties separated by spread properties
                { code => "var obj = {a:1, ...z, b:1}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {b:1, ...z, a:1}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {...a, b:1, ...c, d:1}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {...a, b:1, ...d, ...c, e:2, z:5}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {b:1, ...c, ...d, e:2}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {a:1, ...z, '':2}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {'':1, ...z, 'a':2}", options => ["desc"], /*parserOptions: { ecmaVersion: 2018 }*/ },

                // not ignore properties not separated by spread properties
                { code => "var obj = {...z, a:1, b:1}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {...z, ...c, a:1, b:1}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {a:1, b:1, ...z}", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "var obj = {...z, ...x, a:1, ...c, ...d, f:5, e:4}", options => ["desc"], /*parserOptions: { ecmaVersion: 2018 }*/ },

                // works when spread occurs somewhere other than an object literal
                { code => "function fn(...args) { return [...args].length; }", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },
                { code => "function g() {}; function f(...args) { return g(...args); }", options => [], /*parserOptions: { ecmaVersion: 2018 }*/ },

                // ignore destructuring patterns.
                { code => "let {a, b} = {}", options => [], /*parserOptions: { ecmaVersion: 6 }*/ },

                // nested
                { code => "var obj = {a:1, b:{x:1, y:1}, c:1}", options => [] },

                // asc
                { code => "var obj = {_:2, a:1, b:3} // asc", options => ["asc"] },
                { code => "var obj = {a:1, b:3, c:2}", options => ["asc"] },
                { code => "var obj = {a:2, b:3, b_:1}", options => ["asc"] },
                { code => "var obj = {C:3, b_:1, c:2}", options => ["asc"] },
                { code => "var obj = {$:1, A:3, _:2, a:4}", options => ["asc"] },
                { code => "var obj = {1:1, '11':2, 2:4, A:3}", options => ["asc"] },
                { code => "var obj = {'#':1, 'Z':2, À:3, è:4}", options => ["asc"] },

                // asc, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {a:1, c:2, b:3}", options => ["asc", { min_keys => 4 }] },

                // asc, insensitive
                { code => "var obj = {_:2, a:1, b:3} // asc, insensitive", options => ["asc", { case_sensitive => false }] },
                { code => "var obj = {a:1, b:3, c:2}", options => ["asc", { case_sensitive => false }] },
                { code => "var obj = {a:2, b:3, b_:1}", options => ["asc", { case_sensitive => false }] },
                { code => "var obj = {b_:1, C:3, c:2}", options => ["asc", { case_sensitive => false }] },
                { code => "var obj = {b_:1, c:3, C:2}", options => ["asc", { case_sensitive => false }] },
                { code => "var obj = {$:1, _:2, A:3, a:4}", options => ["asc", { case_sensitive => false }] },
                { code => "var obj = {1:1, '11':2, 2:4, A:3}", options => ["asc", { case_sensitive => false }] },
                { code => "var obj = {'#':1, 'Z':2, À:3, è:4}", options => ["asc", { case_sensitive => false }] },

                // asc, insensitive, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {$:1, A:3, _:2, a:4}", options => ["asc", { case_sensitive => false, min_keys => 5 }] },

                // asc, natural
                { code => "var obj = {_:2, a:1, b:3} // asc, natural", options => ["asc", { natural => true }] },
                { code => "var obj = {a:1, b:3, c:2}", options => ["asc", { natural => true }] },
                { code => "var obj = {a:2, b:3, b_:1}", options => ["asc", { natural => true }] },
                { code => "var obj = {C:3, b_:1, c:2}", options => ["asc", { natural => true }] },
                { code => "var obj = {$:1, _:2, A:3, a:4}", options => ["asc", { natural => true }] },
                { code => "var obj = {1:1, 2:4, '11':2, A:3}", options => ["asc", { natural => true }] },
                { code => "var obj = {'#':1, 'Z':2, À:3, è:4}", options => ["asc", { natural => true }] },

                // asc, natural, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {b_:1, a:2, b:3}", options => ["asc", { natural => true, min_keys => 4 }] },

                // asc, natural, insensitive
                { code => "var obj = {_:2, a:1, b:3} // asc, natural, insensitive", options => ["asc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {a:1, b:3, c:2}", options => ["asc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {a:2, b:3, b_:1}", options => ["asc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {b_:1, C:3, c:2}", options => ["asc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {b_:1, c:3, C:2}", options => ["asc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {$:1, _:2, A:3, a:4}", options => ["asc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {1:1, 2:4, '11':2, A:3}", options => ["asc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {'#':1, 'Z':2, À:3, è:4}", options => ["asc", { natural => true, case_sensitive => false }] },

                // asc, natural, insensitive, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {a:1, _:2, b:3}", options => ["asc", { natural => true, case_sensitive => false, min_keys => 4 }] },

                // desc
                { code => "var obj = {b:3, a:1, _:2} // desc", options => ["desc"] },
                { code => "var obj = {c:2, b:3, a:1}", options => ["desc"] },
                { code => "var obj = {b_:1, b:3, a:2}", options => ["desc"] },
                { code => "var obj = {c:2, b_:1, C:3}", options => ["desc"] },
                { code => "var obj = {a:4, _:2, A:3, $:1}", options => ["desc"] },
                { code => "var obj = {A:3, 2:4, '11':2, 1:1}", options => ["desc"] },
                { code => "var obj = {è:4, À:3, 'Z':2, '#':1}", options => ["desc"] },

                // desc, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {a:1, c:2, b:3}", options => ["desc", { min_keys => 4 }] },

                // desc, insensitive
                { code => "var obj = {b:3, a:1, _:2} // desc, insensitive", options => ["desc", { case_sensitive => false }] },
                { code => "var obj = {c:2, b:3, a:1}", options => ["desc", { case_sensitive => false }] },
                { code => "var obj = {b_:1, b:3, a:2}", options => ["desc", { case_sensitive => false }] },
                { code => "var obj = {c:2, C:3, b_:1}", options => ["desc", { case_sensitive => false }] },
                { code => "var obj = {C:2, c:3, b_:1}", options => ["desc", { case_sensitive => false }] },
                { code => "var obj = {a:4, A:3, _:2, $:1}", options => ["desc", { case_sensitive => false }] },
                { code => "var obj = {A:3, 2:4, '11':2, 1:1}", options => ["desc", { case_sensitive => false }] },
                { code => "var obj = {è:4, À:3, 'Z':2, '#':1}", options => ["desc", { case_sensitive => false }] },

                // desc, insensitive, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {$:1, _:2, A:3, a:4}", options => ["desc", { case_sensitive => false, min_keys => 5 }] },

                // desc, natural
                { code => "var obj = {b:3, a:1, _:2} // desc, natural", options => ["desc", { natural => true }] },
                { code => "var obj = {c:2, b:3, a:1}", options => ["desc", { natural => true }] },
                { code => "var obj = {b_:1, b:3, a:2}", options => ["desc", { natural => true }] },
                { code => "var obj = {c:2, b_:1, C:3}", options => ["desc", { natural => true }] },
                { code => "var obj = {a:4, A:3, _:2, $:1}", options => ["desc", { natural => true }] },
                { code => "var obj = {A:3, '11':2, 2:4, 1:1}", options => ["desc", { natural => true }] },
                { code => "var obj = {è:4, À:3, 'Z':2, '#':1}", options => ["desc", { natural => true }] },

                // desc, natural, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {b_:1, a:2, b:3}", options => ["desc", { natural => true, min_keys => 4 }] },

                // desc, natural, insensitive
                { code => "var obj = {b:3, a:1, _:2} // desc, natural, insensitive", options => ["desc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {c:2, b:3, a:1}", options => ["desc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {b_:1, b:3, a:2}", options => ["desc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {c:2, C:3, b_:1}", options => ["desc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {C:2, c:3, b_:1}", options => ["desc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {a:4, A:3, _:2, $:1}", options => ["desc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {A:3, '11':2, 2:4, 1:1}", options => ["desc", { natural => true, case_sensitive => false }] },
                { code => "var obj = {è:4, À:3, 'Z':2, '#':1}", options => ["desc", { natural => true, case_sensitive => false }] },

                // desc, natural, insensitive, minKeys should ignore unsorted keys when number of keys is less than minKeys
                { code => "var obj = {a:1, _:2, b:3}", options => ["desc", { natural => true, case_sensitive => false, min_keys => 4 }] },

                // allowLineSeparatedGroups option
                {
                    code => r#"
                        var obj = {
                            e: 1,
                            f: 2,
                            g: 3,

                            a: 4,
                            b: 5,
                            c: 6
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }]
                },
                {
                    code => r#"
                        var obj = {
                            b: 1,

                            // comment
                            a: 2,
                            c: 3
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }]
                },
                {
                    code => r#"
                        var obj = {
                            b: 1
                            
                            ,

                            // comment
                            a: 2,
                            c: 3
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }]
                },
                {
                    code => r#"
                        var obj = {
                            c: 1,
                            d: 2,

                            b() {
                            },
                            e: 4
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                            c: 1,
                            d: 2,
                            // comment

                            // comment
                            b() {
                            },
                            e: 4
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                          b,

                          [a+b]: 1,
                          a
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                            c: 1,
                            d: 2,

                            a() {

                            },

                            // abce
                            f: 3,

                            /*

                            */
                            [a+b]: 1,
                            cc: 1,
                            e: 2
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                            b: "/*",

                            a: "*/",
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }]
                },
                {
                    code => r#"
                        var obj = {
                            b,
                            /*
                            */ //

                            a
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                            b,

                            /*
                            */ //
                            a
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                            b: 1

                            ,a: 2
                        };
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                            b: 1
                        // comment before comma

                        ,
                        a: 2
                        };
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => r#"
                        var obj = {
                          b,

                          a,
                          ...z,
                          c
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 2018 }
                },
                {
                    code => r#"
                        var obj = {
                          b,

                          [foo()]: [

                          ],
                          a
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 2018 }
                }
            ],
            invalid => [
                // default (asc)
                {
                    code => "var obj = {a:1, '':2} // default",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, [``]:2} // default",
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, _:2, b:3} // default",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, c:2, C:3}",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "C",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, _:2, A:3, a:4}",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "A",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, A:3, '11':2}",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "11",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "Z",
                                prev_name => "À"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = { null: 1, [/(?<zero>0)/]: 2 }",
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "/(?<zero>0)/",
                                prev_name => "null"
                            }
                        }
                    ]
                },

                // not ignore properties not separated by spread properties
                {
                    code => "var obj = {...z, c:1, b:1}",
                    options => [],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {...z, ...c, d:4, b:1, ...y, ...f, e:2, a:1}",
                    options => [],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "d"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "e"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {c:1, b:1, ...a}",
                    options => [],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {...z, ...a, c:1, b:1}",
                    options => [],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {...z, b:1, a:1, ...d, ...c}",
                    options => [],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {...z, a:2, b:0, ...x, ...c}",
                    options => ["desc"],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {...z, a:2, b:0, ...x}",
                    options => ["desc"],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {...z, '':1, a:2}",
                    options => ["desc"],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "a",
                                prev_name => ""
                            }
                        }
                    ]
                },

                // ignore non-simple computed properties, but their position shouldn't affect other comparisons.
                {
                    code => "var obj = {a:1, [b+c]:2, '':3}",
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'':1, [b+c]:2, a:3}",
                    options => ["desc"],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "a",
                                prev_name => ""
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b:1, [f()]:2, '':3, a:4}",
                    options => ["desc"],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "a",
                                prev_name => ""
                            }
                        }
                    ]
                },

                // not ignore simple computed properties.
                {
                    code => "var obj = {a:1, b:3, [a]: -1, c:2}",
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b"
                            }
                        }
                    ]
                },

                // nested
                {
                    code => "var obj = {a:1, c:{y:1, x:1}, b:1}",
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "x",
                                prev_name => "y"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },

                // asc
                {
                    code => "var obj = {a:1, _:2, b:3} // asc",
                    options => ["asc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["asc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["asc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, c:2, C:3}",
                    options => ["asc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "C",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, _:2, A:3, a:4}",
                    options => ["asc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "A",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, A:3, '11':2}",
                    options => ["asc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "11",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["asc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "Z",
                                prev_name => "À"
                            }
                        }
                    ]
                },

                // asc, minKeys should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["asc", { min_keys => 3 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },

                // asc, insensitive
                {
                    code => "var obj = {a:1, _:2, b:3} // asc, insensitive",
                    options => ["asc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["asc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["asc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, A:3, _:2, a:4}",
                    options => ["asc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "_",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, A:3, '11':2}",
                    options => ["asc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "11",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["asc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "Z",
                                prev_name => "À"
                            }
                        }
                    ]
                },

                // asc, insensitive, minKeys should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["asc", { case_sensitive => false, min_keys => 3 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },

                // asc, natural
                {
                    code => "var obj = {a:1, _:2, b:3} // asc, natural",
                    options => ["asc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["asc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["asc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, c:2, C:3}",
                    options => ["asc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "C",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, A:3, _:2, a:4}",
                    options => ["asc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "_",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, A:3, '11':2}",
                    options => ["asc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "11",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["asc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "Z",
                                prev_name => "À"
                            }
                        }
                    ]
                },

                // asc, natural, minKeys should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["asc", { natural => true, min_keys => 2 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },

                // asc, natural, insensitive
                {
                    code => "var obj = {a:1, _:2, b:3} // asc, natural, insensitive",
                    options => ["asc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["asc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "b",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["asc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, A:3, _:2, a:4}",
                    options => ["asc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "_",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, '11':2, 2:4, A:3}",
                    options => ["asc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "2",
                                prev_name => "11"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["asc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "Z",
                                prev_name => "À"
                            }
                        }
                    ]
                },

                // asc, natural, insensitive, minKeys should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["asc", { natural => true, case_sensitive => false, min_keys => 3 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "asc",
                                this_name => "_",
                                prev_name => "a"
                            }
                        }
                    ]
                },

                // desc
                {
                    code => "var obj = {'':1, a:'2'} // desc",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "a",
                                prev_name => ""
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {[``]:1, a:'2'} // desc",
                    options => ["desc"],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "a",
                                prev_name => ""
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, _:2, b:3} // desc",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "c",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, c:2, C:3}",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "c",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, _:2, A:3, a:4}",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "_",
                                prev_name => "$"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "a",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, A:3, '11':2}",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "2",
                                prev_name => "1"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "A",
                                prev_name => "2"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["desc"],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "À",
                                prev_name => "#"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "è",
                                prev_name => "Z"
                            }
                        }
                    ]
                },

                // desc, minKeys should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["desc", { min_keys => 3 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },

                // desc, insensitive
                {
                    code => "var obj = {a:1, _:2, b:3} // desc, insensitive",
                    options => ["desc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["desc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "c",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["desc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "b",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, c:2, C:3}",
                    options => ["desc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "c",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, _:2, A:3, a:4}",
                    options => ["desc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "_",
                                prev_name => "$"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "A",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, A:3, '11':2}",
                    options => ["desc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "2",
                                prev_name => "1"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "A",
                                prev_name => "2"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["desc", { case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "À",
                                prev_name => "#"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "è",
                                prev_name => "Z"
                            }
                        }
                    ]
                },

                // desc, insensitive should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["desc", { case_sensitive => false, min_keys => 2 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },

                // desc, natural
                {
                    code => "var obj = {a:1, _:2, b:3} // desc, natural",
                    options => ["desc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["desc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "c",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["desc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, c:2, C:3}",
                    options => ["desc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "c",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, _:2, A:3, a:4}",
                    options => ["desc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "_",
                                prev_name => "$"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "A",
                                prev_name => "_"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "a",
                                prev_name => "A"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, A:3, '11':2}",
                    options => ["desc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "2",
                                prev_name => "1"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "A",
                                prev_name => "2"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["desc", { natural => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "À",
                                prev_name => "#"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "è",
                                prev_name => "Z"
                            }
                        }
                    ]
                },

                // desc, natural should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["desc", { natural => true, min_keys => 3 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },

                // desc, natural, insensitive
                {
                    code => "var obj = {a:1, _:2, b:3} // desc, natural, insensitive",
                    options => ["desc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {a:1, c:2, b:3}",
                    options => ["desc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "c",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, a:2, b:3}",
                    options => ["desc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "b",
                                prev_name => "a"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {b_:1, c:2, C:3}",
                    options => ["desc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "c",
                                prev_name => "b_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {$:1, _:2, A:3, a:4}",
                    options => ["desc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "_",
                                prev_name => "$"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "A",
                                prev_name => "_"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {1:1, 2:4, '11':2, A:3}",
                    options => ["desc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "2",
                                prev_name => "1"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "11",
                                prev_name => "2"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "A",
                                prev_name => "11"
                            }
                        }
                    ]
                },
                {
                    code => "var obj = {'#':1, À:3, 'Z':2, è:4}",
                    options => ["desc", { natural => true, case_sensitive => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "À",
                                prev_name => "#"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "è",
                                prev_name => "Z"
                            }
                        }
                    ]
                },

                // desc, natural, insensitive should error when number of keys is greater than or equal to minKeys
                {
                    code => "var obj = {a:1, _:2, b:3}",
                    options => ["desc", { natural => true, case_sensitive => false, min_keys => 2 }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "natural ",
                                insensitive => "insensitive ",
                                order => "desc",
                                this_name => "b",
                                prev_name => "_"
                            }
                        }
                    ]
                },

                // When allowLineSeparatedGroups option is false
                {
                    code => r#"
                        var obj = {
                            b: 1,
                            c: 2,
                            a: 3
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => false }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                        let obj = {
                            b

                            ,a
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => false }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b"
                            }
                        }
                    ]
                },

                // When allowLineSeparatedGroups option is true
                {
                    code => r#"
                         var obj = {
                            b: 1,
                            c () {

                            },
                            a: 3
                          }
                     "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                         var obj = {
                            a: 1,
                            b: 2,

                            z () {

                            },
                            y: 3
                          }
                     "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "y",
                                prev_name => "z"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                         var obj = {
                            b: 1,
                            c () {
                            },
                            // comment
                            a: 3
                          }
                     "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "c"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                        var obj = {
                          b,
                          [a+b]: 1,
                          a // sort-keys: 'a' should be before 'b'
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                        var obj = {
                            c: 1,
                            d: 2,
                            // comment
                            // comment
                            b() {
                            },
                            e: 4
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "d"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                        var obj = {
                            c: 1,
                            d: 2,

                            z() {

                            },
                            f: 3,
                            /*


                            */
                            [a+b]: 1,
                            b: 1,
                            e: 2
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "f",
                                prev_name => "z"
                            }
                        },
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "b",
                                prev_name => "f"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                        var obj = {
                            b: "/*",
                            a: "*/",
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                        var obj = {
                            b: 1
                            // comment before comma
                            , a: 2
                        };
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b"
                            }
                        }
                    ]
                },
                {
                    code => r#"
                        let obj = {
                          b,
                          [foo()]: [
                          // ↓ this blank is inside a property and therefore should not count

                          ],
                          a
                        }
                    "#,
                    options => ["asc", { allow_line_separated_groups => true }],
                    // parserOptions: { ecmaVersion: 2018 },
                    errors => [
                        {
                            message_id => "sort_keys",
                            data => {
                                natural => "",
                                insensitive => "",
                                order => "asc",
                                this_name => "a",
                                prev_name => "b"
                            }
                        }
                    ]
                }
            ]
            },
        )
    }
}
