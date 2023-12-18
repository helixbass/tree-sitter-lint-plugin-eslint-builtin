use std::{collections::HashSet, sync::Arc};

use id_arena::Id;
use squalid::{EverythingExt, OptionExt};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, Rule};

use crate::{
    ast_helpers::{get_method_definition_kind, MethodDefinitionKind},
    kind::{CallExpression, MethodDefinition, Super, This},
    utils::ast_utils,
    CodePath, CodePathAnalyzer, CodePathSegment, EnterOrExit,
};

fn _look_for_this_before_super<'a>(
    current_segment: Id<CodePathSegment<'a>>,
    code_path_analyzer: &CodePathAnalyzer<'a>,
    mut seen_segments: HashSet<Id<CodePathSegment<'a>>>,
    nodes_to_report: &mut HashSet<Node<'a>>,
) {
    if seen_segments.contains(&current_segment) {
        return;
    }
    seen_segments.insert(current_segment);

    for (enter_or_exit, node) in &code_path_analyzer.code_path_segment_arena[current_segment].nodes
    {
        match *enter_or_exit {
            EnterOrExit::Exit => {
                if node.kind() == CallExpression && node.field("function").kind() == Super {
                    return;
                }
            }
            EnterOrExit::Enter => match node.kind() {
                This => {
                    nodes_to_report.insert(*node);
                }
                Super if !ast_utils::is_callee(*node) => {
                    nodes_to_report.insert(*node);
                }
                _ => (),
            },
        }
    }

    code_path_analyzer.code_path_segment_arena[current_segment]
        .next_segments
        .iter()
        .for_each(|&next_segment| {
            _look_for_this_before_super(
                next_segment,
                code_path_analyzer,
                seen_segments.clone(),
                nodes_to_report,
            );
        });
}

fn look_for_this_before_super<'a>(
    code_path: Id<CodePath<'a>>,
    code_path_analyzer: &CodePathAnalyzer<'a>,
    nodes_to_report: &mut HashSet<Node<'a>>,
) {
    _look_for_this_before_super(
        code_path_analyzer.code_path_arena[code_path]
            .state
            .initial_segment,
        code_path_analyzer,
        Default::default(),
        nodes_to_report,
    )
}

