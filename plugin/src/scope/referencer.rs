use std::{
    borrow::Cow,
    cell::{Ref, RefMut},
    ops,
};

use id_arena::Id;
use squalid::OptionExt;
use tracing::{trace, trace_span};
use tree_sitter_lint::{
    tree_sitter::Node, NodeExt, SourceTextProvider,
};

use super::{
    definition::Definition,
    pattern_visitor::{is_pattern, PatternInfo, PatternVisitor},
    reference::ReadWriteFlags,
    scope::_Scope,
    scope_manager::{ScopeManager, ScopeManagerOptions},
    variable::VariableType,
};
use crate::{
    ast_helpers::{get_first_child_of_kind, get_function_params},
    kind::{
        ClassDeclaration, ClassHeritage, ComputedPropertyName, ExportClause, Function,
        FunctionDeclaration, Identifier, ImportClause, LexicalDeclaration, StatementBlock,
        SwitchCase, SwitchDefault, VariableDeclaration, VariableDeclarator,
    },
    visit::{visit_children, Visit},
};

fn traverse_identifier_in_pattern<'a, 'b>(
    options: ScopeManagerOptions,
    root_pattern: Node<'a>,
    referencer: &mut Referencer<'a, 'b>,
    should_visit_referencer: bool,
    mut callback: impl FnMut(&mut Referencer<'a, 'b>, Node<'a>, PatternInfo<'a, '_>),
) {
    let mut visitor = PatternVisitor::new(options, root_pattern, |node, pattern_info| {
        callback(referencer, node, pattern_info)
    });

    let _span = trace_span!("pattern visitor").entered();

    visitor.visit(root_pattern);

    drop(_span);

    if should_visit_referencer {
        let _span = trace_span!("visiting right-hand nodes").entered();

        visitor
            .right_hand_nodes
            .iter()
            .for_each(|&right_hand_node| {
                referencer.visit(right_hand_node);
            });
    }
}

struct Importer<'a, 'b, 'c> {
    declaration: Node<'a>,
    referencer: &'c mut Referencer<'a, 'b>,
}

impl<'a, 'b, 'c> Importer<'a, 'b, 'c> {
    pub fn new(declaration: Node<'a>, referencer: &'c mut Referencer<'a, 'b>) -> Self {
        Self {
            declaration,
            referencer,
        }
    }

    fn visit_import(&mut self, id: Node<'a>, specifier: Node<'a>) {
        self.referencer
            .visit_pattern(id, None, |referencer, pattern, _| {
                let definitions_arena = &referencer.scope_manager.arena.definitions;
                referencer.current_scope_mut().__define(
                    &mut referencer.scope_manager.__declared_variables.borrow_mut(),
                    &referencer.scope_manager.arena.variables,
                    definitions_arena,
                    &*referencer,
                    pattern,
                    Definition::new(
                        definitions_arena,
                        VariableType::ImportBinding,
                        pattern,
                        specifier,
                        Some(self.declaration),
                        None,
                        None,
                    ),
                );
            });
    }
}

impl<'tree: 'a, 'a, 'b, 'c> Visit<'tree> for Importer<'a, 'b, 'c> {
    fn visit_namespace_import(&mut self, node: Node<'tree>) {
        self.visit_import(get_first_child_of_kind(node, Identifier), node);
    }

    fn visit_identifier(&mut self, node: Node<'tree>) {
        if node.parent().unwrap().kind() != ImportClause {
            return;
        }

        self.visit_import(node, node);
    }

    fn visit_import_specifier(&mut self, node: Node<'tree>) {
        if let Some(alias) = node.child_by_field_name("alias") {
            self.visit_import(alias, node);
        } else {
            self.visit_import(node.child_by_field_name("name").unwrap(), node);
        }
    }
}

pub struct Referencer<'a, 'b> {
    options: ScopeManagerOptions,
    scope_manager: &'b mut ScopeManager<'a>,
    is_inner_method_definition: bool,
}

