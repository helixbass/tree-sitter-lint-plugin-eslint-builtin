use std::collections::HashMap;

use squalid::continue_if_none;
use tree_sitter_lint::{tree_sitter::Node, NodeExt};

use crate::{
    ast_helpers::{
        get_last_expression_of_sequence_expression, is_chain_expression, is_logical_expression,
    },
    kind::{BinaryExpression, SequenceExpression, TernaryExpression},
    scope::{Scope, Variable},
};

fn is_modified_global(variable: &Variable) -> bool {
    variable.defs().next().is_some() || variable.references().any(|r| r.is_write())
}

fn is_pass_through(node: Node) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };

    match parent.kind() {
        TernaryExpression => {
            parent.field("consequence") == node || parent.field("alternative") == node
        }
        BinaryExpression => {
            if is_logical_expression(parent) {
                return true;
            }
            false
        }
        SequenceExpression => get_last_expression_of_sequence_expression(parent) == node,
        _ => is_chain_expression(parent),
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum StrictOrLegacy {
    #[default]
    Strict,
    Legacy,
}

pub struct ReferenceTrackerOptions {
    pub mode: Option<StrictOrLegacy>,
    pub global_object_names: Option<Vec<String>>,
}

pub type TraceMap = HashMap<String, TraceMapOrReferenceKinds>;

pub enum TraceMapOrReferenceKinds {
    TraceMap(TraceMap),
    ReferenceKinds(ReferenceKinds),
}

pub type ReferenceKinds = HashMap<ReferenceKind, bool>;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    READ,
    CALL,
    CONSTRUCT,
}

pub struct TrackedReference<'a> {
    node: Node<'a>,
    path: Vec<String>,
    type_: ReferenceKind,
}

pub struct ReferenceTracker<'a, 'b> {
    variable_stack: Vec<Variable<'a, 'b>>,
    global_scope: Scope<'a, 'b>,
    mode: StrictOrLegacy,
    global_object_names: Vec<String>,
}

impl<'a, 'b> ReferenceTracker<'a, 'b> {
    pub fn new(global_scope: Scope<'a, 'b>, options: Option<ReferenceTrackerOptions>) -> Self {
        let mode = options
            .as_ref()
            .and_then(|options| options.mode)
            .unwrap_or_default();
        let global_object_names = options
            .and_then(|options| options.global_object_names)
            .unwrap_or_else(|| {
                vec![
                    "global".to_owned(),
                    "globalThis".to_owned(),
                    "self".to_owned(),
                    "window".to_owned(),
                ]
            });
        Self {
            variable_stack: Default::default(),
            global_scope,
            mode,
            global_object_names,
        }
    }

    pub fn iterate_global_references(&mut self, trace_map: &TraceMap) -> Vec<TrackedReference<'a>> {
        let mut ret: Vec<TrackedReference<'a>> = Default::default();

        for (key, next_trace_map) in trace_map {
            let path = vec![key.clone()];
            let set = self.global_scope.set();
            let variable = continue_if_none!(set.get(&**key));

            if is_modified_global(variable) {
                continue;
            }

            self._iterate_variable_references(&mut ret, variable, &path, next_trace_map, true)
        }

        ret
    }

    fn _iterate_variable_references(
        &mut self,
        ret: &mut Vec<TrackedReference<'a>>,
        variable: &Variable<'a, 'b>,
        path: &[String],
        trace_map: &TraceMapOrReferenceKinds,
        should_report: bool,
    ) {
        if self.variable_stack.contains(variable) {
            return;
        }
        self.variable_stack.push(variable.clone());
        // try {
        for reference in variable.references() {
            if !reference.is_read() {
                continue;
            }
            let node = reference.identifier();

            if should_report
                && matches!(
                    trace_map,
                    TraceMapOrReferenceKinds::ReferenceKinds(trace_map) if trace_map.get(&ReferenceKind::READ).copied() == Some(true)
                )
            {
                ret.push(TrackedReference {
                    node,
                    path: path.to_owned(),
                    type_: ReferenceKind::READ,
                    // info: traceMap[READ]
                });
            }
            self._iterate_property_references(ret, node, &path, trace_map);
        }
        // } finally {
        self.variable_stack.pop();
        // }
    }

    fn _iterate_property_references(
        &mut self,
        ret: &mut Vec<TrackedReference<'a>>,
        root_node: Node<'a>,
        path: &[String],
        trace_map: &TraceMapOrReferenceKinds,
    ) {
        let mut node = root_node;
        while is_pass_through(node) {
            node = node.parent().unwrap();
        }

        let parent = node.parent().unwrap();
        match parent.kind() {
            MemberExpression | SubscriptExpression => {
                if parent.field("object") == node {
                    let key = get_property_name();
                }
            }
            _ => ()
        }
    }
}
