use std::{borrow::Cow, collections::HashSet, sync::Arc};

use itertools::Itertools;
use serde::Deserialize;
use squalid::{EverythingExt, OptionExt};
use tree_sitter_lint::{
    range_between_start_and_end, range_between_starts, rule, tree_sitter::Node,
    tree_sitter_grep::SupportedLanguage, violation, NodeExt, QueryMatchContext, Rule,
};

use crate::{
    assert_kind,
    ast_helpers::{
        get_call_expression_arguments, is_async_function, is_chain_expression,
        is_logical_expression,
    },
    kind::{
        Arguments, BinaryExpression, CallExpression, FormalParameters, Identifier,
        MemberExpression, NewExpression, PropertyIdentifier, SubscriptExpression,
        TernaryExpression, This,
    },
    scope::{Scope, ScopeManager, Variable, VariableType},
    utils::ast_utils,
};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    allow_named_functions: bool,
    allow_unbound_this: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            allow_named_functions: false,
            allow_unbound_this: true,
        }
    }
}

#[derive(Default)]
struct StackEntry {
    this: bool,
    super_: bool,
    meta: bool,
}

fn is_function_name(variable: &Variable) -> bool {
    variable.defs().next().unwrap().type_() == VariableType::FunctionName
}

fn get_variable_of_arguments<'a, 'b>(scope: &Scope<'a, 'b>) -> Option<Variable<'a, 'b>> {
    scope
        .variables()
        .find(|variable| variable.name() == "arguments")
        .filter(|variable| variable.identifiers().next().is_none())
}

#[derive(Default)]
struct CallbackInfo {
    is_callback: bool,
    is_lexical_this: bool,
}

fn get_callback_info<'a>(node: Node<'a>, context: &QueryMatchContext<'a, '_>) -> CallbackInfo {
    let mut retv = CallbackInfo::default();
    let mut current_node = node;
    let mut parent = node.parent_(context);
    let mut bound = false;

    loop {
        match parent.kind() {
            BinaryExpression if is_logical_expression(parent) => (),
            _ if is_chain_expression(current_node) => (),
            TernaryExpression | Arguments => (),

            MemberExpression | SubscriptExpression => {
                if parent.field("object") == current_node
                    && parent.kind() == MemberExpression
                    && parent.field("property").thrush(|property| {
                        property.kind() == PropertyIdentifier && property.text(context) == "bind"
                    })
                {
                    let maybe_callee = parent;

                    if ast_utils::is_callee(maybe_callee, context) {
                        if !bound {
                            bound = true;
                            retv.is_lexical_this =
                                get_call_expression_arguments(maybe_callee.parent_(context))
                                    .matches(|args| {
                                        let args = args.collect_vec();
                                        args.len() == 1 && args[0].kind() == This
                                    });
                        }
                        parent = maybe_callee.parent_(context);
                    } else {
                        return retv;
                    }
                } else {
                    return retv;
                }
            }

            CallExpression => {
                if parent.field("function") != current_node {
                    retv.is_callback = true;
                }
                return retv;
            }
            NewExpression => {
                if parent.field("constructor") != current_node {
                    retv.is_callback = true;
                }
                return retv;
            }

            _ => return retv,
        }

        current_node = parent;
        parent = parent.parent_(context);
    }
}

fn has_duplicate_params(params_list: Node, context: &QueryMatchContext) -> bool {
    assert_kind!(params_list, FormalParameters);

    let params_list = params_list
        .non_comment_named_children(SupportedLanguage::Javascript)
        .collect_vec();
    params_list.iter().all(|param| param.kind() == Identifier)
        && params_list.len()
            != HashSet::<Cow<'_, str>>::from_iter(
                params_list.iter().map(|param| param.text(context)),
            )
            .len()
}

