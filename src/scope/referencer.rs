use tree_sitter_lint::tree_sitter::{Node, TreeCursor};

use crate::visit::{visit, Visit};

use super::{
    pattern_visitor::{is_pattern, PatternInfo, PatternVisitor},
    scope::Scope,
    scope_manager::ScopeManager,
};

fn traverse_identifier_in_pattern<'a>(
    // options,
    root_pattern: Node<'a>,
    referencer: &mut Referencer<'a>,
    should_visit_referencer: bool,
    mut callback: impl FnMut(&mut Referencer<'a>, Node<'a>, PatternInfo<'a>),
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

pub struct Referencer<'a> {
    scope_manager: &'a mut ScopeManager,
}

impl<'a> Referencer<'a> {
    pub fn new(scope_manager: &'a mut ScopeManager) -> Self {
        Self { scope_manager }
    }

    fn current_scope(&self) -> &Scope {
        self.scope_manager.__current_scope()
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
        callback: impl FnMut(&mut Referencer<'a>, Node<'a>, PatternInfo<'a>),
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

impl<'a: 'b, 'b> Visit<'a> for Referencer<'b> {
    fn visit_assignment_expression(&mut self, cursor: &mut TreeCursor<'a>) {
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
                },
            );
        } else {
        }
    }

    fn visit_augmented_assignment_expression(&mut self, cursor: &mut TreeCursor<'a>) {
        unimplemented!()
    }
}

#[derive(Default)]
struct VisitPatternOptions {
    process_right_hand_nodes: bool,
}

#[derive(Copy, Clone)]
struct PatternAndNode<'a> {
    pattern: Node<'a>,
    node: Node<'a>,
}
