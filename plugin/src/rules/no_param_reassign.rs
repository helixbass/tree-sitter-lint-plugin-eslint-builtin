use std::{collections::HashSet, sync::Arc};

use itertools::Itertools;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use squalid::return_default_if_none;
use tree_sitter_lint::{rule, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    kind::{
        AssignmentExpression, AugmentedAssignmentExpression, CallExpression, ForInStatement,
        PairPattern, SubscriptExpression, TernaryExpression, UnaryExpression, UpdateExpression,
    },
    scope::{Reference, ScopeManager, VariableType},
};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    props: bool,
    ignore_property_modifications_for: Vec<String>,
    #[serde(with = "serde_regex")]
    ignore_property_modifications_for_regex: Vec<Regex>,
}

static STOP_NODE_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:Statement|Declaration|Function|Program)$"#).unwrap());

fn is_modifying_prop(reference: &Reference) -> bool {
    let mut node = reference.identifier();
    let Some(mut parent) = node.parent() else {
        return false;
    };

    while !STOP_NODE_PATTERN.is_match(parent.kind()) || parent.kind() == ForInStatement {
        match parent.kind() {
            AssignmentExpression | AugmentedAssignmentExpression => {
                return parent.field("left") == node
            }
            UpdateExpression => return true,
            UnaryExpression => {
                if parent.field("operator").kind() == "delete" {
                    return true;
                }
            }
            ForInStatement => return parent.field("left") == node,
            CallExpression => {
                if parent.field("function") != node {
                    return false;
                }
            }
            SubscriptExpression => {
                if parent.field("index") == node {
                    return false;
                }
            }
            PairPattern => {
                if parent.field("key") == node {
                    return false;
                }
            }
            TernaryExpression => {
                if parent.field("condition") == node {
                    return false;
                }
            }
            _ => (),
        }

        node = parent;
        parent = return_default_if_none!(node.parent());
    }

    false
}

fn is_ignored_property_assignment(
    identifier_name: &str,
    ignored_property_assignments_for: &HashSet<String>,
    ignored_property_assignments_for_regex: &[Regex],
) -> bool {
    ignored_property_assignments_for.contains(identifier_name)
        || ignored_property_assignments_for_regex
            .into_iter()
            .any(|ignored| {
                // TODO: the ESLint version is creating new regex's each
                // time this function gets called which seems like a known
                // not good practice, upstream a fix?
                ignored.is_match(identifier_name)
            })
}

fn check_reference<'a, 'b>(
    reference: &Reference<'a, 'b>,
    index: usize,
    references: &[Reference<'a, 'b>],
    props: bool,
    ignored_property_assignments_for: &HashSet<String>,
    ignored_property_assignments_for_regex: &[Regex],
    context: &QueryMatchContext<'a, '_>,
) {
    let identifier = reference.identifier();

    if
    /* identifier && */
    reference.init() != Some(true)
        && match index {
            0 => true,
            index => references[index - 1].identifier() != identifier,
        }
    {
        if reference.is_write() {
            context.report(violation! {
                node => identifier,
                message_id => "assignment_to_function_param",
                data => {
                    name => identifier.text(context),
                }
            });
        } else if props
            && is_modifying_prop(reference)
            && !is_ignored_property_assignment(
                &identifier.text(context),
                ignored_property_assignments_for,
                ignored_property_assignments_for_regex,
            )
        {
            context.report(violation! {
                node => identifier,
                message_id => "assignment_to_function_param_prop",
                data => {
                    name => identifier.text(context),
                }
            });
        }
    }
}

