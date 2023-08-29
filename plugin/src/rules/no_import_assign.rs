use std::sync::Arc;

use once_cell::sync::Lazy;
use regex::Regex;
use squalid::{OptionExt, EverythingExt};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::{get_call_expression_arguments, NodeExtJs},
    kind::{
        ArrayPattern, AssignmentExpression, AssignmentPattern, AugmentedAssignmentExpression,
        CallExpression, ForInStatement, MemberExpression, NamespaceImport, ObjectPattern,
        PairPattern, RestPattern, SubscriptExpression, UnaryExpression, UpdateExpression, Arguments,
    },
    scope::{Scope, ScopeManager, ScopeType},
    utils::{ast_utils, eslint_utils::find_variable},
};

static WELL_KNOWN_MUTATION_FUNCTIONS_OBJECT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^(?:assign|definePropert(?:y|ies)|freeze|setPrototypeOf)$"#).unwrap()
});

static WELL_KNOWN_MUTATION_FUNCTIONS_REFLECT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^(?:(?:define|delete)Property|set(?:PrototypeOf)?)$"#).unwrap());

fn is_assignment_left(node: Node) -> bool {
    let parent = node.parent().unwrap();

    [AssignmentExpression, AugmentedAssignmentExpression].contains(&parent.kind())
        && parent.field("left") == node
        || parent.kind() == ArrayPattern
        || parent.kind() == PairPattern
            && parent.field("value") == node
            && parent.parent().unwrap().kind() == ObjectPattern
        || parent.kind() == RestPattern
        || parent.kind() == AssignmentPattern && parent.field("left") == node
}

fn is_argument_of_well_known_mutation_function<'a>(
    node: Node<'a>,
    scope: &Scope<'a, '_>,
    context: &QueryMatchContext<'a, '_>,
) -> bool {
    let parent = node.parent().unwrap();
    if parent.kind() != Arguments {
        return false;
    }
    let parent = parent.parent().unwrap();

    if parent.kind() != CallExpression
        || !get_call_expression_arguments(parent)
            .matches(|mut arguments| arguments.next().matches(|argument| argument == node))
    {
        return false;
    }

    let callee = parent.field("function").skip_parentheses();

    if !(ast_utils::is_specific_member_access(
        callee,
        Some("Object"),
        Some(&*WELL_KNOWN_MUTATION_FUNCTIONS_OBJECT),
        context,
    ) || ast_utils::is_specific_member_access(
        callee,
        Some("Reflect"),
        Some(&*WELL_KNOWN_MUTATION_FUNCTIONS_REFLECT),
        context,
    )) {
        return false;
    }

    find_variable(scope, callee.field("object"), context)
        .matches(|variable| variable.scope().type_() == ScopeType::Global)
}

fn is_operand_of_mutation_unary_operator(node: Node) -> bool {
    let argument_node = node;
    let parent = argument_node.parent().unwrap();

    parent.kind() == UpdateExpression && parent.field("argument") == argument_node
        || parent.kind() == UnaryExpression
            && parent.field("operator").kind() == "delete"
            && parent.field("argument") == argument_node
}

fn is_iteration_variable(node: Node) -> bool {
    node.parent().unwrap().thrush(|parent| {
        parent.kind() == ForInStatement && parent.field("left") == node
    })
}

fn is_member_write<'a>(id: Node<'a>, scope: &Scope<'a, '_>, context: &QueryMatchContext<'a, '_>) -> bool {
    let parent = id.parent().unwrap();

    [MemberExpression, SubscriptExpression].contains(&parent.kind())
        && parent.field("object") == id
        && (is_assignment_left(parent)
            || is_operand_of_mutation_unary_operator(parent)
            || is_iteration_variable(parent))
        || is_argument_of_well_known_mutation_function(id, scope, context)
}

fn get_write_node(id: Node) -> Node {
    let mut node = id.parent();

    while let Some(node_present) = node.filter(|node| {
        !matches!(
            node.kind(),
            AssignmentExpression
                | AugmentedAssignmentExpression
                | UpdateExpression
                | UnaryExpression
                | CallExpression
                | ForInStatement
        )
    }) {
        node = node_present.parent();
    }

    node.unwrap_or(id)
}