pub fn prefer_arrow_callback_rule() -> Arc<dyn Rule> {
    rule! {
        name => "prefer-arrow-callback",
        languages => [Javascript],
        messages => [
            prefer_arrow_callback => "Unexpected function expression.",
        ],
        fixable => true,
        // concatenate_adjacent_insert_fixes => true,
        options_type => Options,
        state => {
            [per-config]
            allow_named_functions: bool = options.allow_named_functions,
            allow_unbound_this: bool = options.allow_unbound_this,

            [per-file-run]
            stack: Vec<StackEntry>,
        },
        listeners => [
            r#"
              (this) @c
            "# => |node, context| {
                if let Some(info) = self.stack.last_mut() {
                    info.this = true;
                }
            },
            r#"
              (super) @c
            "# => |node, context| {
                if let Some(info) = self.stack.last_mut() {
                    info.super_ = true;
                }
            },
            r#"
              (meta_property) @c
            "# => |node, context| {
                if let Some(info) = self.stack.last_mut() {
                    info.meta = true;
                }
            },
            r#"
              (function_declaration) @c
              (function) @c
            "# => |node, context| {
                self.stack.push(StackEntry::default());
            },
            r#"
              function_declaration:exit
            "# => |node, context| {
                self.stack.pop().unwrap();
            },
            r#"
              function:exit
            "# => |node, context| {
                let scope_info = self.stack.pop().unwrap();

                if self.allow_named_functions &&
                    node.child_by_field_name("name").is_some() {
                    return;
                }

                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let name_var = scope_manager.get_declared_variables(node).next();
                if name_var.matches(|name_var| {
                    is_function_name(&name_var) &&
                        name_var.references().next().is_some()
                }) {
                    return;
                }

                let variable = get_variable_of_arguments(&scope_manager.get_scope(node));

                if variable.matches(|variable| {
                    variable.references().next().is_some()
                }) {
                    return;
                }

                let callback_info = get_callback_info(node, context);

                if callback_info.is_callback && (
                    !self.allow_unbound_this ||
                        !scope_info.this ||
                        callback_info.is_lexical_this
                ) &&
                    !scope_info.super_ &&
                    !scope_info.meta {
                    context.report(violation! {
                        node => node,
                        message_id => "prefer_arrow_callback",
                        fix => |fixer| {
                            if !callback_info.is_lexical_this && scope_info.this ||
                                has_duplicate_params(node.field("parameters"), context) {
                                return;
                            }

                            if callback_info.is_lexical_this {
                                let member_node = node.parent_(context);

                                if member_node.kind() != MemberExpression {
                                    return;
                                }

                                let call_node = member_node.parent_(context);
                                let first_token_to_remove = context.get_token_after(
                                    member_node.field("object"),
                                    Some(|node: Node| ast_utils::is_not_closing_paren_token(node, context))
                                );
                                let last_token_to_remove = context.get_last_token(
                                    call_node,
                                    Option::<fn(Node) -> bool>::None
                                );

                                if ast_utils::is_parenthesised(member_node) {
                                    return;
                                }

                                if context.comments_exist_between(first_token_to_remove, last_token_to_remove) {
                                    return;
                                }

                                fixer.remove_range(
                                    range_between_start_and_end(first_token_to_remove.range(), last_token_to_remove.range())
                                );
                            }

                            let function_token = context.get_first_token(
                                node,
                                Some(if is_async_function(node) {
                                    1
                                } else {
                                    0
                                })
                            );
                            let left_paren_token = context.get_token_after(
                                function_token,
                                Some(|node: Node| ast_utils::is_opening_paren_token(node, context))
                            );
                            let token_before_body = context.get_token_before(
                                node.field("body"),
                                Option::<fn(Node) -> bool>::None,
                            );

                            if context.comments_exist_between(
                                function_token,
                                left_paren_token
                            ) {
                                fixer.remove(function_token);
                                if let Some(id) = node.child_by_field_name("name") {
                                    fixer.remove(id);
                                }
                            } else {
                                fixer.remove_range(
                                    range_between_starts(
                                        function_token.range(),
                                        left_paren_token.range(),
                                    )
                                );
                            }
                            fixer.insert_text_after(token_before_body, " =>");

                            let mut replaced_node = if callback_info.is_lexical_this {
                                node.parent_(context).parent_(context)
                            } else {
                                node
                            };

                            if is_chain_expression(replaced_node) {
                                replaced_node = replaced_node.parent_(context);
                            }

                            if !matches!(
                                replaced_node.parent_(context).kind(),
                                Arguments | TernaryExpression
                            ) && !ast_utils::is_parenthesised(replaced_node) &&
                                !ast_utils::is_parenthesised(node) {
                                fixer.insert_text_before(replaced_node, "(");
                                fixer.insert_text_after(replaced_node, ")");
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
    use tree_sitter_lint::{rule_tests, RuleTestExpectedErrorBuilder, RuleTester};

    use super::*;
    use crate::{kind::Function, get_instance_provider_factory};

    #[test]
    fn test_prefer_arrow_callback_rule() {
        let errors = vec![RuleTestExpectedErrorBuilder::default()
            .message_id("prefer_arrow_callback")
            .type_(Function)
            .build()
            .unwrap()];

        RuleTester::run_with_from_file_run_context_instance_provider(
            prefer_arrow_callback_rule(),
            rule_tests! {
                valid => [
                    "foo(a => a);",
                    "foo(function*() {});",
                    "foo(function() { this; });",
                    { code => "foo(function bar() {});", options => { allow_named_functions => true } },
                    "foo(function() { (() => this); });",
                    "foo(function() { this; }.bind(obj));",
                    "foo(function() { this; }.call(this));",
                    "foo(a => { (function() {}); });",
                    "var foo = function foo() {};",
                    "(function foo() {})();",
                    "foo(function bar() { bar; });",
                    "foo(function bar() { arguments; });",
                    "foo(function bar() { arguments; }.bind(this));",
                    "foo(function bar() { new.target; });",
                    "foo(function bar() { new.target; }.bind(this));",
                    "foo(function bar() { this; }.bind(this, somethingElse));",
                    "foo((function() {}).bind.bar)",
                    "foo((function() { this.bar(); }).bind(obj).bind(this))"
                ],
                invalid => [
                    {
                        code => "foo(function bar() {});",
                        output => "foo(() => {});",
                        errors => errors,
                    },
                    {
                        code => "foo(function() {});",
                        output => "foo(() => {});",
                        options => { allow_named_functions => true },
                        errors => errors,
                    },
                    {
                        code => "foo(function bar() {});",
                        output => "foo(() => {});",
                        options => { allow_named_functions => false },
                        errors => errors,
                    },
                    {
                        code => "foo(function() {});",
                        output => "foo(() => {});",
                        errors => errors,
                    },
                    {
                        code => "foo(nativeCb || function() {});",
                        output => "foo(nativeCb || (() => {}));",
                        errors => errors,
                    },
                    {
                        code => "foo(bar ? function() {} : function() {});",
                        output => "foo(bar ? () => {} : () => {});",
                        errors => [errors[0], errors[0]],
                    },
                    {
                        code => "foo(function() { (function() { this; }); });",
                        output => "foo(() => { (function() { this; }); });",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { this; }.bind(this));",
                        output => "foo(() => { this; });",
                        errors => errors,
                    },
                    {
                        code => "foo(bar || function() { this; }.bind(this));",
                        output => "foo(bar || (() => { this; }));",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { (() => this); }.bind(this));",
                        output => "foo(() => { (() => this); });",
                        errors => errors,
                    },
                    {
                        code => "foo(function bar(a) { a; });",
                        output => "foo((a) => { a; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function(a) { a; });",
                        output => "foo((a) => { a; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function(arguments) { arguments; });",
                        output => "foo((arguments) => { arguments; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { this; });",
                        output => None, // No fix applied
                        options => { allow_unbound_this => false },
                        errors => errors,
                    },
                    {
                        code => "foo(function() { (() => this); });",
                        output => None, // No fix applied
                        options => { allow_unbound_this => false },
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo, bar, baz) { return foo * 2; })",
                        output => "qux((foo, bar, baz) => { return foo * 2; })",
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo, bar, baz) { return foo * bar; }.bind(this))",
                        output => "qux((foo, bar, baz) => { return foo * bar; })",
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo, bar, baz) { return foo * this.qux; }.bind(this))",
                        output => "qux((foo, bar, baz) => { return foo * this.qux; })",
                        errors => errors,
                    },
                    {
                        code => "foo(function() {}.bind(this, somethingElse))",
                        output => "foo((() => {}).bind(this, somethingElse))",
                        errors => errors,
                    },
                    {
                        code => "qux(function(foo = 1, [bar = 2] = [], {qux: baz = 3} = {foo: 'bar'}) { return foo + bar; });",
                        output => "qux((foo = 1, [bar = 2] = [], {qux: baz = 3} = {foo: 'bar'}) => { return foo + bar; });",
                        errors => errors,
                    },
                    {
                        code => "qux(function(baz, baz) { })",
                        output => None, // Duplicate parameter names are a SyntaxError in arrow functions
                        errors => errors,
                    },
                    {
                        code => "qux(function( /* no params */ ) { })",
                        output => "qux(( /* no params */ ) => { })",
                        errors => errors,
                    },
                    {
                        code => "qux(function( /* a */ foo /* b */ , /* c */ bar /* d */ , /* e */ baz /* f */ ) { return foo; })",
                        output => "qux(( /* a */ foo /* b */ , /* c */ bar /* d */ , /* e */ baz /* f */ ) => { return foo; })",
                        errors => errors,
                    },
                    {
                        code => "qux(async function (foo = 1, bar = 2, baz = 3) { return baz; })",
                        output => "qux(async (foo = 1, bar = 2, baz = 3) => { return baz; })",
                        errors => errors,
                    },
                    {
                        code => "qux(async function (foo = 1, bar = 2, baz = 3) { return this; }.bind(this))",
                        output => "qux(async (foo = 1, bar = 2, baz = 3) => { return this; })",
                        errors => errors,
                    },
                    {
                        code => "foo((bar || function() {}).bind(this))",
                        output => None,
                        errors => errors,
                    },
                    {
                        code => "foo(function() {}.bind(this).bind(obj))",
                        output => "foo((() => {}).bind(obj))",
                        errors => errors,
                    },

                    // Optional chaining
                    {
                        code => "foo?.(function() {});",
                        output => "foo?.(() => {});",
                        errors => errors,
                    },
                    {
                        code => "foo?.(function() { return this; }.bind(this));",
                        output => "foo?.(() => { return this; });",
                        errors => errors,
                    },
                    {
                        code => "foo(function() { return this; }?.bind(this));",
                        output => "foo(() => { return this; });",
                        errors => errors,
                    },
                    {
                        code => "foo((function() { return this; }?.bind)(this));",
                        output => None,
                        errors => errors,
                    },

                    // https://github.com/eslint/eslint/issues/16718
                    {
                        code => r#"
test(
    function ()
    { }
);
                        "#,
                        output => r#"
test(
    () =>
    { }
);
                        "#,
                        errors => errors,
                    },
                    {
                        code => r#"
test(
    function (
        ...args
    ) /* Lorem ipsum
    dolor sit amet. */ {
        return args;
    }
);
                        "#,
                        output => r#"
test(
    (
        ...args
    ) => /* Lorem ipsum
    dolor sit amet. */ {
        return args;
    }
);
                        "#,
                        errors => errors,
                    }
                ]
            },
            get_instance_provider_factory()
        )
    }
}
