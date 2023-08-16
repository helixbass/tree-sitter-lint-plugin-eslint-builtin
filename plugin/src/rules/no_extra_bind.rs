use std::{collections::HashSet, sync::Arc};

use once_cell::sync::Lazy;
use squalid::EverythingExt;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{
        call_expression_has_single_matching_argument, get_call_expression_arguments,
        range_between_start_and_end, NodeExtJs,
    },
    kind::{
        ArrowFunction, CallExpression, Function, Identifier, Kind, SpreadElement, This,
        LITERAL_KINDS,
    },
    utils::ast_utils,
};

static SIDE_EFFECT_FREE_NODE_TYPES: Lazy<HashSet<Kind>> = Lazy::new(|| {
    let mut ret: HashSet<Kind> = LITERAL_KINDS.clone();
    ret.insert(Identifier);
    ret.insert(This);
    ret.insert(Function);
    ret
});

fn is_side_effect_free(node: Node) -> bool {
    SIDE_EFFECT_FREE_NODE_TYPES.contains(node.kind())
}

fn report<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) {
    let member_node = node.next_non_parentheses_ancestor();
    let call_node = member_node.next_non_parentheses_ancestor();

    context.report(violation! {
        node => call_node,
        message_id => "unexpected",
        range => member_node.child_by_field_name("property").unwrap_or_else(|| {
            member_node.field("index")
        }).range(),
        fix => |fixer| {
            if !is_side_effect_free(get_call_expression_arguments(call_node).unwrap().next().unwrap()) {
                return;
            }

            let token_pairs = [
                (
                    context.get_token_after(
                        member_node.field("object"),
                        Some(|node: Node| ast_utils::is_not_closing_paren_token(node, context))
                    ),
                    context.get_last_token(member_node, Option::<fn(Node) -> bool>::None),
                ),
                (
                    context.get_token_after(
                        member_node,
                        Some(|node: Node| ast_utils::is_not_closing_paren_token(node, context))
                    ),
                    context.get_last_token(call_node, Option::<fn(Node) -> bool>::None),
                ),
            ];
            let first_token_to_remove = token_pairs[0].0;
            let last_token_to_remove = token_pairs[1].1;

            if context.comments_exist_between(first_token_to_remove, last_token_to_remove) {
                return;
            }

            token_pairs.into_iter().for_each(|(start, end)| {
                fixer.remove_range(range_between_start_and_end(start.range(), end.range()));
            });
        }
    });
}

fn is_callee_of_bind_method(node: Node, context: &QueryMatchContext) -> bool {
    let parent = node.next_non_parentheses_ancestor();
    if !ast_utils::is_specific_member_access(parent, Option::<&str>::None, Some("bind"), context) {
        return false;
    }

    let bind_node = parent;

    bind_node.next_non_parentheses_ancestor().thrush(|parent| {
        parent.kind() == CallExpression
            && parent.field("function").skip_parentheses() == bind_node
            && call_expression_has_single_matching_argument(parent, |arg| {
                arg.kind() != SpreadElement
            })
    })
}

#[derive(Debug)]
struct ScopeInfo<'a> {
    is_bound: bool,
    this_found: bool,
    upper: Option<Box<ScopeInfo<'a>>>,
    node: Node<'a>,
}