pub fn no_param_reassign_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-param-reassign",
        languages => [Javascript],
        messages => [
            assignment_to_function_param => "Assignment to function parameter '{{name}}'.",
            assignment_to_function_param_prop => "Assignment to property of function parameter '{{name}}'.",
        ],
        options_type => Options,
        state => {
            [per-config]
            props: bool = options.props,
            ignored_property_assignments_for: HashSet<String> = options.ignore_property_modifications_for.iter().cloned().collect(),
            ignored_property_assignments_for_regex: Vec<Regex> = options.ignore_property_modifications_for_regex,
        },
        listeners => [
            r#"
              (function_declaration) @c
              (function) @c
              (arrow_function) @c
            "# => |node, context| {
                let scope_manager = context.retrieve::<ScopeManager<'a>>();

                scope_manager.get_declared_variables(node).for_each(|variable| {
                    if variable.defs().next().unwrap().type_() == VariableType::Parameter {
                        let references = variable.references().collect_vec();
                        references.iter().enumerate().for_each(|(index, reference)| {
                            check_reference(
                                reference,
                                index,
                                &references,
                                self.props,
                                &self.ignored_property_assignments_for,
                                &self.ignored_property_assignments_for_regex,
                                context,
                            );
                        });
                    }
                });
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::get_instance_provider_factory;

    #[test]
    fn test_no_param_reassign_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_param_reassign_rule(),
            rule_tests! {
                valid => [
                    "function foo(a) { var b = a; }",
                    "function foo(a) { for (b in a); }",
                    { code => "function foo(a) { for (b of a); }", environment => { ecma_version => 6 } },
                    "function foo(a) { a.prop = 'value'; }",
                    "function foo(a) { for (a.prop in obj); }",
                    { code => "function foo(a) { for (a.prop of arr); }", environment => { ecma_version => 6 } },
                    "function foo(a) { (function() { var a = 12; a++; })(); }",
                    "function foo() { someGlobal = 13; }",
                    { code => "function foo() { someGlobal = 13; }", environment => { globals => { someGlobal => false } } },
                    "function foo(a) { a.b = 0; }",
                    "function foo(a) { delete a.b; }",
                    "function foo(a) { ++a.b; }",
                    { code => "function foo(a) { [a.b] = []; }", environment => { ecma_version => 6 } },
                    { code => "function foo(a) { bar(a.b).c = 0; }", options => { props => true } },
                    { code => "function foo(a) { data[a.b] = 0; }", options => { props => true } },
                    { code => "function foo(a) { +a.b; }", options => { props => true } },
                    { code => "function foo(a) { (a ? [] : [])[0] = 1; }", options => { props => true } },
                    { code => "function foo(a) { (a.b ? [] : [])[0] = 1; }", options => { props => true } },
                    { code => "function foo(a) { a.b = 0; }", options => { props => true, ignore_property_modifications_for => ["a"] } },
                    { code => "function foo(a) { ++a.b; }", options => { props => true, ignore_property_modifications_for => ["a"] } },
                    { code => "function foo(a) { delete a.b; }", options => { props => true, ignore_property_modifications_for => ["a"] } },
                    { code => "function foo(a) { for (a.b in obj); }", options => { props => true, ignore_property_modifications_for => ["a"] } },
                    { code => "function foo(a) { for (a.b of arr); }", options => { props => true, ignore_property_modifications_for => ["a"] }, environment => { ecma_version => 6 } },
                    { code => "function foo(a, z) { a.b = 0; x.y = 0; }", options => { props => true, ignore_property_modifications_for => ["a", "x"] } },
                    { code => "function foo(a) { a.b.c = 0;}", options => { props => true, ignore_property_modifications_for => ["a"] } },
                    { code => "function foo(aFoo) { aFoo.b = 0; }", options => { props => true, ignore_property_modifications_for_regex => ["^a.*$"] } },
                    { code => "function foo(aFoo) { ++aFoo.b; }", options => { props => true, ignore_property_modifications_for_regex => ["^a.*$"] } },
                    { code => "function foo(aFoo) { delete aFoo.b; }", options => { props => true, ignore_property_modifications_for_regex => ["^a.*$"] } },
                    { code => "function foo(a, z) { aFoo.b = 0; x.y = 0; }", options => { props => true, ignore_property_modifications_for_regex => ["^a.*$", "^x.*$"] } },
                    { code => "function foo(aFoo) { aFoo.b.c = 0;}", options => { props => true, ignore_property_modifications_for_regex => ["^a.*$"] } },
                    {
                        code => "function foo(a) { ({ [a]: variable } = value) }",
                        options => { props => true },
                        environment => { ecma_version => 6 },
                    },
                    {
                        code => "function foo(a) { ([...a.b] = obj); }",
                        options => { props => false },
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "function foo(a) { ({...a.b} = obj); }",
                        options => { props => false },
                        environment => { ecma_version => 2018 }
                    },
                    {
                        code => "function foo(a) { for (obj[a.b] in obj); }",
                        options => { props => true }
                    },
                    {
                        code => "function foo(a) { for (obj[a.b] of arr); }",
                        options => { props => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "function foo(a) { for (bar in a.b); }",
                        options => { props => true }
                    },
                    {
                        code => "function foo(a) { for (bar of a.b); }",
                        options => { props => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "function foo(a) { for (bar in baz) a.b; }",
                        options => { props => true }
                    },
                    {
                        code => "function foo(a) { for (bar of baz) a.b; }",
                        options => { props => true },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "function foo(bar, baz) { bar.a = true; baz.b = false; }",
                        options => {
                            props => true,
                            ignore_property_modifications_for_regex => ["^(foo|bar)$"],
                            ignore_property_modifications_for => ["baz"]
                        }
                    }
                ],
                invalid => [
                    {
                        code => "function foo(bar) { bar = 13; }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { bar += 13; }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { (function() { bar = 13; })(); }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { ++bar; }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { bar++; }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { --bar; }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { bar--; }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo({bar}) { bar = 13; }",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo([, {bar}]) { bar = 13; }",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { ({bar} = {}); }",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { ({x: [, bar = 0]} = {}); }",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { for (bar in baz); }",
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { for (bar of baz); }",
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "bar" }
                        }]
                    },

                    {
                        code => "function foo(bar) { bar.a = 0; }",
                        options => { props => true },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { bar.get(0).a = 0; }",
                        options => { props => true },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { delete bar.a; }",
                        options => { props => true },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { ++bar.a; }",
                        options => { props => true },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { for (bar.a in {}); }",
                        options => { props => true },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { for (bar.a of []); }",
                        options => { props => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { (bar ? bar : [])[0] = 1; }",
                        options => { props => true },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { [bar.a] = []; }",
                        options => { props => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { [bar.a] = []; }",
                        options => { props => true, ignore_property_modifications_for => ["a"] },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { [bar.a] = []; }",
                        options => { props => true, ignore_property_modifications_for_regex => ["^a.*$"] },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { [bar.a] = []; }",
                        options => { props => true, ignore_property_modifications_for_regex => ["^B.*$"] },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(bar) { ({foo: bar.a} = {}); }",
                        options => { props => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "bar" }
                        }]
                    },
                    {
                        code => "function foo(a) { ({a} = obj); }",
                        options => { props => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => {
                                name => "a"
                            }
                        }]
                    },
                    {
                        code => "function foo(a) { ([...a] = obj); }",
                        environment => { ecma_version => 2015 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => {
                                name => "a"
                            }
                        }]
                    },
                    {
                        code => "function foo(a) { ({...a} = obj); }",
                        environment => { ecma_version => 2018 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => {
                                name => "a"
                            }
                        }]
                    },
                    {
                        code => "function foo(a) { ([...a.b] = obj); }",
                        options => { props => true },
                        environment => { ecma_version => 2015 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { ({...a.b} = obj); }",
                        options => { props => true },
                        environment => { ecma_version => 2018 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { for ({bar: a.b} in {}); }",
                        options => { props => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { for ([a.b] of []); }",
                        options => { props => true },
                        environment => { ecma_version => 6 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { a &&= b; }",
                        environment => { ecma_version => 2021 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { a ||= b; }",
                        environment => { ecma_version => 2021 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { a ??= b; }",
                        environment => { ecma_version => 2021 },
                        errors => [{
                            message_id => "assignment_to_function_param",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { a.b &&= c; }",
                        options => { props => true },
                        environment => { ecma_version => 2021 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "a" }
                        }],
                    },
                    {
                        code => "function foo(a) { a.b.c ||= d; }",
                        options => { props => true },
                        environment => { ecma_version => 2021 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "a" }
                        }]
                    },
                    {
                        code => "function foo(a) { a[b] ??= c; }",
                        options => { props => true },
                        environment => { ecma_version => 2021 },
                        errors => [{
                            message_id => "assignment_to_function_param_prop",
                            data => { name => "a" }
                        }]
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