pub fn no_this_before_super_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-this-before-super",
        languages => [Javascript],
        messages => [
            no_before_super => "'{{kind}}' is not allowed before 'super()'.",
        ],
        listeners => [
            "program:exit" => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                let mut nodes_to_report: HashSet<Node<'a>> = Default::default();

                for &code_path in code_path_analyzer.code_paths.iter().filter(|&&code_path| {
                    code_path_analyzer.code_path_arena[code_path]
                        .root_node(&code_path_analyzer.code_path_segment_arena)
                        .thrush(|root_node| {
                            if !(root_node.kind() == MethodDefinition
                                && get_method_definition_kind(root_node, context)
                                    == MethodDefinitionKind::Constructor)
                            {
                                return false;
                            }

                            let class_node = root_node.parent().unwrap().parent().unwrap();

                            class_node
                                .maybe_first_child_of_kind("class_heritage")
                                .matches(|class_heritage| {
                                    !ast_utils::is_null_or_undefined(class_heritage)
                                })
                        })
                }) {
                    look_for_this_before_super(code_path, code_path_analyzer, &mut nodes_to_report);
                }

                for node in nodes_to_report {
                    context.report(violation! {
                        message_id => "no_before_super",
                        node => node,
                        data => {
                            kind => match node.kind() {
                                Super => "super",
                                This => "this",
                                _ => unreachable!()
                            }
                        }
                    });
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::{
        get_instance_provider_factory,
        kind::{Super, This},
    };

    #[test]
    fn test_no_this_before_super_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_this_before_super_rule(),
            rule_tests! {
                valid => [
                    /*
                     * if the class has no extends or `extends null`, just ignore.
                     * those classes cannot call `super()`.
                     */
                    "class A { }",
                    "class A { constructor() { } }",
                    "class A { constructor() { this.b = 0; } }",
                    "class A { constructor() { this.b(); } }",
                    "class A extends null { }",
                    "class A extends null { constructor() { } }",

                    // allows `this`/`super` after `super()`.
                    "class A extends B { }",
                    "class A extends B { constructor() { super(); } }",
                    "class A extends B { constructor() { super(); this.c = this.d; } }",
                    "class A extends B { constructor() { super(); this.c(); } }",
                    "class A extends B { constructor() { super(); super.c(); } }",
                    "class A extends B { constructor() { if (true) { super(); } else { super(); } this.c(); } }",
                    "class A extends B { constructor() { foo = super(); this.c(); } }",
                    "class A extends B { constructor() { foo += super().a; this.c(); } }",
                    "class A extends B { constructor() { foo |= super().a; this.c(); } }",
                    "class A extends B { constructor() { foo &= super().a; this.c(); } }",

                    // allows `this`/`super` in nested executable scopes, even if before `super()`.
                    "class A extends B { constructor() { class B extends C { constructor() { super(); this.d = 0; } } super(); } }",
                    "class A extends B { constructor() { var B = class extends C { constructor() { super(); this.d = 0; } }; super(); } }",
                    "class A extends B { constructor() { function c() { this.d(); } super(); } }",
                    "class A extends B { constructor() { var c = function c() { this.d(); }; super(); } }",
                    "class A extends B { constructor() { var c = () => this.d(); super(); } }",

                    // ignores out of constructors.
                    "class A { b() { this.c = 0; } }",
                    "class A extends B { c() { this.d = 0; } }",
                    "function a() { this.b = 0; }",

                    // multi code path.
                    "class A extends B { constructor() { if (a) { super(); this.a(); } else { super(); this.b(); } } }",
                    "class A extends B { constructor() { if (a) super(); else super(); this.a(); } }",
                    "class A extends B { constructor() { try { super(); } finally {} this.a(); } }",

                    // https://github.com/eslint/eslint/issues/5261
                    "class A extends B { constructor(a) { super(); for (const b of a) { this.a(); } } }",
                    "class A extends B { constructor(a) { for (const b of a) { foo(b); } super(); } }",

                    // https://github.com/eslint/eslint/issues/5319
                    "class A extends B { constructor(a) { super(); this.a = a && function(){} && this.foo; } }",

                    // https://github.com/eslint/eslint/issues/5394
                    "class A extends Object {
                        constructor() {
                            super();
                            for (let i = 0; i < 0; i++);
                            this;
                        }
                    }",

                    // https://github.com/eslint/eslint/issues/5894
                    "class A { constructor() { return; this; } }",
                    "class A extends B { constructor() { return; this; } }",

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

                    // Class field initializers are always evaluated after `super()`.
                    "class C { field = this.toString(); }",
                    "class C extends B { field = this.foo(); }",
                    "class C extends B { field = this.foo(); constructor() { super(); } }",
                    "class C extends B { field = this.foo(); constructor() { } }" // < in this case, initializers are never evaluated.
                ],
                invalid => [
                    // disallows all `this`/`super` if `super()` is missing.
                    {
                        code => "class A extends B { constructor() { this.c = 0; } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { this.c(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super.c(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "super" }, type => Super }]
                    },

                    // disallows `this`/`super` before `super()`.
                    {
                        code => "class A extends B { constructor() { this.c = 0; super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { this.c(); super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super.c(); super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "super" }, type => Super }]
                    },

                    // disallows `this`/`super` in arguments of `super()`.
                    {
                        code => "class A extends B { constructor() { super(this.c); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super(this.c()); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { super(super.c()); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "super" }, type => Super }]
                    },

                    // even if is nested, reports correctly.
                    {
                        code => "class A extends B { constructor() { class C extends D { constructor() { super(); this.e(); } } this.f(); super(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This, column => 96 }]
                    },
                    {
                        code => "class A extends B { constructor() { class C extends D { constructor() { this.e(); super(); } } super(); this.f(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This, column => 73 }]
                    },

                    // multi code path.
                    {
                        code => "class A extends B { constructor() { if (a) super(); this.a(); } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { try { super(); } finally { this.a; } } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { try { super(); } catch (err) { } this.a; } }",
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { foo &&= super().a; this.c(); } }",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { foo ||= super().a; this.c(); } }",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    },
                    {
                        code => "class A extends B { constructor() { foo ??= super().a; this.c(); } }",
                        // parserOptions: { ecmaVersion: 2021 },
                        errors => [{ message_id => "no_before_super", data => { kind => "this" }, type => This }]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
