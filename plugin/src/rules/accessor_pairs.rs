use std::{borrow::Cow, collections::HashSet, iter, sync::Arc};

use itertools::Itertools;
use serde::Deserialize;
use squalid::{BoolExt, EverythingExt, OptionExt};
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule,
};

use crate::{
    ast_helpers::{
        get_call_expression_arguments, get_method_definition_kind, is_class_member_static,
        MethodDefinitionKind, NodeExtJs,
    },
    kind::{
        Arguments, CallExpression, ComputedPropertyName, MethodDefinition, Object, Pair,
        PropertyIdentifier,
    },
    utils::ast_utils,
};

#[derive(Deserialize)]
#[serde(default)]
struct Options {
    get_without_set: bool,
    set_without_get: bool,
    enforce_for_class_members: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            get_without_set: Default::default(),
            set_without_get: true,
            enforce_for_class_members: true,
        }
    }
}

#[derive(Debug)]
enum CowStrOrVecNode<'a> {
    CowStr(Cow<'a, str>),
    VecNode(Vec<Node<'a>>),
}

impl<'a> From<Cow<'a, str>> for CowStrOrVecNode<'a> {
    fn from(value: Cow<'a, str>) -> Self {
        Self::CowStr(value)
    }
}

impl<'a> From<Vec<Node<'a>>> for CowStrOrVecNode<'a> {
    fn from(value: Vec<Node<'a>>) -> Self {
        Self::VecNode(value)
    }
}

fn are_equal_token_lists(a: &Vec<Node>, b: &Vec<Node>, context: &QueryMatchContext) -> bool {
    if a.len() != b.len() {
        return false;
    }
    for (a_item, b_item) in iter::zip(a, b) {
        if a_item.kind() != b_item.kind() {
            return false;
        }
        if a_item.text(context) != b_item.text(context) {
            return false;
        }
    }
    true
}

fn are_equal_keys(a: &CowStrOrVecNode, b: &CowStrOrVecNode, context: &QueryMatchContext) -> bool {
    match (a, b) {
        (CowStrOrVecNode::CowStr(a), CowStrOrVecNode::CowStr(b)) => a == b,
        (CowStrOrVecNode::VecNode(a), CowStrOrVecNode::VecNode(b)) => {
            are_equal_token_lists(a, b, context)
        }
        _ => false,
    }
}

fn is_argument_of_method_call(
    node: Node,
    index: usize,
    object: &str,
    property: &str,
    context: &QueryMatchContext,
) -> bool {
    let parent = node.parent().unwrap();
    if parent.kind() != Arguments {
        return false;
    }
    let grandparent = parent.parent().unwrap();

    grandparent.kind() == CallExpression
        && ast_utils::is_specific_member_access(
            grandparent.field("function").skip_parentheses(),
            Some(object),
            Some(property),
            context,
        )
        && get_call_expression_arguments(grandparent)
            .matches(|mut arguments| arguments.nth(index) == Some(node))
}

fn is_property_descriptor(node: Node, context: &QueryMatchContext) -> bool {
    if is_argument_of_method_call(node, 2, "Object", "defineProperty", context)
        || is_argument_of_method_call(node, 2, "Reflect", "defineProperty", context)
    {
        return true;
    }

    let grandparent = node.parent().unwrap().parent().unwrap();

    grandparent.kind() == Object
        && (is_argument_of_method_call(grandparent, 1, "Object", "create", context)
            || is_argument_of_method_call(grandparent, 1, "Object", "defineProperties", context))
}

fn report(node: Node, message_kind: &str, context: &QueryMatchContext) {
    match node.kind() {
        MethodDefinition => match node.parent().unwrap().kind() {
            Object => {
                context.report(violation! {
                    node => node,
                    message_id => format!("{message_kind}_in_object_literal"),
                    range => ast_utils::get_function_head_range(node),
                    data => {
                        name => ast_utils::get_function_name_with_kind(node, context)
                    }
                });
            }
            _ => {
                context.report(violation! {
                    node => node,
                    message_id => format!("{message_kind}_in_class"),
                    range => ast_utils::get_function_head_range(node),
                    data => {
                        name => ast_utils::get_function_name_with_kind(node, context)
                    }
                });
            }
        },
        _ => {
            context.report(violation! {
                node => node,
                message_id => format!("{message_kind}_in_property_descriptor"),
            });
        }
    }
}

fn report_list(nodes: &[Node], message_kind: &str, context: &QueryMatchContext) {
    for &node in nodes {
        report(node, message_kind, context);
    }
}

struct FoundAccessors<'a> {
    key: CowStrOrVecNode<'a>,
    getters: Vec<Node<'a>>,
    setters: Vec<Node<'a>>,
}

impl<'a> From<CowStrOrVecNode<'a>> for FoundAccessors<'a> {
    fn from(key: CowStrOrVecNode<'a>) -> Self {
        Self {
            key,
            getters: Default::default(),
            setters: Default::default(),
        }
    }
}

