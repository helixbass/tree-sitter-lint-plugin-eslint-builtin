use std::cell::{Ref, RefMut};

use tree_sitter_lint::tree_sitter::{Node, TreeCursor};

use crate::visit::{visit, Visit};

use super::{
    pattern_visitor::{is_pattern, PatternInfo, PatternVisitor},
    reference::ReadWriteFlags,
    scope::Scope,
    scope_manager::ScopeManager,
};

fn traverse_identifier_in_pattern<'a, 'b>(
    // options,
    root_pattern: Node<'a>,
    referencer: &mut Referencer<'a, 'b>,
    should_visit_referencer: bool,
    mut callback: impl FnMut(&mut Referencer<'a, 'b>, Node<'a>, PatternInfo<'a>),
) {
    let mut visitor = PatternVisitor::new(
        // options,
        root_pattern,
        |node, pattern_info| callback(referencer, node, pattern_info),
    );

    visit(&mut visitor, root_pattern);

    if should_visit_referencer {
        visitor
            .right_hand_nodes
            .iter()
            .for_each(|&right_hand_node| {
                visit(referencer, right_hand_node);
            });
    }
}

pub struct Referencer<'a, 'b> {
    scope_manager: &'b mut ScopeManager<'a>,
}

impl<'a, 'b> Referencer<'a, 'b> {
    pub fn new(scope_manager: &'b mut ScopeManager<'a>) -> Self {
        Self { scope_manager }
    }

    fn current_scope(&self) -> Ref<Scope> {
        self.scope_manager.__current_scope()
    }

    fn current_scope_mut(&self) -> RefMut<Scope> {
        self.scope_manager.__current_scope_mut()
    }

    fn referencing_default_value(
        &self,
        pattern: Node,
        assignments: &[Node],
        maybe_implicit_global: Option<PatternAndNode>,
        init: bool,
    ) {
        unimplemented!();
    }

    fn visit_pattern(
        &mut self,
        node: Node<'a>,
        options: Option<VisitPatternOptions>,
        callback: impl FnMut(&mut Referencer<'a, 'b>, Node<'a>, PatternInfo<'a>),
    ) {
        let options = options.unwrap_or_default();

        traverse_identifier_in_pattern(
            // this.options,
            node,
            self,
            options.process_right_hand_nodes,
            callback,
        );
    }
}

impl<'tree: 'referencer, 'referencer, 'b> Visit<'tree> for Referencer<'referencer, 'b> {
    fn visit_assignment_expression(&mut self, cursor: &mut TreeCursor<'tree>) {
        let node = cursor.node();
        if is_pattern(node) {
            self.visit_pattern(
                node.child_by_field_name("left").unwrap(),
                Some(VisitPatternOptions {
                    process_right_hand_nodes: true,
                }),
                |referencer, pattern, info| {
                    let mut maybe_implicit_global: Option<PatternAndNode> = Default::default();

                    if !referencer.current_scope().is_strict() {
                        maybe_implicit_global = Some(PatternAndNode { pattern, node });
                    }
                    referencer.referencing_default_value(
                        pattern,
                        info.assignments,
                        maybe_implicit_global,
                        false,
                    );
                    referencer.current_scope_mut().__referencing(
                        &mut referencer.scope_manager.arena.references.borrow_mut(),
                        pattern,
                        ReadWriteFlags::WRITE,
                        node.child_by_field_name("right"),
                        maybe_implicit_global,
                        !info.top_level,
                        false,
                    );
                },
            );
        } else {
        }
    }

    fn visit_augmented_assignment_expression(&mut self, cursor: &mut TreeCursor<'tree>) {
        unimplemented!()
    }
}

#[derive(Default)]
struct VisitPatternOptions {
    process_right_hand_nodes: bool,
}

#[derive(Copy, Clone)]
pub struct PatternAndNode<'a> {
    pattern: Node<'a>,
    node: Node<'a>,
}