pub fn no_import_assign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-import-assign",
        languages => [Javascript],
        messages => [
            readonly => "'{{name}}' is read-only.",
            readonly_member => "The members of '{{name}}' are read-only.",
        ],
        listeners => [
            r#"
              (import_statement) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();

                let scope = scope_manager.get_scope(node);

                for variable in scope_manager.get_declared_variables(node) {
                    let should_check_members = variable.defs().any(|d| {
                        d.node().kind() == NamespaceImport
                    });
                    let mut prev_id_node: Option<Node> = Default::default();

                    for reference in variable.references() {
                        let id_node = reference.identifier();

                        if Some(id_node) == prev_id_node {
                            continue;
                        }
                        prev_id_node = Some(id_node);

                        if reference.is_write() {
                            context.report(violation! {
                                node => get_write_node(id_node),
                                message_id => "readonly",
                                data => {
                                    name => id_node.text(context),
                                }
                            });
                        } else if should_check_members && is_member_write(id_node, &scope, context) {
                            context.report(violation! {
                                node => get_write_node(id_node),
                                message_id => "readonly_member",
                                data => {
                                    name => id_node.text(context),
                                }
                            });
                        }
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use squalid::json_object;
    use tree_sitter_lint::{rule_tests, RuleTester};

    use crate::get_instance_provider_factory;

    use super::*;

    #[test]
    fn test_no_import_assign_rule() {
        RuleTester::run_with_instance_provider_and_environment(
            no_import_assign_rule(),
            rule_tests! {
                valid => [
                    "import mod from 'mod'; mod.prop = 0",
                    "import mod from 'mod'; mod.prop += 0",
                    "import mod from 'mod'; mod.prop++",
                    "import mod from 'mod'; delete mod.prop",
                    "import mod from 'mod'; for (mod.prop in foo);",
                    "import mod from 'mod'; for (mod.prop of foo);",
                    "import mod from 'mod'; [mod.prop] = foo;",
                    "import mod from 'mod'; [...mod.prop] = foo;",
                    "import mod from 'mod'; ({ bar: mod.prop } = foo);",
                    "import mod from 'mod'; ({ ...mod.prop } = foo);",
                    "import {named} from 'mod'; named.prop = 0",
                    "import {named} from 'mod'; named.prop += 0",
                    "import {named} from 'mod'; named.prop++",
                    "import {named} from 'mod'; delete named.prop",
                    "import {named} from 'mod'; for (named.prop in foo);",
                    "import {named} from 'mod'; for (named.prop of foo);",
                    "import {named} from 'mod'; [named.prop] = foo;",
                    "import {named} from 'mod'; [...named.prop] = foo;",
                    "import {named} from 'mod'; ({ bar: named.prop } = foo);",
                    "import {named} from 'mod'; ({ ...named.prop } = foo);",
                    "import * as mod from 'mod'; mod.named.prop = 0",
                    "import * as mod from 'mod'; mod.named.prop += 0",
                    "import * as mod from 'mod'; mod.named.prop++",
                    "import * as mod from 'mod'; delete mod.named.prop",
                    "import * as mod from 'mod'; for (mod.named.prop in foo);",
                    "import * as mod from 'mod'; for (mod.named.prop of foo);",
                    "import * as mod from 'mod'; [mod.named.prop] = foo;",
                    "import * as mod from 'mod'; [...mod.named.prop] = foo;",
                    "import * as mod from 'mod'; ({ bar: mod.named.prop } = foo);",
                    "import * as mod from 'mod'; ({ ...mod.named.prop } = foo);",
                    "import * as mod from 'mod'; obj[mod] = 0",
                    "import * as mod from 'mod'; obj[mod.named] = 0",
                    "import * as mod from 'mod'; for (var foo in mod.named);",
                    "import * as mod from 'mod'; for (var foo of mod.named);",
                    "import * as mod from 'mod'; [bar = mod.named] = foo;",
                    "import * as mod from 'mod'; ({ bar = mod.named } = foo);",
                    "import * as mod from 'mod'; ({ bar: baz = mod.named } = foo);",
                    "import * as mod from 'mod'; ({ [mod.named]: bar } = foo);",
                    "import * as mod from 'mod'; var obj = { ...mod.named };",
                    "import * as mod from 'mod'; var obj = { foo: mod.named };",
                    "import mod from 'mod'; { let mod = 0; mod = 1 }",
                    "import * as mod from 'mod'; { let mod = 0; mod = 1 }",
                    "import * as mod from 'mod'; { let mod = 0; mod.named = 1 }",
                    "import {} from 'mod'",
                    "import 'mod'",
                    "import mod from 'mod'; Object.assign(mod, obj);",
                    "import {named} from 'mod'; Object.assign(named, obj);",
                    "import * as mod from 'mod'; Object.assign(mod.prop, obj);",
                    "import * as mod from 'mod'; Object.assign(obj, mod, other);",
                    "import * as mod from 'mod'; Object[assign](mod, obj);",
                    "import * as mod from 'mod'; Object.getPrototypeOf(mod);",
                    "import * as mod from 'mod'; Reflect.set(obj, key, mod);",
                    "import * as mod from 'mod'; { var Object; Object.assign(mod, obj); }",
                    "import * as mod from 'mod'; var Object; Object.assign(mod, obj);",
                    "import * as mod from 'mod'; Object.seal(mod, obj)",
                    "import * as mod from 'mod'; Object.preventExtensions(mod)",
                    "import * as mod from 'mod'; Reflect.preventExtensions(mod)"
                ],
                invalid => [
                    {
                        code => "import mod1 from 'mod'; mod1 = 0",
                        errors => [{ message_id => "readonly", data => { name => "mod1" }, column => 25 }]
                    },
                    {
                        code => "import mod2 from 'mod'; mod2 += 0",
                        errors => [{ message_id => "readonly", data => { name => "mod2" }, column => 25 }]
                    },
                    {
                        code => "import mod3 from 'mod'; mod3++",
                        errors => [{ message_id => "readonly", data => { name => "mod3" }, column => 25 }]
                    },
                    {
                        code => "import mod4 from 'mod'; for (mod4 in foo);",
                        errors => [{ message_id => "readonly", data => { name => "mod4" }, column => 25 }]
                    },
                    {
                        code => "import mod5 from 'mod'; for (mod5 of foo);",
                        errors => [{ message_id => "readonly", data => { name => "mod5" }, column => 25 }]
                    },
                    {
                        code => "import mod6 from 'mod'; [mod6] = foo",
                        errors => [{ message_id => "readonly", data => { name => "mod6" }, column => 25 }]
                    },
                    {
                        code => "import mod7 from 'mod'; [mod7 = 0] = foo",
                        errors => [{ message_id => "readonly", data => { name => "mod7" }, column => 25 }]
                    },
                    {
                        code => "import mod8 from 'mod'; [...mod8] = foo",
                        errors => [{ message_id => "readonly", data => { name => "mod8" }, column => 25 }]
                    },
                    {
                        code => "import mod9 from 'mod'; ({ bar: mod9 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "mod9" }, column => 26 }]
                    },
                    {
                        code => "import mod10 from 'mod'; ({ bar: mod10 = 0 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "mod10" }, column => 27 }]
                    },
                    {
                        code => "import mod11 from 'mod'; ({ ...mod11 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "mod11" }, column => 27 }]
                    },
                    {
                        code => "import {named1} from 'mod'; named1 = 0",
                        errors => [{ message_id => "readonly", data => { name => "named1" }, column => 29 }]
                    },
                    {
                        code => "import {named2} from 'mod'; named2 += 0",
                        errors => [{ message_id => "readonly", data => { name => "named2" }, column => 29 }]
                    },
                    {
                        code => "import {named3} from 'mod'; named3++",
                        errors => [{ message_id => "readonly", data => { name => "named3" }, column => 29 }]
                    },
                    {
                        code => "import {named4} from 'mod'; for (named4 in foo);",
                        errors => [{ message_id => "readonly", data => { name => "named4" }, column => 29 }]
                    },
                    {
                        code => "import {named5} from 'mod'; for (named5 of foo);",
                        errors => [{ message_id => "readonly", data => { name => "named5" }, column => 29 }]
                    },
                    {
                        code => "import {named6} from 'mod'; [named6] = foo",
                        errors => [{ message_id => "readonly", data => { name => "named6" }, column => 29 }]
                    },
                    {
                        code => "import {named7} from 'mod'; [named7 = 0] = foo",
                        errors => [{ message_id => "readonly", data => { name => "named7" }, column => 29 }]
                    },
                    {
                        code => "import {named8} from 'mod'; [...named8] = foo",
                        errors => [{ message_id => "readonly", data => { name => "named8" }, column => 29 }]
                    },
                    {
                        code => "import {named9} from 'mod'; ({ bar: named9 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "named9" }, column => 30 }]
                    },
                    {
                        code => "import {named10} from 'mod'; ({ bar: named10 = 0 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "named10" }, column => 31 }]
                    },
                    {
                        code => "import {named11} from 'mod'; ({ ...named11 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "named11" }, column => 31 }]
                    },
                    {
                        code => "import {named12 as foo} from 'mod'; foo = 0; named12 = 0",
                        errors => [{ message_id => "readonly", data => { name => "foo" }, column => 37 }]
                    },
                    {
                        code => "import * as mod1 from 'mod'; mod1 = 0",
                        errors => [{ message_id => "readonly", data => { name => "mod1" }, column => 30 }]
                    },
                    {
                        code => "import * as mod2 from 'mod'; mod2 += 0",
                        errors => [{ message_id => "readonly", data => { name => "mod2" }, column => 30 }]
                    },
                    {
                        code => "import * as mod3 from 'mod'; mod3++",
                        errors => [{ message_id => "readonly", data => { name => "mod3" }, column => 30 }]
                    },
                    {
                        code => "import * as mod4 from 'mod'; for (mod4 in foo);",
                        errors => [{ message_id => "readonly", data => { name => "mod4" }, column => 30 }]
                    },
                    {
                        code => "import * as mod5 from 'mod'; for (mod5 of foo);",
                        errors => [{ message_id => "readonly", data => { name => "mod5" }, column => 30 }]
                    },
                    {
                        code => "import * as mod6 from 'mod'; [mod6] = foo",
                        errors => [{ message_id => "readonly", data => { name => "mod6" }, column => 30 }]
                    },
                    {
                        code => "import * as mod7 from 'mod'; [mod7 = 0] = foo",
                        errors => [{ message_id => "readonly", data => { name => "mod7" }, column => 30 }]
                    },
                    {
                        code => "import * as mod8 from 'mod'; [...mod8] = foo",
                        errors => [{ message_id => "readonly", data => { name => "mod8" }, column => 30 }]
                    },
                    {
                        code => "import * as mod9 from 'mod'; ({ bar: mod9 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "mod9" }, column => 31 }]
                    },
                    {
                        code => "import * as mod10 from 'mod'; ({ bar: mod10 = 0 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "mod10" }, column => 32 }]
                    },
                    {
                        code => "import * as mod11 from 'mod'; ({ ...mod11 } = foo)",
                        errors => [{ message_id => "readonly", data => { name => "mod11" }, column => 32 }]
                    },
                    {
                        code => "import * as mod1 from 'mod'; mod1.named = 0",
                        errors => [{ message_id => "readonly_member", data => { name => "mod1" }, column => 30 }]
                    },
                    {
                        code => "import * as mod2 from 'mod'; mod2.named += 0",
                        errors => [{ message_id => "readonly_member", data => { name => "mod2" }, column => 30 }]
                    },
                    {
                        code => "import * as mod3 from 'mod'; mod3.named++",
                        errors => [{ message_id => "readonly_member", data => { name => "mod3" }, column => 30 }]
                    },
                    {
                        code => "import * as mod4 from 'mod'; for (mod4.named in foo);",
                        errors => [{ message_id => "readonly_member", data => { name => "mod4" }, column => 30 }]
                    },
                    {
                        code => "import * as mod5 from 'mod'; for (mod5.named of foo);",
                        errors => [{ message_id => "readonly_member", data => { name => "mod5" }, column => 30 }]
                    },
                    {
                        code => "import * as mod6 from 'mod'; [mod6.named] = foo",
                        errors => [{ message_id => "readonly_member", data => { name => "mod6" }, column => 30 }]
                    },
                    {
                        code => "import * as mod7 from 'mod'; [mod7.named = 0] = foo",
                        errors => [{ message_id => "readonly_member", data => { name => "mod7" }, column => 30 }]
                    },
                    {
                        code => "import * as mod8 from 'mod'; [...mod8.named] = foo",
                        errors => [{ message_id => "readonly_member", data => { name => "mod8" }, column => 30 }]
                    },
                    {
                        code => "import * as mod9 from 'mod'; ({ bar: mod9.named } = foo)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod9" }, column => 31 }]
                    },
                    {
                        code => "import * as mod10 from 'mod'; ({ bar: mod10.named = 0 } = foo)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod10" }, column => 32 }]
                    },
                    {
                        code => "import * as mod11 from 'mod'; ({ ...mod11.named } = foo)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod11" }, column => 32 }]
                    },
                    {
                        code => "import * as mod12 from 'mod'; delete mod12.named",
                        errors => [{ message_id => "readonly_member", data => { name => "mod12" }, column => 31 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Object.assign(mod, obj)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Object.defineProperty(mod, key, d)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Object.defineProperties(mod, d)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Object.setPrototypeOf(mod, proto)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Object.freeze(mod)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Reflect.defineProperty(mod, key, d)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Reflect.deleteProperty(mod, key)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Reflect.set(mod, key, value)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; Reflect.setPrototypeOf(mod, proto)",
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import mod, * as mod_ns from 'mod'; mod.prop = 0; mod_ns.prop = 0",
                        errors => [{ message_id => "readonly_member", data => { name => "mod_ns" }, column => 51 }]
                    },

                    // Optional chaining
                    {
                        code => "import * as mod from 'mod'; Object?.defineProperty(mod, key, d)",
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; (Object?.defineProperty)(mod, key, d)",
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    },
                    {
                        code => "import * as mod from 'mod'; delete mod?.prop",
                        environment => { ecma_version => 2020 },
                        errors => [{ message_id => "readonly_member", data => { name => "mod" }, column => 29 }]
                    }
                ]
            },
            get_instance_provider_factory(),
            json_object!({
                "ecma_version": 2018,
                "source_type": "module",
            })
        )
    }
}