pub fn accessor_pairs_rule() -> Arc<dyn Rule> {
    rule! {
        name => "accessor-pairs",
        languages => [Javascript],
        messages => [
            missing_getter_in_property_descriptor => "Getter is not present in property descriptor.",
            missing_setter_in_property_descriptor => "Setter is not present in property descriptor.",
            missing_getter_in_object_literal => "Getter is not present for {{ name }}.",
            missing_setter_in_object_literal => "Setter is not present for {{ name }}.",
            missing_getter_in_class => "Getter is not present for class {{ name }}.",
            missing_setter_in_class => "Setter is not present for class {{ name }}."
        ],
        options_type => Options,
        state => {
            [per-run]
            check_get_without_set: bool = options.get_without_set,
            check_set_without_get: bool = options.set_without_get,
            enforce_for_class_members: bool = options.enforce_for_class_members,
        },
        methods => {
            fn check_list(
                &self,
                nodes: impl Iterator<Item = Node<'a>>,
                context: &QueryMatchContext<'a, '_>,
            ) {
                let mut accessors: Vec<FoundAccessors> = Default::default();

                for node in nodes {
                    let accessor_kind = get_method_definition_kind(node, context);
                    if !matches!(
                        accessor_kind,
                        MethodDefinitionKind::Get | MethodDefinitionKind::Set
                    ) {
                        continue;
                    }

                    let name = ast_utils::get_static_property_name(node, context);
                    let key: CowStrOrVecNode<'a> = name.map_or_else(
                        || {
                            context
                                .get_tokens(
                                    node.field("name").thrush(|name| match name.kind() {
                                        ComputedPropertyName => name
                                            .first_non_comment_named_child(SupportedLanguage::Javascript)
                                            .skip_parentheses(),
                                        _ => name,
                                    }),
                                    Option::<fn(Node) -> bool>::None,
                                )
                                .collect_vec()
                                .into()
                        },
                        Into::into,
                    );

                    let accessor = if let Some(index) = accessors
                        .iter()
                        .position(|accessor| are_equal_keys(&accessor.key, &key, context))
                    {
                        &mut accessors[index]
                    } else {
                        accessors.push(key.into());
                        accessors.last_mut().unwrap()
                    };
                    match accessor_kind {
                        MethodDefinitionKind::Get => accessor.getters.push(node),
                        MethodDefinitionKind::Set => accessor.setters.push(node),
                        _ => unreachable!(),
                    }
                }

                for FoundAccessors {
                    getters, setters, ..
                } in accessors
                {
                    if self.check_set_without_get && !setters.is_empty() && getters.is_empty() {
                        report_list(&setters, "missing_getter", context);
                    }
                    if self.check_get_without_set && !getters.is_empty() && setters.is_empty() {
                        report_list(&getters, "missing_setter", context);
                    }
                }
            }

            fn check_object_literal(
                &self,
                node: Node<'a>,
                context: &QueryMatchContext<'a, '_>,
            ) {
                self.check_list(
                    node.non_comment_named_children(SupportedLanguage::Javascript)
                        .filter(|child| child.kind() == MethodDefinition),
                    context,
                );
            }

            fn check_property_descriptor(
                &self,
                node: Node<'a>,
                context: &QueryMatchContext<'a, '_>,
            ) {
                let names_to_check: HashSet<Cow<'_, str>> = node
                    .non_comment_named_children(SupportedLanguage::Javascript)
                    .filter_map(|child| {
                        (child.kind() == Pair).then_and(|| {
                            child
                                .field("key")
                                .when(|key| key.kind() == PropertyIdentifier)
                                .map(|key| key.text(context))
                        })
                    })
                    .collect();

                let has_getter = names_to_check.contains("get");
                let has_setter = names_to_check.contains("set");

                if self.check_set_without_get && has_setter && !has_getter {
                    report(node, "missing_getter", context);
                }
                if self.check_get_without_set && has_getter && !has_setter {
                    report(node, "missing_setter", context);
                }
            }

            fn check_class_body(
                &self,
                node: Node<'a>,
                context: &QueryMatchContext<'a, '_>,
            ) {
                let method_definitions = node
                    .non_comment_named_children(SupportedLanguage::Javascript)
                    .filter(|child| child.kind() == MethodDefinition)
                    .collect_vec();
                self.check_list(
                    method_definitions
                        .iter()
                        .filter(|&&m| is_class_member_static(m, context))
                        .copied(),
                    context,
                );
                self.check_list(
                    method_definitions
                        .iter()
                        .filter(|&&m| !is_class_member_static(m, context))
                        .copied(),
                    context,
                );
            }

            fn check_object_expression(
                &self,
                node: Node<'a>,
                context: &QueryMatchContext<'a, '_>,
            ) {
                self.check_object_literal(node, context);
                if is_property_descriptor(node, context) {
                    self.check_property_descriptor(node, context);
                }
            }
        },
        listeners => [
            r#"
              (object) @c
            "# => |node, context| {
                if !(self.check_set_without_get || self.check_get_without_set) {
                    return;
                }
                self.check_object_expression(node, context);
            },
            r#"
              (class_body) @c
            "# => |node, context| {
                if !self.enforce_for_class_members {
                    return;
                }
                self.check_class_body(node, context);
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::MethodDefinition;

    #[test]
    fn test_accessor_pairs_rule() {
        RuleTester::run(
            accessor_pairs_rule(),
            rule_tests! {
                valid => [
                    //------------------------------------------------------------------------------
                    // General
                    //------------------------------------------------------------------------------

                    // Does not check object patterns
                    {
                        code => "var { get: foo } = bar; ({ set: foo } = bar);",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var { set } = foo; ({ get } = foo);",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },

                    //------------------------------------------------------------------------------
                    // Object literals
                    //------------------------------------------------------------------------------

                    // Test default settings, this would be an error if `getWithoutSet` was set to `true`
                    "var o = { get a() {} }",
                    {
                        code => "var o = { get a() {} }",
                        options => {}
                    },

                    // No accessors
                    {
                        code => "var o = {};",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { a: 1 };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { a };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { a: get };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { a: set };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { get: function(){} };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { set: function(foo){} };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { get };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { set };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { [get]: function() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { [set]: function(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { set(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },

                    // Disabled options
                    {
                        code => "var o = { get a() {} };",
                        options => { set_without_get => false, get_without_set => false }
                    },
                    {
                        code => "var o = { get a() {} };",
                        options => { set_without_get => true, get_without_set => false }
                    },
                    {
                        code => "var o = { set a(foo) {} };",
                        options => { set_without_get => false, get_without_set => false }
                    },
                    {
                        code => "var o = { set a(foo) {} };",
                        options => { set_without_get => false, get_without_set => true }
                    },
                    {
                        code => "var o = { set a(foo) {} };",
                        options => { set_without_get => false }
                    },

                    // Valid pairs with identifiers
                    {
                        code => "var o = { get a() {}, set a(foo) {} };",
                        options => { set_without_get => false, get_without_set => true }
                    },
                    {
                        code => "var o = { get a() {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => false }
                    },
                    {
                        code => "var o = { get a() {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { set a(foo) {}, get a() {} };",
                        options => { set_without_get => true, get_without_set => true }
                    },

                    // Valid pairs with statically computed names
                    {
                        code => "var o = { get 'a'() {}, set 'a'(foo) {} };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { get a() {}, set 'a'(foo) {} };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { get ['abc']() {}, set ['abc'](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [1e2]() {}, set 100(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get abc() {}, set [`abc`](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get ['123']() {}, set 123(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },

                    // Valid pairs with expressions
                    {
                        code => "var o = { get [a]() {}, set [a](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [a]() {}, set [(a)](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                    },
                    {
                        code => "var o = { get [(a)]() {}, set [a](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [a]() {}, set [ a ](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [/*comment*/a/*comment*/]() {}, set [a](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [f()]() {}, set [f()](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [f(a)]() {}, set [f(a)](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [a + b]() {}, set [a + b](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get [`${a}`]() {}, set [`${a}`](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },

                    // Multiple valid pairs in the same literal
                    {
                        code => "var o = { get a() {}, set a(foo) {}, get b() {}, set b(bar) {} };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { get a() {}, set c(foo) {}, set a(bar) {}, get b() {}, get c() {}, set b(baz) {} };",
                        options => { set_without_get => true, get_without_set => true }
                    },

                    // Valid pairs with other elements
                    {
                        code => "var o = { get a() {}, set a(foo) {}, b: bar };",
                        options => { set_without_get => true, get_without_set => true }
                    },
                    {
                        code => "var o = { get a() {}, b, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get a() {}, ...b, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 2018 }
                    },
                    {
                        code => "var o = { get a() {}, set a(foo) {}, ...a };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 2018 }
                    },

                    // Duplicate keys. This is the responsibility of no-dupe-keys, but this rule still checks is there the other accessor kind.
                    {
                        code => "var o = { get a() {}, get a() {}, set a(foo) {}, };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get a() {}, set a(foo) {}, get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get a() {}, set a(foo) {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { set a(bar) {}, get a() {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get a() {}, get a() {} };",
                        options => { set_without_get => true, get_without_set => false },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { set a(foo) {}, set a(foo) {} };",
                        options => { set_without_get => false, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { get a() {}, set a(foo) {}, a };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "var o = { a, get a() {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },

                    /*
                     * This should be actually invalid by this rule!
                     * This code creates a property with the setter only, the getter will be ignored.
                     * It's treated as 3 attempts to define the same key, and the last wins.
                     * However, this edge case is not covered, it should be reported by no-dupe-keys anyway.
                     */
                    {
                        code => "var o = { get a() {}, a:1, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 }
                    },

                    //------------------------------------------------------------------------------
                    // Property descriptors
                    //------------------------------------------------------------------------------

                    "var o = {a: 1};\n Object.defineProperty(o, 'b', \n{set: function(value) {\n val = value; \n},\n get: function() {\n return val; \n} \n});",

                    // https://github.com/eslint/eslint/issues/3262
                    "var o = {set: function() {}}",
                    "Object.defineProperties(obj, {set: {value: function() {}}});",
                    "Object.create(null, {set: {value: function() {}}});",
                    { code => "var o = {get: function() {}}", options => { get_without_set => true } },
                    { code => "var o = {[set]: function() {}}", environment => { ecma_version => 6 } },
                    { code => "var set = 'value'; Object.defineProperty(obj, 'foo', {[set]: function(value) {}});", environment => { ecma_version => 6 } },

                    //------------------------------------------------------------------------------
                    // Classes
                    //------------------------------------------------------------------------------

                    // Test default settings
                    {
                        code => "class A { get a() {} }",
                        options => { enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get #a() {} }",
                        options => { enforce_for_class_members => true },
                        environment => { ecma_version => 13 }
                    },

                    // Explicitly disabled option
                    {
                        code => "class A { set a(foo) {} }",
                        options => { enforce_for_class_members => false },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get a() {} set b(foo) {} static get c() {} static set d(bar) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => false },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "(class A { get a() {} set b(foo) {} static get c() {} static set d(bar) {} });",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => false },
                        environment => { ecma_version => 6 }
                    },

                    // Disabled accessor kind options
                    {
                        code => "class A { get a() {} }",
                        options => { set_without_get => true, get_without_set => false, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { set a(foo) {} }",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static get a() {} }",
                        options => { set_without_get => true, get_without_set => false, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static set a(foo) {} }",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { set a(foo) {} };",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get a() {} set b(foo) {} static get c() {} static set d(bar) {} }",
                        options => { set_without_get => false, get_without_set => false, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    // No accessors
                    {
                        code => "class A {}",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "(class {})",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { constructor () {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static a() {} 'b'() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { [a]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { a() {} static a() {} b() {} static c() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    // Valid pairs with identifiers
                    {
                        code => "class A { get a() {} set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { set a(foo) {} get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static get a() {} static set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static set a(foo) {} static get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "(class { set a(foo) {} get a() {} });",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    // Valid pairs with statically computed names
                    {
                        code => "class A { get 'a'() {} set ['a'](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { set [`a`](foo) {} get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get 'a'() {} set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { static get 1e2() {} static set [100](foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    // Valid pairs with expressions
                    {
                        code => "class A { get [a]() {} set [a](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { set [(f())](foo) {} get [(f())]() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static set [f(a)](foo) {} static get [f(a)]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    // Multiple valid pairs in the same class
                    {
                        code => "class A { get a() {} set b(foo) {} set a(bar) {} get b() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get a() {} set a(bar) {} b() {} set c(foo) {} get c() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "(class { get a() {} static set a(foo) {} set a(bar) {} static get a() {} });",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    // Valid pairs with other elements
                    {
                        code => "class A { get a() {} b() {} set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { set a(foo) {} get a() {} b() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { a() {} get b() {} c() {} set b(foo) {} d() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get a() {} set a(foo) {} static a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { static get a() {} static b() {} static set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { static set a(foo) {} static get a() {} a() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    // Duplicate keys. This is the responsibility of no-dupe-class-members, but this rule still checks if there is the other accessor kind.
                    {
                        code => "class A { get a() {} get a() {} set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get [a]() {} set [a](foo) {} set [a](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { get a() {} set 'a'(foo) {} get [`a`]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { get a() {} set a(foo) {} a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "A = class { a() {} get a() {} set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static set a(foo) {} static set a(foo) {} static get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static get a() {} static set a(foo) {} static get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static set a(foo) {} static get a() {} static a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },

                    /*
                     * This code should be invalid by this rule because it creates a class with the setter only, while the getter is ignored.
                     * However, this edge case is not covered, it should be reported by no-dupe-class-members anyway.
                     */
                    {
                        code => "class A { get a() {} a() {} set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class A { static set a(foo) {} static a() {} static get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 }
                    }
                ],
                invalid => [
                    //------------------------------------------------------------------------------
                    // Object literals
                    //------------------------------------------------------------------------------

                    // Test default settings
                    {
                        code => "var o = { set a(value) {} };",
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set a(value) {} };",
                        options => {},
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition }]
                    },

                    // Test that the options do not affect each other
                    {
                        code => "var o = { set a(value) {} };",
                        options => { set_without_get => true, get_without_set => false },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set a(value) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get a() {} };",
                        options => { set_without_get => false, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get a() {} };",
                        options => { get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition }]
                    },

                    // Various kinds of the getter's key
                    {
                        code => "var o = { get abc() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get 'abc'() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get 123() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter '123'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get 1e2() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter '100'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get ['abc']() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get [`abc`]() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get [123]() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter '123'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get [abc]() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get [f(abc)]() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { get [a + b]() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter.", type => MethodDefinition }]
                    },

                    // Various kinds of the setter's key
                    {
                        code => "var o = { set abc(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set 'abc'(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set 123(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter '123'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set 1e2(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter '100'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set ['abc'](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set [`abc`](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set [123](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter '123'.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set [abc](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set [f(abc)](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter.", type => MethodDefinition }]
                    },
                    {
                        code => "var o = { set [a + b](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter.", type => MethodDefinition }]
                    },

                    // Different keys
                    {
                        code => "var o = { get a() {}, set b(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'b'.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "var o = { set a(foo) {}, get b() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for getter 'b'.", type => MethodDefinition, column => 26 }
                        ]
                    },
                    {
                        code => "var o = { get 1() {}, set b(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter '1'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'b'.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "var o = { get a() {}, set 1(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter '1'.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "var o = { get a() {}, set 'a '(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'a '.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "var o = { get ' a'() {}, set 'a'(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter ' a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 26 }
                        ]
                    },
                    {
                        code => "var o = { get ''() {}, set ' '(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter ''.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter ' '.", type => MethodDefinition, column => 24 }
                        ]
                    },
                    {
                        code => "var o = { get ''() {}, set null(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter ''.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'null'.", type => MethodDefinition, column => 24 }
                        ]
                    },
                    {
                        code => "var o = { get [`a`]() {}, set b(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'b'.", type => MethodDefinition, column => 27 }
                        ]
                    },
                    {
                        code => "var o = { get [a]() {}, set [b](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter.", type => MethodDefinition, column => 25 }
                        ]
                    },
                    {
                        code => "var o = { get [a]() {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 25 }
                        ]
                    },
                    {
                        code => "var o = { get a() {}, set [a](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "var o = { get [a + b]() {}, set [a - b](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter.", type => MethodDefinition, column => 29 }
                        ]
                    },
                    // TODO: uncomment if https://github.com/tree-sitter/tree-sitter-javascript/issues/275 gets resolved?
                    // {
                    //     code => "var o = { get [`abc${0}wro`]() {}, set [`${0}`](foo) {} };",
                    //     options => { set_without_get => true, get_without_set => true },
                    //     environment => { ecma_version => 6 },
                    //     errors => [
                    //         { message => "Setter is not present for getter.", type => MethodDefinition, column => 11 },
                    //         { message => "Getter is not present for setter.", type => MethodDefinition, column => 31 }
                    //     ],
                    // },

                    // Multiple invalid of same and different kinds
                    {
                        code => "var o = { get a() {}, get b() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for getter 'b'.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "var o = { set a(foo) {}, set b(bar) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'b'.", type => MethodDefinition, column => 26 }
                        ]
                    },
                    {
                        code => "var o = { get a() {}, set b(foo) {}, set c(foo) {}, get d() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'b'.", type => MethodDefinition, column => 23 },
                            { message => "Getter is not present for setter 'c'.", type => MethodDefinition, column => 38 },
                            { message => "Setter is not present for getter 'd'.", type => MethodDefinition, column => 53 }
                        ]
                    },

                    // Checks per object literal
                    {
                        code => "var o1 = { get a() {} }, o2 = { set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 12 },
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 33 }
                        ]
                    },
                    {
                        code => "var o1 = { set a(foo) {} }, o2 = { get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 12 },
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 36 }
                        ]
                    },

                    // Combinations or valid and invalid
                    {
                        code => "var o = { get a() {}, get b() {}, set b(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "var o = { get b() {}, get a() {}, set b(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 23 }]
                    },
                    {
                        code => "var o = { get b() {}, set b(foo) {}, get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 38 }]
                    },
                    {
                        code => "var o = { set a(foo) {}, get b() {}, set b(bar) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "var o = { get b() {}, set a(foo) {}, set b(bar) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 23 }]
                    },
                    {
                        code => "var o = { get b() {}, set b(bar) {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 38 }]
                    },
                    {
                        code => "var o = { get v1() {}, set i1(foo) {}, get v2() {}, set v2(bar) {}, get i2() {}, set v1(baz) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [
                            { message => "Getter is not present for setter 'i1'.", type => MethodDefinition, column => 24 },
                            { message => "Setter is not present for getter 'i2'.", type => MethodDefinition, column => 69 }
                        ]
                    },

                    // In the case of duplicates which don't have the other kind, all nodes are reported
                    {
                        code => "var o = { get a() {}, get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "var o = { set a(foo) {}, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 26 }
                        ]
                    },

                    // Other elements or even value property duplicates in the same literal do not affect this rule
                    {
                        code => "var o = { a, get b() {}, c };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter 'b'.", type => MethodDefinition, column => 14 }]
                    },
                    {
                        code => "var o = { a, get b() {}, c, set d(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter 'b'.", type => MethodDefinition, column => 14 },
                            { message => "Getter is not present for setter 'd'.", type => MethodDefinition, column => 29 }
                        ]
                    },
                    {
                        code => "var o = { get a() {}, a:1 };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "var o = { a, get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 14 }]
                    },
                    {
                        code => "var o = { set a(foo) {}, a:1 };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "var o = { a, set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 14 }]
                    },
                    {
                        code => "var o = { get a() {}, ...b };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 2018 },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "var o = { get a() {}, ...a };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 2018 },
                        errors => [{ message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "var o = { set a(foo) {}, ...a };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 2018 },
                        errors => [{ message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 11 }]
                    },

                    // Full location tests
                    {
                        code => "var o = { get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        errors => [{
                            message => "Setter is not present for getter 'a'.",
                            type => MethodDefinition,
                            line => 1,
                            column => 11,
                            end_line => 1,
                            end_column => 16
                        }]
                    },
                    {
                        code => "var o = {\n  set [\n a](foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 2015 },
                        errors => [{
                            message => "Getter is not present for setter.",
                            type => MethodDefinition,
                            line => 2,
                            column => 3,
                            end_line => 3,
                            end_column => 4
                        }]
                    },

                    //------------------------------------------------------------------------------
                    // Property descriptors
                    //------------------------------------------------------------------------------

                    {
                        code => "var o = {d: 1};\n Object.defineProperty(o, 'c', \n{set: function(value) {\n val = value; \n} \n});",
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }],
                    },
                    {
                        code => "Reflect.defineProperty(obj, 'foo', {set: function(value) {}});",
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "Object.defineProperties(obj, {foo: {set: function(value) {}}});",
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "Object.create(null, {foo: {set: function(value) {}}});",
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "var o = {d: 1};\n Object?.defineProperty(o, 'c', \n{set: function(value) {\n val = value; \n} \n});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }],
                    },
                    {
                        code => "Reflect?.defineProperty(obj, 'foo', {set: function(value) {}});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "Object?.defineProperties(obj, {foo: {set: function(value) {}}});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "Object?.create(null, {foo: {set: function(value) {}}});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "var o = {d: 1};\n (Object?.defineProperty)(o, 'c', \n{set: function(value) {\n val = value; \n} \n});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }],
                    },
                    {
                        code => "(Reflect?.defineProperty)(obj, 'foo', {set: function(value) {}});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "(Object?.defineProperties)(obj, {foo: {set: function(value) {}}});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },
                    {
                        code => "(Object?.create)(null, {foo: {set: function(value) {}}});",
                        environment => { ecma_version => 2020 },
                        errors => [{ message => "Getter is not present in property descriptor.", type => Object }]
                    },

                    //------------------------------------------------------------------------------
                    // Classes
                    //------------------------------------------------------------------------------

                    // Test default settings
                    {
                        code => "class A { set a(foo) {} }",
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { get a() {} set b(foo) {} }",
                        options => {},
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'b'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { get a() {} }",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static get a() {} }",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class static getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class static setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "A = class { get a() {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "A = class { get a() {} set b(foo) {} };",
                        options => { set_without_get => true, get_without_set => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition },
                            { message => "Getter is not present for class setter 'b'.", type => MethodDefinition }
                        ]
                    },
                    {
                        code => "class A { set a(value) {} }",
                        options => { enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static set a(value) {} }",
                        options => { enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class static setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "A = class { set a(value) {} };",
                        options => { enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "(class A { static set a(value) {} });",
                        options => { enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class static setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { set '#a'(foo) {} }",
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Getter is not present for class setter '#a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { set #a(foo) {} }",
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Getter is not present for class private setter #a.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static set '#a'(foo) {} }",
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Getter is not present for class static setter '#a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static set #a(foo) {} }",
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Getter is not present for class static private setter #a.", type => MethodDefinition }]
                    },

                    // Test that the accessor kind options do not affect each other
                    {
                        code => "class A { set a(value) {} }",
                        options => { set_without_get => true, get_without_set => false, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "A = class { static set a(value) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class static setter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "let foo = class A { get a() {} };",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static get a() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class static getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "(class { get a() {} });",
                        options => { get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { get '#a'() {} };",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Setter is not present for class getter '#a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { get #a() {} };",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Setter is not present for class private getter #a.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static get '#a'() {} };",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Setter is not present for class static getter '#a'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static get #a() {} };",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 13 },
                        errors => [{ message => "Setter is not present for class static private getter #a.", type => MethodDefinition }]
                    },

                    // Various kinds of keys
                    {
                        code => "class A { get abc() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "A = class { static set 'abc'(foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class static setter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "(class { get 123() {} });",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter '123'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static get 1e2() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class static getter '100'.", type => MethodDefinition }]
                    },
                    {
                        code => "A = class { get ['abc']() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { set [`abc`](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'abc'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static get [123]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class static getter '123'.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { get [abc]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { static get [f(abc)]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class static getter.", type => MethodDefinition }]
                    },
                    {
                        code => "A = class { set [a + b](foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter.", type => MethodDefinition }]
                    },
                    {
                        code => "class A { get ['constructor']() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'constructor'.", type => MethodDefinition }]
                    },

                    // Different keys
                    {
                        code => "class A { get a() {} set b(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter 'b'.", type => MethodDefinition, column => 22 }
                        ]
                    },
                    {
                        code => "A = class { set a(foo) {} get b() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Setter is not present for class getter 'b'.", type => MethodDefinition, column => 27 }
                        ]
                    },
                    {
                        code => "A = class { static get a() {} static set b(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Getter is not present for class static setter 'b'.", type => MethodDefinition, column => 31 }
                        ]
                    },
                    {
                        code => "class A { get a() {} set b(foo) {} }",
                        options => { set_without_get => false, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 }
                        ]
                    },
                    {
                        code => "class A { get a() {} set b(foo) {} }",
                        options => { set_without_get => true, get_without_set => false, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter 'b'.", type => MethodDefinition, column => 22 }
                        ]
                    },
                    {
                        code => "class A { get 'a '() {} set 'a'(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a '.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 25 }
                        ]
                    },
                    {
                        code => "class A { get 'a'() {} set 1(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter '1'.", type => MethodDefinition, column => 24 }
                        ]
                    },
                    {
                        code => "class A { get 1() {} set 2(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter '1'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter '2'.", type => MethodDefinition, column => 22 }
                        ]
                    },
                    {
                        code => "class A { get ''() {} set null(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter ''.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter 'null'.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "class A { get a() {} set [a](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter.", type => MethodDefinition, column => 22 }
                        ]
                    },
                    {
                        code => "class A { get [a]() {} set [b](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter.", type => MethodDefinition, column => 24 }
                        ]
                    },
                    {
                        code => "class A { get [a]() {} set [a++](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter.", type => MethodDefinition, column => 24 }
                        ]
                    },
                    {
                        code => "class A { get [a + b]() {} set [a - b](foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter.", type => MethodDefinition, column => 28 }
                        ]
                    },
                    {
                        code => "class A { get #a() {} set '#a'(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 13 },
                        errors => [
                            { message => "Setter is not present for class private getter #a.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter '#a'.", type => MethodDefinition, column => 23 }
                        ]
                    },
                    {
                        code => "class A { get '#a'() {} set #a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 13 },
                        errors => [
                            { message => "Setter is not present for class getter '#a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class private setter #a.", type => MethodDefinition, column => 25 }
                        ]
                    },

                    // Prototype and static accessors with same keys
                    {
                        code => "class A { get a() {} static set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class static setter 'a'.", type => MethodDefinition, column => 22 }
                        ]
                    },
                    {
                        code => "A = class { static get a() {} set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 31 }
                        ]
                    },
                    {
                        code => "class A { set [a](foo) {} static get [a]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for class static getter.", type => MethodDefinition, column => 27 }
                        ]
                    },
                    {
                        code => "class A { static set [a](foo) {} get [a]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class static setter.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 34 }
                        ]
                    },

                    // Multiple invalid of same and different kinds
                    {
                        code => "class A { get a() {} get b() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for class getter 'b'.", type => MethodDefinition, column => 22 }
                        ]
                    },
                    {
                        code => "A = class { get a() {} get [b]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 24 }
                        ]
                    },
                    {
                        code => "class A { get [a]() {} get [b]() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 24 }
                        ]
                    },
                    {
                        code => "A = class { set a(foo) {} set b(bar) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Getter is not present for class setter 'b'.", type => MethodDefinition, column => 27 }
                        ]
                    },
                    {
                        code => "class A { static get a() {} static get b() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for class static getter 'b'.", type => MethodDefinition, column => 29 }
                        ]
                    },
                    {
                        code => "A = class { static set a(foo) {} static set b(bar) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class static setter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Getter is not present for class static setter 'b'.", type => MethodDefinition, column => 34 }
                        ]
                    },
                    {
                        code => "class A { static get a() {} set b(foo) {} static set c(bar) {} get d() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter 'b'.", type => MethodDefinition, column => 29 },
                            { message => "Getter is not present for class static setter 'c'.", type => MethodDefinition, column => 43 },
                            { message => "Setter is not present for class getter 'd'.", type => MethodDefinition, column => 64 }
                        ]
                    },

                    // Checks per class
                    {
                        code => "class A { get a() {} } class B { set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 34 }
                        ]
                    },
                    {
                        code => "A = class { set a(foo) {} }, class { get a() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 38 }
                        ]
                    },
                    {
                        code => "A = class { get a() {} }, { set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Getter is not present for setter 'a'.", type => MethodDefinition, column => 29 }
                        ]
                    },
                    {
                        code => "A = { get a() {} }, class { set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for getter 'a'.", type => MethodDefinition, column => 7 },
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 29 }
                        ]
                    },

                    // Combinations or valid and invalid
                    {
                        code => "class A { get a() {} get b() {} set b(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "A = class { get b() {} get a() {} set b(foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 24 }]
                    },
                    {
                        code => "class A { set b(foo) {} get b() {} set a(bar) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 36 }]
                    },
                    {
                        code => "A = class { static get b() {} set a(foo) {} static set b(bar) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 31 }]
                    },
                    {
                        code => "class A { static set a(foo) {} get b() {} set b(bar) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Getter is not present for class static setter 'a'.", type => MethodDefinition, column => 11 }]
                    },
                    {
                        code => "class A { get b() {} static get a() {} set b(bar) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 22 }]
                    },
                    {
                        code => "class A { static set b(foo) {} static get a() {} static get b() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{ message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 32 }]
                    },
                    {
                        code => "class A { get [v1](){} static set i1(foo){} static set v2(bar){} get [i2](){} static get i3(){} set [v1](baz){} static get v2(){} set i4(quux){} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class static setter 'i1'.", type => MethodDefinition, column => 24 },
                            { message => "Setter is not present for class getter.", type => MethodDefinition, column => 66 },
                            { message => "Setter is not present for class static getter 'i3'.", type => MethodDefinition, column => 79 },
                            { message => "Getter is not present for class setter 'i4'.", type => MethodDefinition, column => 131 }
                        ]
                    },

                    // In the case of duplicates which don't have the other kind, all nodes are reported
                    {
                        code => "class A { get a() {} get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 22 }
                        ]
                    },
                    {
                        code => "A = class { set a(foo) {} set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 27 }
                        ]
                    },
                    {
                        code => "A = class { static get a() {} static get a() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 13 },
                            { message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 31 }
                        ]
                    },
                    {
                        code => "class A { set a(foo) {} set a(foo) {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 11 },
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 25 }
                        ]
                    },

                    // Other elements or even method duplicates in the same class do not affect this rule
                    {
                        code => "class A { a() {} get b() {} c() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'b'.", type => MethodDefinition, column => 18 }
                        ]
                    },
                    {
                        code => "A = class { a() {} get b() {} c() {} set d(foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'b'.", type => MethodDefinition, column => 20 },
                            { message => "Getter is not present for class setter 'd'.", type => MethodDefinition, column => 38 }
                        ]
                    },
                    {
                        code => "class A { static a() {} get b() {} static c() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'b'.", type => MethodDefinition, column => 25 }
                        ]
                    },
                    {
                        code => "class A { a() {} get a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class getter 'a'.", type => MethodDefinition, column => 18 }
                        ]
                    },
                    {
                        code => "A = class { static a() {} set a(foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class setter 'a'.", type => MethodDefinition, column => 27 }
                        ]
                    },
                    {
                        code => "class A { a() {} static get b() {} c() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class static getter 'b'.", type => MethodDefinition, column => 18 }
                        ]
                    },
                    {
                        code => "A = class { static a() {} static set b(foo) {} static c() {} d() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class static setter 'b'.", type => MethodDefinition, column => 27 }
                        ]
                    },
                    {
                        code => "class A { a() {} static get a() {} a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Setter is not present for class static getter 'a'.", type => MethodDefinition, column => 18 }
                        ]
                    },
                    {
                        code => "class A { static set a(foo) {} static a() {} }",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [
                            { message => "Getter is not present for class static setter 'a'.", type => MethodDefinition, column => 11 }
                        ]
                    },

                    // Full location tests
                    {
                        code => "class A { get a() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message => "Setter is not present for class getter 'a'.",
                            type => MethodDefinition,
                            line => 1,
                            column => 11,
                            end_line => 1,
                            end_column => 16
                        }]
                    },
                    {
                        code => "A = class {\n  set [\n a](foo) {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message => "Getter is not present for class setter.",
                            type => MethodDefinition,
                            line => 2,
                            column => 3,
                            end_line => 3,
                            end_column => 4
                        }]
                    },
                    {
                        code => "class A { static get a() {} };",
                        options => { set_without_get => true, get_without_set => true, enforce_for_class_members => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message => "Setter is not present for class static getter 'a'.",
                            type => MethodDefinition,
                            line => 1,
                            column => 11,
                            end_line => 1,
                            end_column => 23
                        }]
                    }
                ]
            },
        )
    }
}