pub fn no_extra_bind_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-extra-bind",
        languages => [Javascript],
        fixable => true,
        messages => [
            unexpected => "The function binding is unnecessary.",
        ],
        state => {
            [per-file-run]
            scope_info: Option<ScopeInfo<'a>>,
        },
        listeners => [
            ArrowFunction => |node, context| {
                if is_callee_of_bind_method(node, context) {
                    report(node, context);
                }
            },
            r#"
              (function) @c
              (function_declaration) @c
              (generator_function) @c
              (generator_function_declaration) @c
              (method_definition) @c
            "# => |node, context| {
                let old_scope_info = self.scope_info.take();
                self.scope_info = Some(ScopeInfo {
                    is_bound: is_callee_of_bind_method(node, context),
                    this_found: false,
                    upper: old_scope_info.map(Box::new),
                    node,
                });
            },
            r#"
              function:exit,
              generator_function:exit,
              function_declaration:exit,
              generator_function_declaration:exit
            "# => |node, context| {
                let scope_info = self.scope_info.take().unwrap();
                if scope_info.is_bound && !scope_info.this_found {
                    report(scope_info.node, context);
                }
                self.scope_info = scope_info.upper.map(|upper| *upper);
            },
            r#"
              (this) @c
            "# => |node, context| {
                if let Some(scope_info) = self.scope_info.as_mut() {
                    scope_info.this_found = true;
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    #[test]
    fn test_no_extra_bind_rule() {
        let errors = [RuleTestExpectedErrorBuilder::default()
            .message_id("unexpected")
            .type_("call_expression")
            .build()
            .unwrap()];

        RuleTester::run(
            no_extra_bind_rule(),
            rule_tests! {
                valid => [
                    "var a = function(b) { return b }.bind(c, d)",
                    { code => "var a = function(b) { return b }.bind(...c)", /*parserOptions: { ecmaVersion: 6 }*/ },
                    "var a = function() { this.b }()",
                    "var a = function() { this.b }.foo()",
                    "var a = f.bind(a)",
                    "var a = function() { return this.b }.bind(c)",
                    { code => "var a = (() => { return b }).bind(c, d)", /*parserOptions: { ecmaVersion: 6 }*/ },
                    "(function() { (function() { this.b }.bind(this)) }.bind(c))",
                    "var a = function() { return 1; }[bind](b)",
                    { code => "var a = function() { return 1; }[`bi${n}d`](b)", /*parserOptions: { ecmaVersion: 6 }*/ },
                    { code => "var a = function() { return () => this; }.bind(b)", /*parserOptions: { ecmaVersion: 6 }*/ }
                ],
                invalid => [
                    {
                        code => "var a = function() { return 1; }.bind(b)",
                        output => "var a = function() { return 1; }",
                        errors => [{
                            message_id => "unexpected",
                            type => "call_expression",
                            line => 1,
                            column => 34,
                            end_line => 1,
                            end_column => 38
                        }]
                    },
                    {
                        code => "var a = function() { return 1; }['bind'](b)",
                        output => "var a = function() { return 1; }",
                        errors => [{
                            message_id => "unexpected",
                            type => "call_expression",
                            line => 1,
                            column => 34,
                            end_line => 1,
                            end_column => 40
                        }]
                    },
                    {
                        code => "var a = function() { return 1; }[`bind`](b)",
                        output => "var a = function() { return 1; }",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            type => "call_expression",
                            line => 1,
                            column => 34,
                            end_line => 1,
                            end_column => 40
                        }]
                    },
                    {
                        code => "var a = (() => { return 1; }).bind(b)",
                        output => "var a = (() => { return 1; })",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => errors,
                    },
                    {
                        code => "var a = (() => { return this; }).bind(b)",
                        output => "var a = (() => { return this; })",
                        // parserOptions: { ecmaVersion: 6 },
                        errors => errors,
                    },
                    {
                        code => "var a = function() { (function(){ this.c }) }.bind(b)",
                        output => "var a = function() { (function(){ this.c }) }",
                        errors => errors,
                    },
                    {
                        code => "var a = function() { function c(){ this.d } }.bind(b)",
                        output => "var a = function() { function c(){ this.d } }",
                        errors => errors,
                    },
                    {
                        code => "var a = function() { return 1; }.bind(this)",
                        output => "var a = function() { return 1; }",
                        errors => errors,
                    },
                    {
                        code => "var a = function() { (function(){ (function(){ this.d }.bind(c)) }) }.bind(b)",
                        output => "var a = function() { (function(){ (function(){ this.d }.bind(c)) }) }",
                        errors => [{ message_id => "unexpected", type => "call_expression", column => 71 }]
                    },
                    {
                        code => "var a = (function() { return 1; }).bind(this)",
                        output => "var a = (function() { return 1; })",
                        errors => errors,
                    },
                    {
                        code => "var a = (function() { return 1; }.bind)(this)",
                        output => "var a = (function() { return 1; })",
                        errors => errors,
                    },

                    // Should not autofix if bind expression args have side effects
                    {
                        code => "var a = function() {}.bind(b++)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind(b())",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind(b.c)",
                        output => None,
                        errors => errors,
                    },

                    // Should not autofix if it would remove comments
                    {
                        code => "var a = function() {}/**/.bind(b)",
                        output => "var a = function() {}/**/",
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}/**/['bind'](b)",
                        output => "var a = function() {}/**/",
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}//comment\n.bind(b)",
                        output => "var a = function() {}//comment\n",
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}./**/bind(b)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}[/**/'bind'](b)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.//\nbind(b)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind/**/(b)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind(\n/**/b)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind(b/**/)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind(b//\n)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind(b\n/**/)",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "var a = function() {}.bind(b)/**/",
                        output => "var a = function() {}/**/",
                        errors => errors,
                    },

                    // Optional chaining
                    {
                        code => "var a = function() { return 1; }.bind?.(b)",
                        output => "var a = function() { return 1; }",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unexpected" }]
                    },
                    {
                        code => "var a = function() { return 1; }?.bind(b)",
                        output => "var a = function() { return 1; }",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unexpected" }]
                    },
                    {
                        code => "var a = (function() { return 1; }?.bind)(b)",
                        output => "var a = (function() { return 1; })",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unexpected" }]
                    },
                    {
                        code => "var a = function() { return 1; }['bind']?.(b)",
                        output => "var a = function() { return 1; }",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unexpected" }]
                    },
                    {
                        code => "var a = function() { return 1; }?.['bind'](b)",
                        output => "var a = function() { return 1; }",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unexpected" }]
                    },
                    {
                        code => "var a = (function() { return 1; }?.['bind'])(b)",
                        output => "var a = (function() { return 1; })",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "unexpected" }]
                    }
                ]
            },
        )
    }
}
