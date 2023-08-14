use std::{collections::HashSet, sync::Arc};

use id_arena::Id;
use squalid::{EverythingExt, OptionExt};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{get_method_definition_kind, MethodDefinitionKind, NodeExtJs},
    kind::{CallExpression, ClassHeritage, MethodDefinition, ReturnStatement, Super},
    utils::ast_utils,
    CodePathAnalyzer, CodePathSegment, EnterOrExit,
};

fn is_constructor_function(node: Node, context: &QueryMatchContext) -> bool {
    node.kind() == MethodDefinition
        && get_method_definition_kind(node, context) == MethodDefinitionKind::Constructor
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Found {
    No,
    InSome,
    InAll,
}

impl<TIterator> From<TIterator> for Found
where
    TIterator: Iterator<Item = Found>,
{
    fn from(value: TIterator) -> Self {
        let mut has_seen_yes = false;
        let mut has_seen_no = false;
        for item in value {
            match item {
                Found::No => has_seen_no = true,
                Found::InSome => {
                    has_seen_yes = true;
                    has_seen_no = true;
                }
                Found::InAll => has_seen_yes = true,
            }
            if has_seen_yes && has_seen_no {
                return Found::InSome;
            }
        }
        match (has_seen_yes, has_seen_no) {
            (_, false) => Found::InAll,
            (false, true) => Found::No,
            _ => unreachable!(),
        }
    }
}

fn check_for_no_super<'a>(
    segment: Id<CodePathSegment<'a>>,
    code_path_analyzer: &CodePathAnalyzer<'a>,
    seen_segments: &mut HashSet<Id<CodePathSegment<'a>>>,
) -> Found {
    seen_segments.insert(segment);
    println!(
        "check_for_no_super() 1, segment: {:#?}",
        &code_path_analyzer.code_path_segment_arena[segment]
    );
    if code_path_analyzer.code_path_segment_arena[segment]
        .nodes
        .iter()
        .filter(|(enter_or_exit, _)| *enter_or_exit == EnterOrExit::Enter)
        .any(|(_, node)| {
            node.kind() == ReturnStatement && node.has_non_comment_named_children()
                || node.kind() == CallExpression && node.field("function").kind() == Super
        })
    {
        println!("check_for_no_super() 2");
        return Found::InAll;
    }

    (&code_path_analyzer.code_path_segment_arena[segment].prev_segments).thrush(|prev_segments| {
        if prev_segments.is_empty() {
            println!("check_for_no_super() 3");
            Found::No
        } else {
            println!("check_for_no_super() 4");
            prev_segments
                .into_iter()
                .filter_map(|&prev_segment| {
                    if seen_segments.contains(&prev_segment) {
                        None
                    } else {
                        Some(check_for_no_super(
                            prev_segment,
                            code_path_analyzer,
                            seen_segments,
                        ))
                    }
                })
                .into()
        }
    })
}