impl<'a, 'b> Referencer<'a, 'b> {
    pub fn new(options: ScopeManagerOptions, scope_manager: &'b mut ScopeManager<'a>) -> Self {
        Self {
            options,
            scope_manager,
            is_inner_method_definition: Default::default(),
        }
    }

    fn current_scope(&self) -> Ref<_Scope<'a>> {
        self.scope_manager.__current_scope()
    }

    fn maybe_current_scope(&self) -> Option<Ref<_Scope<'a>>> {
        self.scope_manager.maybe_current_scope()
    }

    fn current_scope_mut(&self) -> RefMut<_Scope<'a>> {
        self.scope_manager.__current_scope_mut()
    }

    fn close(&mut self, node: Node<'a>) {
        while matches!(
            self.maybe_current_scope(),
            Some(current_scope) if node == current_scope.block()
        ) {
            trace!(id = ?self.maybe_current_scope().unwrap(), "closing scope");

            let closed = _Scope::__close(
                self.scope_manager.__current_scope.unwrap(),
                self.scope_manager,
            );
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
        pattern: Node<'a>,
        assignments: &[Node<'a>],
        maybe_implicit_global: Option<PatternAndNode<'a>>,
        init: bool,
    ) {
        let mut scope = self.current_scope_mut();

        assignments.into_iter().for_each(|assignment| {
            scope.__referencing(
                &mut self.scope_manager.arena.references.borrow_mut(),
                pattern,
                Some(ReadWriteFlags::WRITE),
                assignment.child_by_field_name("right"),
                maybe_implicit_global,
                Some(pattern != assignment.field("left")),
                Some(init),
            );
        });
    }

    fn visit_pattern(
        &mut self,
        node: Node<'a>,
        options: Option<VisitPatternOptions>,
        callback: impl FnMut(&mut Referencer<'a, 'b>, Node<'a>, PatternInfo<'a, '_>),
    ) {
        let options = options.unwrap_or_default();

        traverse_identifier_in_pattern(
            self.options,
            node,
            self,
            options.process_right_hand_nodes,
            callback,
        );
    }

    fn _visit_function(&mut self, node: Node<'a>) {
        if node.kind() == FunctionDeclaration {
            let definitions_arena = &self.scope_manager.arena.definitions;
            self.current_scope_mut().__define(
                &mut self.scope_manager.__declared_variables.borrow_mut(),
                &self.scope_manager.arena.variables,
                definitions_arena,
                &*self,
                node.field("name"),
                Definition::new(
                    definitions_arena,
                    VariableType::FunctionName,
                    node.field("name"),
                    node,
                    None,
                    None,
                    None,
                ),
            );
        }

        if node.kind() == Function && node.child_by_field_name("name").is_some() {
            self.scope_manager
                .__nest_function_expression_name_scope(node);
        }

        self.scope_manager
            .__nest_function_scope(node, self.is_inner_method_definition);

        for (param_index, param) in get_function_params(node).enumerate()
        {
            self.visit_pattern(
                param,
                Some(VisitPatternOptions {
                    process_right_hand_nodes: true,
                }),
                |this, pattern: Node<'a>, info: PatternInfo<'a, '_>| {
                    let definitions_arena = &this.scope_manager.arena.definitions;
                    this.current_scope_mut().__define(
                        &mut this.scope_manager.__declared_variables.borrow_mut(),
                        &this.scope_manager.arena.variables,
                        definitions_arena,
                        &*this,
                        pattern,
                        Definition::new_parameter(
                            definitions_arena,
                            pattern,
                            node,
                            Some(param_index),
                            info.rest,
                        ),
                    );

                    this.referencing_default_value(pattern, info.assignments, None, true);
                },
            );
        }

        let body = node.field("body");
        match body.kind() {
            StatementBlock => visit_children(self, body),
            _ => self.visit(body),
        }

        self.close(node);
    }

    fn _visit_class(&mut self, node: Node<'a>) {
        if node.kind() == ClassDeclaration {
            let definitions_arena = &self.scope_manager.arena.definitions;
            self.current_scope_mut().__define(
                &mut self.scope_manager.__declared_variables.borrow_mut(),
                &self.scope_manager.arena.variables,
                definitions_arena,
                &*self,
                node.field("name"),
                Definition::new(
                    definitions_arena,
                    VariableType::ClassName,
                    node.field("name"),
                    node,
                    None,
                    None,
                    None,
                ),
            );
        }

        if let Some(super_class) = node.maybe_first_child_of_kind(ClassHeritage) {
            self.visit(super_class);
        }

        self.scope_manager.__nest_class_scope(node);

        if let Some(name) = node.child_by_field_name("name") {
            let definitions_arena = &self.scope_manager.arena.definitions;
            self.current_scope_mut().__define(
                &mut self.scope_manager.__declared_variables.borrow_mut(),
                &self.scope_manager.arena.variables,
                definitions_arena,
                &*self,
                name,
                Definition::new(
                    definitions_arena,
                    VariableType::ClassName,
                    name,
                    node,
                    None,
                    None,
                    None,
                ),
            );
        }

        self.visit(node.field("body"));

        self.close(node);
    }

    #[allow(clippy::too_many_arguments)]
    fn _visit_variable_declaration(
        &mut self,
        variable_target_scope: Id<_Scope<'a>>,
        type_: VariableType,
        node: Node<'a>,
        index: usize,
        decl: Node<'a>,
        decl_name: Node<'a>,
        init: Option<Node<'a>>,
        node_kind: &str,
    ) {
        self.visit_pattern(
            decl_name,
            Some(VisitPatternOptions {
                process_right_hand_nodes: true,
            }),
            |this, pattern, info| {
                let definitions_arena = &this.scope_manager.arena.definitions;
                this.scope_manager.arena.scopes.borrow_mut()[variable_target_scope].__define(
                    &mut this.scope_manager.__declared_variables.borrow_mut(),
                    &this.scope_manager.arena.variables,
                    definitions_arena,
                    &*this,
                    pattern,
                    Definition::new(
                        definitions_arena,
                        type_,
                        pattern,
                        decl,
                        Some(node),
                        Some(index),
                        Some(node_kind.to_owned()),
                    ),
                );

                this.referencing_default_value(pattern, info.assignments, None, true);
                if let Some(init) = init {
                    this.current_scope_mut().__referencing(
                        &mut this.scope_manager.arena.references.borrow_mut(),
                        pattern,
                        Some(ReadWriteFlags::WRITE),
                        Some(init),
                        None,
                        Some(!info.top_level),
                        Some(true),
                    );
                }
            },
        );
    }

    fn visit_variable_or_lexical_declaration<'tree: 'a>(&mut self, node: Node<'tree>) {
        let variable_target_scope = if node.kind() == VariableDeclaration {
            self.current_scope().variable_scope()
        } else {
            self.current_scope().id()
        };

        let mut cursor = node.walk();
        for (i, decl) in node
            .named_children(&mut cursor)
            .filter(|child| child.kind() == VariableDeclarator)
            .enumerate()
        {
            self._visit_variable_declaration(
                variable_target_scope,
                VariableType::Variable,
                node,
                i,
                decl,
                decl.field("name"),
                decl.child_by_field_name("value"),
                match node.kind() {
                    VariableDeclaration => "var",
                    LexicalDeclaration => node.field("kind").kind(),
                    _ => unreachable!(),
                },
            );

            if let Some(init) = decl.child_by_field_name("value") {
                self.visit(init);
            }
        }
    }

    fn visit_for_in_left_declaration(&mut self, node: Node<'a>, kind: &str) {
        let variable_target_scope = if kind == "var" {
            self.current_scope().variable_scope()
        } else {
            self.current_scope().id()
        };

        self._visit_variable_declaration(
            variable_target_scope,
            VariableType::Variable,
            node,
            0,
            node,
            node,
            None,
            kind,
        );
    }
}

impl<'tree: 'a, 'a, 'b> Visit<'tree> for Referencer<'a, 'b> {
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
            self.visit(node.child_by_field_name("left").unwrap());
        }
        self.visit(node.child_by_field_name("right").unwrap());
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
            self.visit(node.child_by_field_name("left").unwrap());
        }
        self.visit(node.child_by_field_name("right").unwrap());
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

        self.visit(node.child_by_field_name("body").unwrap());

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

        visit_children(self, node);
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
            visit_children(self, node);
        }
    }

    fn visit_member_expression(&mut self, node: Node<'tree>) {
        self.visit(node.child_by_field_name("object").unwrap());
    }

    fn visit_subscript_expression(&mut self, node: Node<'tree>) {
        self.visit(node.child_by_field_name("object").unwrap());
        self.visit(node.child_by_field_name("index").unwrap());
    }

    fn visit_pair(&mut self, node: Node<'tree>) {
        let key = node.child_by_field_name("key").unwrap();
        if key.kind() == ComputedPropertyName {
            self.visit(key);
        }
        self.visit(node.child_by_field_name("value").unwrap());
    }

    fn visit_field_definition(&mut self, node: Node<'tree>) {
        let property = node.child_by_field_name("property").unwrap();
        if property.kind() == ComputedPropertyName {
            self.visit(property);
        }
        if let Some(value) = node.child_by_field_name("value") {
            self.scope_manager
                .__nest_class_field_initializer_scope(value);
            self.visit(value);
            self.close(value);
        }
    }

    fn visit_class_static_block(&mut self, node: Node<'tree>) {
        self.scope_manager.__nest_class_static_block_scope(node);

        visit_children(self, node.field("body"));

        self.close(node);
    }

    fn visit_method_definition(&mut self, node: Node<'tree>) {
        let key = node.child_by_field_name("name").unwrap();
        if key.kind() == ComputedPropertyName {
            self.visit(key);
        }
        let previous = self.push_inner_method_definition(true);
        self.visit(node.child_by_field_name("parameters").unwrap());
        self.visit(node.child_by_field_name("body").unwrap());
        self.pop_inner_method_definition(previous);
    }

    fn visit_break_statement(&mut self, _node: Node<'tree>) {}

    fn visit_continue_statement(&mut self, _node: Node<'tree>) {}

    fn visit_labeled_statement(&mut self, node: Node<'tree>) {
        self.visit(node.child_by_field_name("body").unwrap());
    }

    fn visit_for_statement(&mut self, node: Node<'tree>) {
        let initializer = node.child_by_field_name("initializer").unwrap();
        if initializer.kind() == LexicalDeclaration {
            self.scope_manager.__nest_for_scope(node);
        }

        visit_children(self, node);

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
            && self.node_text(callee) == "eval"
        {
            let variable_scope = self.current_scope().variable_scope();
            _Scope::__detect_eval(
                variable_scope,
                &mut self.scope_manager.arena.scopes.borrow_mut(),
            );
        }
        visit_children(self, node);
    }

    fn visit_statement_block(&mut self, node: Node<'tree>) {
        if self.scope_manager.__is_es6() {
            self.scope_manager.__nest_block_scope(node);
        }

        visit_children(self, node);

        self.close(node);
    }

    fn visit_this(&mut self, _node: Node<'tree>) {
        let variable_scope = self.current_scope().variable_scope();
        self.scope_manager
            .arena
            .scopes
            .borrow_mut()
            .get_mut(variable_scope)
            .unwrap()
            .__detect_this();
    }

    fn visit_with_statement(&mut self, node: Node<'tree>) {
        self.visit(node.child_by_field_name("object").unwrap());

        self.scope_manager.__nest_with_scope(node);

        self.visit(node.child_by_field_name("body").unwrap());

        self.close(node);
    }

    fn visit_variable_declaration(&mut self, node: Node<'tree>) {
        self.visit_variable_or_lexical_declaration(node);
    }

    fn visit_lexical_declaration(&mut self, node: Node<'tree>) {
        self.visit_variable_or_lexical_declaration(node);
    }

    fn visit_switch_statement(&mut self, node: Node<'tree>) {
        self.visit(node.child_by_field_name("value").unwrap());

        if self.scope_manager.__is_es6() {
            self.scope_manager.__nest_switch_scope(node);
        }

        let mut cursor = node.walk();
        for case in node
            .child_by_field_name("body")
            .unwrap()
            .named_children(&mut cursor)
            .filter(|child| matches!(child.kind(), SwitchCase | SwitchDefault))
        {
            self.visit(case);
        }

        self.close(node);
    }

    fn visit_function_declaration(&mut self, node: Node<'tree>) {
        self._visit_function(node);
    }

    fn visit_function(&mut self, node: Node<'tree>) {
        self._visit_function(node);
    }

    fn visit_for_in_statement(&mut self, node: Node<'tree>) {
        let left = node.field("left");
        let kind = node.child_by_field_name("kind");
        if kind.matches(|kind| ["let", "const"].contains(&kind.kind())) {
            self.scope_manager.__nest_for_scope(node);
        }
        if let Some(kind) = kind {
            self.visit_for_in_left_declaration(left, kind.kind());
            self.visit_pattern(left, None, |this, pattern, _| {
                this.current_scope_mut().__referencing(
                    &mut this.scope_manager.arena.references.borrow_mut(),
                    pattern,
                    Some(ReadWriteFlags::WRITE),
                    node.child_by_field_name("right"),
                    None,
                    Some(true),
                    Some(true),
                );
            });
        } else {
            self.visit_pattern(
                left,
                Some(VisitPatternOptions {
                    process_right_hand_nodes: true,
                }),
                |this, pattern, info| {
                    let maybe_implicit_global = (!this.current_scope().is_strict())
                        .then_some(PatternAndNode { pattern, node });
                    this.referencing_default_value(
                        pattern,
                        info.assignments,
                        maybe_implicit_global,
                        false,
                    );
                    this.current_scope_mut().__referencing(
                        &mut this.scope_manager.arena.references.borrow_mut(),
                        pattern,
                        Some(ReadWriteFlags::WRITE),
                        node.child_by_field_name("right"),
                        maybe_implicit_global,
                        Some(true),
                        Some(false),
                    );
                },
            );
        }
        self.visit(node.field("right"));
        self.visit(node.field("body"));

        self.close(node);
    }

    fn visit_arrow_function(&mut self, node: Node<'tree>) {
        self._visit_function(node);
    }

    fn visit_import_statement(&mut self, node: Node<'tree>) {
        assert!(
            self.scope_manager.__is_es6() && self.scope_manager.is_module(),
            "import_statement should appear when the mode is ES6 and in the module context.",
        );

        let mut importer = Importer::new(node, self);

        importer.visit(node);
    }

    fn visit_export_statement(&mut self, node: Node<'tree>) {
        if node.child_by_field_name("source").is_some() {
            return;
        }
        if let Some(declaration) = node.child_by_field_name("declaration") {
            self.visit(declaration);
        } else if let Some(value) = node.child_by_field_name("value") {
            self.visit(value);
        }
        let mut cursor = node.walk();
        for export_clause in node
            .named_children(&mut cursor)
            .filter(|child| child.kind() == ExportClause)
        {
            self.visit(export_clause);
        }
    }

    fn visit_export_specifier(&mut self, node: Node<'tree>) {
        let name = node
            .child_by_field_name("alias")
            .unwrap_or_else(|| node.child_by_field_name("name").unwrap());
        if name.kind() == Identifier {
            self.visit(name);
        }
    }

    fn visit_meta_property(&mut self, _node: Node<'tree>) {}
}

#[derive(Default)]
struct VisitPatternOptions {
    process_right_hand_nodes: bool,
}

#[derive(Copy, Clone, Debug)]
pub struct PatternAndNode<'a> {
    pub pattern: Node<'a>,
    pub node: Node<'a>,
}

impl<'a> SourceTextProvider<'a> for Referencer<'a, '_> {
    fn node_text(&self, node: Node) -> Cow<'a, str> {
        self.scope_manager.node_text(node)
    }

    fn slice(&self, range: ops::Range<usize>) -> Cow<'a, str> {
        self.scope_manager.slice(range)
    }
}
