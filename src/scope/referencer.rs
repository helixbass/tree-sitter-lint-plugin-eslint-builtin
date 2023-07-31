use std::cell::{Ref, RefMut};

use tree_sitter_lint::tree_sitter::Node;

use crate::{
    kind::{ComputedPropertyName, Identifier, LexicalDeclaration},
    text::SourceTextProvider,
    visit::{
        visit_call_expression, visit_class_static_block, visit_expression, visit_expressions,
        visit_for_statement, visit_program, visit_update_expression, Visit,
    },
};

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

    visitor.visit(root_pattern);

    if should_visit_referencer {
        visitor
            .right_hand_nodes
            .iter()
            .for_each(|&right_hand_node| {
                referencer.visit(right_hand_node);
            });
    }
}

pub struct Referencer<'a, 'b> {
    scope_manager: &'b mut ScopeManager<'a>,
    is_inner_method_definition: bool,
}

impl<'a, 'b> Referencer<'a, 'b> {
    pub fn new(scope_manager: &'b mut ScopeManager<'a>) -> Self {
        Self {
            scope_manager,
            is_inner_method_definition: Default::default(),
        }
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

    fn push_inner_method_definition(&mut self, is_inner_method_definition: bool) -> bool {
        let previous = self.is_inner_method_definition;

        self.is_inner_method_definition = is_inner_method_definition;
        previous
    }

    fn pop_inner_method_definition(&mut self, is_inner_method_definition: bool) {
        self.is_inner_method_definition = is_inner_method_definition;
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

    fn _visit_class(&mut self, node: Node) {
        unimplemented!()
    }
}

impl<'tree: 'referencer, 'referencer, 'b> Visit<'tree> for Referencer<'referencer, 'b> {
    fn visit_assignment_expression(&mut self, node: Node<'tree>) {
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
                        Some(ReadWriteFlags::WRITE),
                        node.child_by_field_name("right"),
                        maybe_implicit_global,
                        Some(!info.top_level),
                        Some(false),
                    );
                },
            );
        } else {
            self.visit_expression(node.child_by_field_name("left").unwrap());
        }
        self.visit_expression(node.child_by_field_name("right").unwrap());
    }

    fn visit_augmented_assignment_expression(&mut self, node: Node<'tree>) {
        if is_pattern(node) {
            self.current_scope_mut().__referencing(
                &mut self.scope_manager.arena.references.borrow_mut(),
                node.child_by_field_name("left").unwrap(),
                Some(ReadWriteFlags::RW),
                node.child_by_field_name("right"),
                None,
                None,
                None,
            );
        } else {
            self.visit_expression(node.child_by_field_name("left").unwrap());
        }
        self.visit_expression(node.child_by_field_name("right").unwrap());
    }

    fn visit_catch_clause(&mut self, node: Node<'tree>) {
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
                        &*this,
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

        self.visit_statement_block(node.child_by_field_name("body").unwrap());

        self.close(node);
    }

    fn visit_program(&mut self, node: Node<'tree>) {
        self.scope_manager.__nest_global_scope(node);

        if self.scope_manager.is_global_return() {
            self.current_scope_mut().set_is_strict(false);
            self.scope_manager.__nest_function_scope(node, false);
        }

        if self.scope_manager.__is_es6() && self.scope_manager.is_module() {
            self.scope_manager.__nest_module_scope(node);
        }

        if self.scope_manager.is_strict_mode_supported() && self.scope_manager.is_implied_strict() {
            self.current_scope_mut().set_is_strict(true);
        }

        visit_program(self, node);
        self.close(node);
    }

    fn visit_identifier(&mut self, node: Node<'tree>) {
        self.current_scope_mut().__referencing(
            &mut self.scope_manager.arena.references.borrow_mut(),
            node,
            None,
            None,
            None,
            None,
            None,
        );
    }

    fn visit_private_property_identifier(&mut self, _node: Node<'tree>) {}

    fn visit_update_expression(&mut self, node: Node<'tree>) {
        let argument = node.child_by_field_name("argument").unwrap();
        if is_pattern(argument) {
            self.current_scope_mut().__referencing(
                &mut self.scope_manager.arena.references.borrow_mut(),
                argument,
                Some(ReadWriteFlags::RW),
                None,
                None,
                None,
                None,
            );
        } else {
            visit_update_expression(self, node);
        }
    }

    fn visit_member_expression(&mut self, node: Node<'tree>) {
        visit_expression(self, node.child_by_field_name("object").unwrap());
    }

    fn visit_subscript_expression(&mut self, node: Node<'tree>) {
        visit_expression(self, node.child_by_field_name("object").unwrap());
        visit_expressions(self, node.child_by_field_name("index").unwrap());
    }

    fn visit_pair(&mut self, node: Node<'tree>) {
        let key = node.child_by_field_name("key").unwrap();
        if key.kind() == ComputedPropertyName {
            self.visit_computed_property_name(key);
        }
        self.visit_expression(node.child_by_field_name("value").unwrap());
    }

    fn visit_field_definition(&mut self, node: Node<'tree>) {
        let property = node.child_by_field_name("property").unwrap();
        if property.kind() == ComputedPropertyName {
            self.visit_computed_property_name(property);
        }
        if let Some(value) = node.child_by_field_name("value") {
            self.scope_manager
                .__nest_class_field_initializer_scope(value);
            self.visit_expression(value);
            self.close(value);
        }
    }

    fn visit_class_static_block(&mut self, node: Node<'tree>) {
        self.scope_manager.__nest_class_static_block_scope(node);

        visit_class_static_block(self, node);

        self.close(node);
    }

    fn visit_method_definition(&mut self, node: Node<'tree>) {
        let key = node.child_by_field_name("name").unwrap();
        if key.kind() == ComputedPropertyName {
            self.visit_computed_property_name(key);
        }
        let previous = self.push_inner_method_definition(true);
        self.visit_formal_parameters(node.child_by_field_name("parameters").unwrap());
        self.visit_statement_block(node.child_by_field_name("body").unwrap());
        self.pop_inner_method_definition(previous);
    }

    fn visit_break_statement(&mut self, _node: Node<'tree>) {}

    fn visit_continue_statement(&mut self, _node: Node<'tree>) {}

    fn visit_labeled_statement(&mut self, node: Node<'tree>) {
        self.visit_statement(node.child_by_field_name("body").unwrap());
    }

    fn visit_for_statement(&mut self, node: Node<'tree>) {
        let initializer = node.child_by_field_name("initializer").unwrap();
        if initializer.kind() == LexicalDeclaration {
            self.scope_manager.__nest_for_scope(node);
        }

        visit_for_statement(self, node);

        self.close(node);
    }

    fn visit_class(&mut self, node: Node<'tree>) {
        self._visit_class(node);
    }

    fn visit_class_declaration(&mut self, node: Node<'tree>) {
        self._visit_class(node);
    }

    fn visit_call_expression(&mut self, node: Node<'tree>) {
        let callee = node.child_by_field_name("function").unwrap();
        if !self.scope_manager.__ignore_eval()
            && callee.kind() == Identifier
            && self.get_node_text(callee) == "eval"
        {
            let variable_scope = self.current_scope().variable_scope();
            Scope::__detect_eval(
                variable_scope,
                &mut self.scope_manager.arena.scopes.borrow_mut(),
            );
        }
        visit_call_expression(self, node);
    }

    fn visit_for_in_statement(&mut self, node: Node<'tree>) {
        let left = node.child_by_field_name("left").unwrap();
        let kind = node.child_by_field_name("kind");
        if matches!(
            kind,
            Some(kind) if ["let", "const"].contains(&self.get_node_text(kind))
        ) {
            self.scope_manager.__nest_for_scope(node);
        }
        // if kind.is_some() {
        // } else {
        // }
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

impl<'a> SourceTextProvider<'a> for Referencer<'a, '_> {
    fn get_node_text(&self, node: Node) -> &'a str {
        self.scope_manager.get_node_text(node)
    }
}
