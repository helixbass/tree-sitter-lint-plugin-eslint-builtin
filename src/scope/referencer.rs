use std::cell::{Ref, RefMut};

use tree_sitter_lint::tree_sitter::{Node, TreeCursor};

use crate::visit::{visit, Visit};

use super::{
    definition::Definition,
    pattern_visitor::{is_pattern, PatternInfo, PatternVisitor},
    reference::ReadWriteFlags,
    scope::Scope,
    scope_manager::ScopeManager,
    variable::VariableType,
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

    fn current_scope(&self) -> Ref<Scope<'a>> {
        self.scope_manager.__current_scope()
    }

    fn maybe_current_scope(&self) -> Option<Ref<Scope<'a>>> {
        self.scope_manager.maybe_current_scope()
    }

    fn current_scope_mut(&self) -> RefMut<Scope<'a>> {
        self.scope_manager.__current_scope_mut()
    }

    fn close(&mut self, node: Node<'a>) {
        while matches!(
            self.maybe_current_scope(),
            Some(current_scope) if node == current_scope.block()
        ) {
            let closed = self.current_scope().__close(&self.scope_manager);
            self.scope_manager.__current_scope = closed;
        }
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
            cursor.reset(node.child_by_field_name("left").unwrap());
            self.visit_expression(cursor);
        }
        cursor.reset(node.child_by_field_name("right").unwrap());
        self.visit_expression(cursor);
    }

    fn visit_augmented_assignment_expression(&mut self, cursor: &mut TreeCursor<'tree>) {
        let node = cursor.node();
        if is_pattern(node) {
            self.current_scope_mut().__referencing(
                &mut self.scope_manager.arena.references.borrow_mut(),
                node.child_by_field_name("left").unwrap(),
                ReadWriteFlags::RW,
                node.child_by_field_name("right"),
                None,
                false,
                false,
            );
        } else {
            cursor.reset(node.child_by_field_name("left").unwrap());
            self.visit_expression(cursor);
        }
        cursor.reset(node.child_by_field_name("right").unwrap());
        self.visit_expression(cursor);
    }

    fn visit_catch_clause(&mut self, cursor: &mut TreeCursor<'tree>) {
        let node = cursor.node();
        self.scope_manager.__nest_catch_scope(node);

        if let Some(parameter) = node.child_by_field_name("parameter") {
            self.visit_pattern(
                parameter,
                Some(VisitPatternOptions {
                    process_right_hand_nodes: true,
                }),
                |this, pattern, info| {
                    let definitions_arena = &this.scope_manager.arena.definitions;
                    this.current_scope_mut().__define(
                        &mut this.scope_manager.__declared_variables.borrow_mut(),
                        &this.scope_manager.arena.variables,
                        definitions_arena,
                        this.scope_manager.source_text,
                        pattern,
                        Definition::new(
                            definitions_arena,
                            VariableType::CatchClause,
                            parameter,
                            node,
                            None,
                            None,
                            None,
                        ),
                    );
                    this.referencing_default_value(pattern, info.assignments, None, true);
                },
            );
        }

        cursor.reset(node.child_by_field_name("body").unwrap());
        self.visit_statement_block(cursor);

        self.close(node);
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
