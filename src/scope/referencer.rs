use tree_sitter_lint::tree_sitter::{Node, TreeCursor};

use crate::visit::{visit, Visit};

use super::{
    pattern_visitor::{is_pattern, PatternVisitor},
    scope_manager::ScopeManager,
};

fn traverse_identifier_in_pattern(
    // options,
    root_pattern: Node,
    referencer: Option<&mut Referencer>,
    callback: impl FnMut((), ()),
) {
    let mut visitor = PatternVisitor::new(
        // options,
        root_pattern,
        callback,
    );

    visit(&mut visitor, root_pattern);

    if let Some(referencer) = referencer {
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

    fn visit_pattern(
        &mut self,
        node: Node,
        options: Option<VisitPatternOptions>,
        callback: impl FnMut((), ()),
    ) {
        let options = options.unwrap_or_default();

        traverse_identifier_in_pattern(
            // this.options,
            node,
            options.process_right_hand_nodes.then_some(self),
            callback,
        );
    }
}

impl<'a, 'b> Visit<'a> for Referencer<'b> {
    fn visit_assignment_expression(&mut self, cursor: &mut TreeCursor<'a>) {
        let node = cursor.node();
        if is_pattern(node) {
            self.visit_pattern(
                node.child_by_field_name("left").unwrap(),
                Some(VisitPatternOptions {
                    process_right_hand_nodes: true,
                }),
                |pattern, info| {},
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