pub fn constructor_super_rule() -> Arc<dyn Rule> {
    rule! {
        name => "constructor-super",
        languages => [Javascript],
        messages => [
            missing_some => "Lacked a call of 'super()' in some code paths.",
            missing_all => "Expected to call 'super()'.",
            duplicate => "Unexpected duplicate 'super()'.",
            bad_super => "Unexpected 'super()' because 'super' is not a constructor.",
            unexpected => "Unexpected 'super()'.",
        ],
        listeners => [
            r#"
              (call_expression
                function: (super)
              ) @c
            "# => |node, context| {
                if !ast_utils::get_upper_function(node).matches(|node| is_constructor_function(node, context)) {
                    context.report(violation! {
                        node => node,
                        message_id => "unexpected",
                    });
                }
            },
            "program:exit" => |node, context| {
                println!("program exit");
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                for &code_path in code_path_analyzer
                    .code_paths
                    .iter()
                    .filter(|&&code_path| {
                        code_path_analyzer.code_path_arena[code_path]
                            .root_node(&code_path_analyzer.code_path_segment_arena)
                            .thrush(|root_node| is_constructor_function(root_node, context))
                    })
                {
                    println!("code path");
                    let root_node = code_path_analyzer.code_path_arena[code_path]
                        .root_node(&code_path_analyzer.code_path_segment_arena);

                    let class_node = root_node.parent().unwrap().parent().unwrap();
                    let has_extends = class_node.has_child_of_kind(ClassHeritage);

                    if has_extends {
                        println!("has extends");

                        let mut seen_segments: HashSet<Id<CodePathSegment<'a>>> = Default::default();

                        let no_supers: Found = code_path_analyzer
                            .code_path_arena[code_path]
                            .returned_segments()
                            .into_iter()
                            .map(|&returned_segment| {
                                println!("entering check_for_no_super()");
                                check_for_no_super(
                                    returned_segment,
                                    code_path_analyzer,
                                    &mut seen_segments,
                                )
                            })
                            .into();
                        match no_supers {
                            Found::No => {
                                context.report(violation! {
                                    node => root_node,
                                    message_id => "missing_all",
                                });
                            }
                            Found::InSome => {
                                context.report(violation! {
                                    node => root_node,
                                    message_id => "missing_some",
                                });
                            }
                            _ => ()
                        }
                    }
                }
            }
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        kind::{CallExpression, MethodDefinition},
        CodePathAnalyzerInstanceProviderFactory,
    };

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_constructor_super_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            constructor_super_rule(),
            rule_tests! {
                valid => [
                    // non derived classes.
                    "class A { }",
                    "class A { constructor() { } }",

                    /*
                     * inherit from non constructors.
                     * those are valid if we don't define the constructor.
                     */
                    "class A extends null { }",

                    // derived classes.
                    "class A extends B { }",
                    "class A extends B { constructor() { super(); } }",
                    "class A extends B { constructor() { if (true) { super(); } else { super(); } } }",
                    "class A extends (class B {}) { constructor() { super(); } }",
                    "class A extends (B = C) { constructor() { super(); } }",
                    "class A extends (B &&= C) { constructor() { super(); } }",
                    "class A extends (B ||= C) { constructor() { super(); } }",
                    "class A extends (B ??= C) { constructor() { super(); } }",
                    "class A extends (B ||= 5) { constructor() { super(); } }",
                    "class A extends (B ??= 5) { constructor() { super(); } }",
                    "class A extends (B || C) { constructor() { super(); } }",
                    "class A extends (5 && B) { constructor() { super(); } }",

                    // A future improvement could detect the left side as statically falsy, making this invalid.
                    "class A extends (false && B) { constructor() { super(); } }",
                    "class A extends (B || 5) { constructor() { super(); } }",
                    "class A extends (B ?? 5) { constructor() { super(); } }",

                    "class A extends (a ? B : C) { constructor() { super(); } }",
                    "class A extends (B, C) { constructor() { super(); } }",

                    // nested.
                    "class A { constructor() { class B extends C { constructor() { super(); } } } }",
                    "class A extends B { constructor() { super(); class C extends D { constructor() { super(); } } } }",
                    "class A extends B { constructor() { super(); class C { constructor() { } } } }",

                    // multi code path.
                    "class A extends B { constructor() { a ? super() : super(); } }",
                    "class A extends B { constructor() { if (a) super(); else super(); } }",
                    "class A extends B { constructor() { switch (a) { case 0: super(); break; default: super(); } } }",
                    "class A extends B { constructor() { try {} finally { super(); } } }",
                    "class A extends B { constructor() { if (a) throw Error(); super(); } }",

                    // returning value is a substitute of 'super()'.
                    "class A extends B { constructor() { if (true) return a; super(); } }",
                    "class A extends null { constructor() { return a; } }",
                    "class A { constructor() { return a; } }",

                    // https://github.com/eslint/eslint/issues/5261
                    "class A extends B { constructor(a) { super(); for (const b of a) { this.a(); } } }",

                    // https://github.com/eslint/eslint/issues/5319
                    "class Foo extends Object { constructor(method) { super(); this.method = method || function() {}; } }",

                    // https://github.com/eslint/eslint/issues/5394
                    "class A extends Object {
                        constructor() {
                            super();
                            for (let i = 0; i < 0; i++);
                        }
                    }",

                    // https://github.com/eslint/eslint/issues/8848
                    "
                        class A extends B {
                            constructor(props) {
                                super(props);

                                try {
                                    let arr = [];
                                    for (let a of arr) {
                                    }
                                } catch (err) {
                                }
                            }
                        }
                    ",

                    // Optional chaining
                    "class A extends obj?.prop { constructor() { super(); } }"
                ],
                invalid => [
                    // inherit from non constructors.
                    {
                        code => "class A extends null { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends null { constructor() { } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends 100 { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends 'test' { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends (B = 5) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends (B && 5) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {

                        // `B &&= 5` evaluates either to a falsy value of `B` (which, then, cannot be a constructor), or to '5'
                        code => "class A extends (B &&= 5) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends (B += C) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends (B -= C) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends (B **= C) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends (B |= C) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },
                    {
                        code => "class A extends (B &= C) { constructor() { super(); } }",
                        errors => [{ message_id => "bad_super", type => CallExpression }]
                    },

                    // derived classes.
                    {
                        code => "class A extends B { constructor() { } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { for (var a of b) super.foo(); } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    },

                    // nested execution scope.
                    {
                        code => "class A extends B { constructor() { class C extends D { constructor() { super(); } } } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { var c = class extends D { constructor() { super(); } } } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { var c = () => super(); } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { class C extends D { constructor() { super(); } } } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition, column => 21 }]
                    },
                    {
                        code => "class A extends B { constructor() { var C = class extends D { constructor() { super(); } } } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition, column => 21 }]
                    },
                    {
                        code => "class A extends B { constructor() { super(); class C extends D { constructor() { } } } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition, column => 66 }]
                    },
                    {
                        code => "class A extends B { constructor() { super(); var C = class extends D { constructor() { } } } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition, column => 72 }]
                    },

                    // lacked in some code path.
                    {
                        code => "class A extends B { constructor() { if (a) super(); } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { if (a); else super(); } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { a && super(); } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { switch (a) { case 0: super(); } } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { switch (a) { case 0: break; default: super(); } } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { try { super(); } catch (err) {} } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { try { a; } catch (err) { super(); } } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },
                    {
                        code => "class A extends B { constructor() { if (a) return; super(); } }",
                        errors => [{ message_id => "missing_some", type => MethodDefinition }]
                    },

                    // duplicate.
                    {
                        code => "class A extends B { constructor() { super(); super(); } }",
                        errors => [{ message_id => "duplicate", type => CallExpression, column => 46 }]
                    },
                    {
                        code => "class A extends B { constructor() { super() || super(); } }",
                        errors => [{ message_id => "duplicate", type => CallExpression, column => 48 }]
                    },
                    {
                        code => "class A extends B { constructor() { if (a) super(); super(); } }",
                        errors => [{ message_id => "duplicate", type => CallExpression, column => 53 }]
                    },
                    {
                        code => "class A extends B { constructor() { switch (a) { case 0: super(); default: super(); } } }",
                        errors => [{ message_id => "duplicate", type => CallExpression, column => 76 }]
                    },
                    {
                        code => "class A extends B { constructor(a) { while (a) super(); } }",
                        errors => [
                            { message_id => "missing_some", type => MethodDefinition },
                            { message_id => "duplicate", type => CallExpression, column => 48 }
                        ]
                    },

                    // ignores `super()` on unreachable paths.
                    {
                        code => "class A extends B { constructor() { return; super(); } }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    },

                    // https://github.com/eslint/eslint/issues/8248
                    {
                        code => "class Foo extends Bar {
                            constructor() {
                                for (a in b) for (c in d);
                            }
                        }",
                        errors => [{ message_id => "missing_all", type => MethodDefinition }]
                    }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
