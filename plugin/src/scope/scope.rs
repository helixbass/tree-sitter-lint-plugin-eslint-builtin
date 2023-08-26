use std::{
    borrow::Cow,
    cell::{Ref, RefCell},
    collections::HashMap,
};

use id_arena::{Arena, Id};
use itertools::Itertools;
use squalid::{return_default_if_none, EverythingExt, OptionExt};
use tracing::{instrument, trace};
use tree_sitter_lint::{
    tree_sitter::Node,
    tree_sitter_grep::{return_if_none, SupportedLanguage},
    NodeExt, SourceTextProvider,
};

use super::{
    definition::Definition,
    reference::{ReadWriteFlags, Reference, _Reference},
    referencer::PatternAndNode,
    scope_manager::{NodeId, ScopeManager},
    variable::{Variable, VariableType, _Variable},
};
use crate::{
    ast_helpers::maybe_get_directive,
    break_if_none,
    kind::{ArrowFunction, Identifier, Program, StatementBlock},
};

fn is_strict_scope<'a>(
    arena: &Arena<_Scope>,
    source_text_provider: &impl SourceTextProvider<'a>,
    scope_upper: Option<Id<_Scope>>,
    scope_type: ScopeType,
    block: Node,
    is_method_definition: bool,
) -> bool {
    if scope_upper.matches(|scope_upper| arena[scope_upper].is_strict()) {
        return true;
    }

    if is_method_definition {
        return true;
    }

    if matches!(scope_type, ScopeType::Class | ScopeType::Module) {
        return true;
    }

    if matches!(scope_type, ScopeType::Block | ScopeType::Switch) {
        return false;
    }

    let body = match scope_type {
        ScopeType::Function => {
            if block.kind() == ArrowFunction && block.field("body").kind() != StatementBlock {
                return false;
            }

            return_default_if_none!(match block.kind() {
                Program => Some(block),
                _ => block.child_by_field_name("body"),
            })
        }
        ScopeType::Global => block,
        _ => return false,
    };

    body.non_comment_named_children(SupportedLanguage::Javascript)
        .map(|statement| maybe_get_directive(statement, source_text_provider))
        .take_while(|maybe_directive_text| maybe_directive_text.is_some())
        .any(|directive_text| {
            matches!(&*directive_text.unwrap(), "\"use strict\"" | "'use strict'")
        })
}

fn register_scope<'a>(scope_manager: &mut ScopeManager<'a>, scope: Id<_Scope<'a>>) {
    scope_manager.scopes.push(scope);

    scope_manager
        .__node_to_scope
        .entry(scope_manager.arena.scopes.borrow()[scope].block().id())
        .or_default()
        .push(scope);
}

fn should_be_statically(arena: &Arena<Definition>, def: Id<Definition>) -> bool {
    arena[def].type_() == VariableType::ClassName
        || arena[def].type_() == VariableType::Variable
            && arena[def].parent().unwrap().field("kind").kind() != "var"
}

#[derive(Debug)]
pub enum _Scope<'a> {
    Base(ScopeBase<'a>),
    Global(GlobalScope<'a>),
    Function(FunctionScope<'a>),
    With(WithScope<'a>),
}

impl<'a> _Scope<'a> {
    fn _new(
        scope_manager: &mut ScopeManager<'a>,
        type_: ScopeType,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
        is_method_definition: bool,
        create_from_base: impl Fn(ScopeBase<'a>, &ScopeManager<'a>) -> Self,
    ) -> Id<Self> {
        let id = {
            let mut arena = scope_manager.arena.scopes.borrow_mut();
            let variable_scope_or_waiting_to_grab_id = match type_ {
                ScopeType::Global
                | ScopeType::Module
                | ScopeType::Function
                | ScopeType::ClassFieldInitializer
                | ScopeType::ClassStaticBlock => None,
                _ => Some(arena.get(upper_scope.unwrap()).unwrap().variable_scope()),
            };
            let is_strict = if scope_manager.is_strict_mode_supported() {
                is_strict_scope(
                    &arena,
                    &*scope_manager,
                    upper_scope,
                    type_,
                    block,
                    is_method_definition,
                )
            } else {
                false
            };
            let id = arena.alloc_with_id(|id| {
                create_from_base(
                    ScopeBase {
                        id,
                        type_,
                        set: Default::default(),
                        taints: Default::default(),
                        dynamic: matches!(type_, ScopeType::Global | ScopeType::With),
                        block,
                        through: Default::default(),
                        variables: Default::default(),
                        references: Default::default(),
                        variable_scope: variable_scope_or_waiting_to_grab_id.unwrap_or(id),
                        function_expression_scope: Default::default(),
                        direct_call_to_eval_scope: Default::default(),
                        this_found: Default::default(),
                        __left: Some(Default::default()),
                        upper: upper_scope,
                        is_strict,
                        child_scopes: Default::default(),
                        // this.__declaredVariables = scopeManager.__declaredVariables
                    },
                    scope_manager,
                )
            });

            if let Some(upper_scope) = upper_scope {
                arena
                    .get_mut(upper_scope)
                    .unwrap()
                    .child_scopes_mut()
                    .push(id);
            }

            id
        };

        register_scope(scope_manager, id);

        id
    }

    pub fn new_base(
        scope_manager: &mut ScopeManager<'a>,
        type_: ScopeType,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
        is_method_definition: bool,
    ) -> Id<Self> {
        Self::_new(
            scope_manager,
            type_,
            upper_scope,
            block,
            is_method_definition,
            |base, _| Self::Base(base),
        )
    }

    pub fn new_global_scope(scope_manager: &mut ScopeManager<'a>, block: Node<'a>) -> Id<Self> {
        Self::_new(
            scope_manager,
            ScopeType::Global,
            None,
            block,
            false,
            |base, _| Self::Global(GlobalScope::new(base)),
        )
    }

    pub fn new_module_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Module, upper_scope, block, false)
    }

    pub fn new_function_expression_name_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        let ret = Self::_new(
            scope_manager,
            ScopeType::FunctionExpressionName,
            upper_scope,
            block,
            false,
            |mut base, _| {
                base.function_expression_scope = true;
                Self::Base(base)
            },
        );
        let definitions_arena = &scope_manager.arena.definitions;
        scope_manager.arena.scopes.borrow_mut()[ret].__define(
            &mut scope_manager.__declared_variables.borrow_mut(),
            &scope_manager.arena.variables,
            definitions_arena,
            &*scope_manager,
            block.field("name"),
            Definition::new(
                definitions_arena,
                VariableType::FunctionName,
                block.field("name"),
                block,
                None,
                None,
                None,
            ),
        );
        ret
    }

    pub fn new_catch_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Catch, upper_scope, block, false)
    }

    pub fn new_with_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::_new(
            scope_manager,
            ScopeType::With,
            upper_scope,
            block,
            false,
            |base, _| Self::With(WithScope::new(base)),
        )
    }

    pub fn new_block_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Block, upper_scope, block, false)
    }

    pub fn new_switch_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Switch, upper_scope, block, false)
    }

    pub fn new_function_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
        is_method_definition: bool,
    ) -> Id<Self> {
        Self::_new(
            scope_manager,
            ScopeType::Function,
            upper_scope,
            block,
            is_method_definition,
            |base, scope_manager| {
                Self::Function(FunctionScope::new(
                    &mut scope_manager.__declared_variables.borrow_mut(),
                    &scope_manager.arena.variables,
                    &scope_manager.arena.definitions,
                    base,
                ))
            },
        )
    }

    pub fn new_for_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::For, upper_scope, block, false)
    }

    pub fn new_class_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Class, upper_scope, block, false)
    }

    pub fn new_class_field_initializer_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(
            scope_manager,
            ScopeType::ClassFieldInitializer,
            upper_scope,
            block,
            true,
        )
    }

    pub fn new_class_static_block_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<_Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(
            scope_manager,
            ScopeType::ClassStaticBlock,
            upper_scope,
            block,
            true,
        )
    }

    fn __should_statically_close(&self, scope_manager: &ScopeManager) -> bool {
        !self.dynamic() || scope_manager.__is_optimistic()
    }

    fn __should_statically_close_for_global(
        &self,
        reference_arena: &Arena<_Reference<'a>>,
        variable_arena: &Arena<_Variable<'a>>,
        definition_arena: &Arena<Definition<'a>>,
        source_text_provider: &impl SourceTextProvider<'a>,
        ref_: Id<_Reference<'a>>,
    ) -> bool {
        let name = reference_arena[ref_].identifier.text(source_text_provider);

        let Some(variable) = self.set().get(&name).copied() else {
            return false;
        };
        let defs = &variable_arena[variable].defs;

        !defs.is_empty()
            && defs
                .into_iter()
                .all(|&def| should_be_statically(definition_arena, def))
    }

    fn __static_close_ref(
        self_: Id<Self>,
        reference_arena: &mut Arena<_Reference<'a>>,
        variable_arena: &mut Arena<_Variable<'a>>,
        scope_arena: &mut Arena<Self>,
        definition_arena: &Arena<Definition<'a>>,
        source_text_provider: &impl SourceTextProvider<'a>,
        ref_: Id<_Reference<'a>>,
    ) {
        if !Self::__resolve(
            self_,
            reference_arena,
            variable_arena,
            scope_arena,
            definition_arena,
            source_text_provider,
            ref_,
        ) {
            Self::__delegate_to_upper_scope(self_, scope_arena, ref_);
        }
    }

    fn __dynamic_close_ref(self_: Id<Self>, arena: &mut Arena<Self>, ref_: Id<_Reference<'a>>) {
        let mut current = self_;

        loop {
            arena[current].through_mut().push(ref_);
            current = return_if_none!(arena[current].maybe_upper());
        }
    }

    fn __global_close_ref(
        self_: Id<Self>,
        reference_arena: &mut Arena<_Reference<'a>>,
        variable_arena: &mut Arena<_Variable<'a>>,
        scope_arena: &mut Arena<Self>,
        definition_arena: &Arena<Definition<'a>>,
        source_text_provider: &impl SourceTextProvider<'a>,
        ref_: Id<_Reference<'a>>,
    ) {
        if scope_arena[self_].__should_statically_close_for_global(
            reference_arena,
            variable_arena,
            definition_arena,
            source_text_provider,
            ref_,
        ) {
            Self::__static_close_ref(
                self_,
                reference_arena,
                variable_arena,
                scope_arena,
                definition_arena,
                source_text_provider,
                ref_,
            );
        } else {
            Self::__dynamic_close_ref(self_, scope_arena, ref_);
        }
    }

    pub fn __close(self_: Id<Self>, scope_manager: &ScopeManager<'a>) -> Option<Id<Self>> {
        let arena = &mut scope_manager.arena.scopes.borrow_mut();
        if let Self::Global(global_scope) = &mut arena[self_] {
            let implicit = global_scope
                .base
                .__left
                .as_ref()
                .unwrap()
                .iter()
                .copied()
                .filter_map(|ref_| {
                    (&scope_manager.arena.references.borrow()[ref_]).thrush(|ref_| {
                        ref_.__maybe_implicit_global.filter(|_| {
                            !global_scope
                                .base
                                .set
                                .contains_key(&ref_.identifier.text(scope_manager))
                        })
                    })
                })
                .collect_vec();

            for info in implicit {
                info.pattern.thrush(|node| {
                    if node.kind() == Identifier {
                        global_scope.base.__define_generic(
                            &mut scope_manager.__declared_variables.borrow_mut(),
                            &scope_manager.arena.variables,
                            &scope_manager.arena.definitions,
                            node.text(scope_manager),
                            Some(&mut global_scope.implicit.set),
                            Some(&mut global_scope.implicit.variables),
                            Some(node),
                            Some(Definition::new(
                                &scope_manager.arena.definitions,
                                VariableType::ImplicitGlobalVariable,
                                node,
                                info.node,
                                None,
                                None,
                                None,
                            )),
                        );
                    }
                });
            }

            global_scope.implicit.left = global_scope.base.__left.clone().unwrap();
        }

        if matches!(&arena[self_], Self::With(_))
            && !arena[self_].__should_statically_close(scope_manager)
        {
            #[allow(clippy::unnecessary_to_owned)]
            for ref_ in arena[self_].__left().to_owned() {
                scope_manager.arena.references.borrow_mut()[ref_].tainted = true;
                Self::__delegate_to_upper_scope(self_, arena, ref_);
            }
            arena[self_].set__left(None);

            return arena[self_].maybe_upper();
        }

        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        #[allow(clippy::enum_variant_names)]
        enum CloseRef {
            StaticCloseRef,
            DynamicCloseRef,
            GlobalCloseRef,
        }
        let close_ref = if arena[self_].__should_statically_close(scope_manager) {
            CloseRef::StaticCloseRef
        } else if arena[self_].type_() != ScopeType::Global {
            CloseRef::DynamicCloseRef
        } else {
            CloseRef::GlobalCloseRef
        };

        #[allow(clippy::unnecessary_to_owned)]
        for ref_ in arena[self_].__left().to_owned() {
            match close_ref {
                CloseRef::StaticCloseRef => {
                    Self::__static_close_ref(
                        self_,
                        &mut scope_manager.arena.references.borrow_mut(),
                        &mut scope_manager.arena.variables.borrow_mut(),
                        arena,
                        &scope_manager.arena.definitions.borrow(),
                        scope_manager,
                        ref_,
                    );
                }
                CloseRef::DynamicCloseRef => {
                    Self::__dynamic_close_ref(self_, arena, ref_);
                }
                CloseRef::GlobalCloseRef => {
                    Self::__global_close_ref(
                        self_,
                        &mut scope_manager.arena.references.borrow_mut(),
                        &mut scope_manager.arena.variables.borrow_mut(),
                        arena,
                        &scope_manager.arena.definitions.borrow(),
                        scope_manager,
                        ref_,
                    );
                }
            }
        }
        arena[self_].set__left(None);

        arena[self_].maybe_upper()
    }

    fn __is_valid_resolution(
        &self,
        variable_arena: &Arena<_Variable<'a>>,
        reference_arena: &Arena<_Reference<'a>>,
        definition_arena: &Arena<Definition<'a>>,
        ref_: Id<_Reference<'a>>,
        variable: Id<_Variable<'a>>,
    ) -> bool {
        match self {
            Self::Function(_) => {
                if self.block().kind() == Program {
                    return true;
                }

                let body_start = self.block().range().start_byte;

                !(variable_arena[variable].scope == self.id()
                    && reference_arena[ref_].identifier.range().start_byte < body_start
                    && variable_arena[variable]
                        .defs
                        .iter()
                        .all(|&d| definition_arena[d].name().range().start_byte >= body_start))
            }
            _ => true,
        }
    }

    fn __resolve(
        self_: Id<Self>,
        reference_arena: &mut Arena<_Reference<'a>>,
        variable_arena: &mut Arena<_Variable<'a>>,
        scope_arena: &mut Arena<Self>,
        definition_arena: &Arena<Definition<'a>>,
        source_text_provider: &impl SourceTextProvider<'a>,
        ref_: Id<_Reference<'a>>,
    ) -> bool {
        let name = reference_arena[ref_].identifier.text(source_text_provider);

        let Some(variable) = scope_arena[self_].set().get(&name).copied() else {
            return false;
        };

        if !scope_arena[self_].__is_valid_resolution(
            variable_arena,
            reference_arena,
            definition_arena,
            ref_,
            variable,
        ) {
            return false;
        }
        variable_arena[variable].references.push(ref_);
        (&mut variable_arena[variable]).thrush(|variable| {
            variable.stack = variable.stack
                && scope_arena[reference_arena[ref_].from].variable_scope()
                    == scope_arena[self_].variable_scope();
        });
        if reference_arena[ref_].tainted {
            variable_arena[variable].tainted = true;
            scope_arena[self_]
                .taints_mut()
                .insert(variable_arena[variable].name.clone().into_owned(), true);
        }
        reference_arena[ref_].resolved = Some(variable);

        true
    }

    fn __delegate_to_upper_scope(
        self_: Id<Self>,
        arena: &mut Arena<Self>,
        ref_: Id<_Reference<'a>>,
    ) {
        if let Some(upper) = arena[self_].maybe_upper() {
            arena[upper].__left_mut().push(ref_);
        }
        arena[self_].through_mut().push(ref_);
    }

    fn __add_declared_variables_of_node(
        &self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<_Variable<'a>>>>,
        variable: Id<_Variable<'a>>,
        node: Option<Node>,
    ) {
        self.base()
            .__add_declared_variables_of_node(__declared_variables, variable, node)
    }

    #[allow(clippy::too_many_arguments)]
    fn __define_generic(
        &mut self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<_Variable<'a>>>>,
        variable_arena: &RefCell<Arena<_Variable<'a>>>,
        definition_arena: &RefCell<Arena<Definition<'a>>>,
        name: Cow<'a, str>,
        set: Option<&mut Set<'a>>,
        variables: Option<&mut Vec<Id<_Variable<'a>>>>,
        node: Option<Node<'a>>,
        def: Option<Id<Definition<'a>>>,
    ) {
        self.base_mut().__define_generic(
            __declared_variables,
            variable_arena,
            definition_arena,
            name,
            set,
            variables,
            node,
            def,
        )
    }

    pub fn __define(
        &mut self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<_Variable<'a>>>>,
        variable_arena: &RefCell<Arena<_Variable<'a>>>,
        definition_arena: &RefCell<Arena<Definition<'a>>>,
        source_text_provider: &impl SourceTextProvider<'a>,
        node: Node<'a>,
        def: Id<Definition<'a>>,
    ) {
        if node.kind() == Identifier {
            self.__define_generic(
                __declared_variables,
                variable_arena,
                definition_arena,
                source_text_provider.node_text(node),
                None,
                None,
                Some(node),
                Some(def),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn __referencing(
        &mut self,
        arena: &mut Arena<_Reference<'a>>,
        node: Node<'a>,
        assign: Option<ReadWriteFlags>,
        write_expr: Option<Node<'a>>,
        maybe_implicit_global: Option<PatternAndNode<'a>>,
        partial: Option<bool>,
        init: Option<bool>,
    ) {
        if node.kind() != Identifier {
            return;
        }

        let ref_ = _Reference::new(
            arena,
            node,
            self.id(),
            if assign.unwrap_or_default() == ReadWriteFlags::NONE {
                ReadWriteFlags::READ
            } else {
                assign.unwrap()
            },
            write_expr,
            maybe_implicit_global,
            partial.unwrap_or_default(),
            init.unwrap_or_default(),
        );

        self.references_mut().push(ref_);
        self.__left_mut().push(ref_);
    }

    pub fn __detect_eval(id: Id<Self>, arena: &mut Arena<Self>) {
        arena
            .get_mut(id)
            .unwrap()
            .set_direct_call_to_eval_scope(true);
        let mut current = id;
        loop {
            let current_scope = arena.get_mut(current).unwrap();
            current_scope.set_dynamic(true);
            current = break_if_none!(current_scope.maybe_upper());
        }
    }

    pub fn __detect_this(&mut self) {
        self.set_this_found(true);
    }

    fn __is_closed(&self) -> bool {
        self.maybe__left().is_none()
    }

    pub fn resolve(
        &self,
        reference_arena: &Arena<_Reference<'a>>,
        ident: Node<'a>,
    ) -> Option<Id<_Reference<'a>>> {
        assert!(self.__is_closed(), "Scope should be closed.");
        assert!(ident.kind() == Identifier, "Target should be identifier.");
        self.references()
            .into_iter()
            .find(|&&ref_| reference_arena[ref_].identifier == ident)
            .copied()
    }

    pub fn is_static(&self) -> bool {
        !self.dynamic()
    }

    pub fn is_arguments_materialized(&self, variable_arena: &Arena<_Variable<'a>>) -> bool {
        match self {
            Self::Function(_) => {
                if self.block().kind() == ArrowFunction {
                    return false;
                }

                if !self.is_static() {
                    return true;
                }

                let variable = self
                    .set()
                    .get("arguments")
                    .copied()
                    .expect("Always have arguments variable.");
                (&variable_arena[variable])
                    .thrush(|variable| variable.tainted || !variable.references.is_empty())
            }
            _ => true,
        }
    }

    pub fn is_this_materialized(&self) -> bool {
        match self {
            Self::Function(_) => {
                if !self.is_static() {
                    return true;
                }
                self.this_found()
            }
            _ => true,
        }
    }

    pub fn is_used_name(
        &self,
        reference_arena: &Arena<_Reference<'a>>,
        source_text_provider: &impl SourceTextProvider<'a>,
        name: &str,
    ) -> bool {
        if self.set().contains_key(name) {
            return true;
        }
        self.through().into_iter().any(|&through| {
            reference_arena[through]
                .identifier
                .text(source_text_provider)
                == name
        })
    }

    pub fn block(&self) -> Node {
        self.base().block
    }

    pub fn child_scopes_mut(&mut self) -> &mut Vec<Id<Self>> {
        &mut self.base_mut().child_scopes
    }

    pub fn variable_scope(&self) -> Id<Self> {
        self.base().variable_scope
    }

    pub fn type_(&self) -> ScopeType {
        self.base().type_
    }

    fn set(&self) -> &Set<'a> {
        &self.base().set
    }

    pub fn set_is_strict(&mut self, is_strict: bool) {
        self.base_mut().is_strict = is_strict;
    }

    pub fn set_direct_call_to_eval_scope(&mut self, direct_call_to_eval_scope: bool) {
        self.base_mut().direct_call_to_eval_scope = direct_call_to_eval_scope;
    }

    pub fn dynamic(&self) -> bool {
        self.base().dynamic
    }

    pub fn set_dynamic(&mut self, dynamic: bool) {
        self.base_mut().dynamic = dynamic;
    }

    pub fn maybe_upper(&self) -> Option<Id<Self>> {
        self.base().upper
    }

    pub fn this_found(&self) -> bool {
        self.base().this_found
    }

    pub fn set_this_found(&mut self, this_found: bool) {
        self.base_mut().this_found = this_found;
    }

    #[allow(non_snake_case)]
    fn maybe__left(&self) -> Option<&[Id<_Reference<'a>>]> {
        self.base().__left.as_deref()
    }

    fn __left(&self) -> &[Id<_Reference<'a>>] {
        self.maybe__left().unwrap()
    }

    fn __left_mut(&mut self) -> &mut Vec<Id<_Reference<'a>>> {
        self.base_mut().__left.as_mut().unwrap()
    }

    #[allow(non_snake_case)]
    fn set__left(&mut self, __left: Option<Vec<Id<_Reference<'a>>>>) {
        self.base_mut().__left = __left;
    }

    pub fn through(&self) -> &[Id<_Reference<'a>>] {
        &self.base().through
    }

    pub fn through_mut(&mut self) -> &mut Vec<Id<_Reference<'a>>> {
        &mut self.base_mut().through
    }

    fn base(&self) -> &ScopeBase<'a> {
        match self {
            _Scope::Base(value) => value,
            _Scope::Global(value) => &value.base,
            _Scope::Function(value) => &value.base,
            _Scope::With(value) => &value.base,
        }
    }

    fn base_mut(&mut self) -> &mut ScopeBase<'a> {
        match self {
            _Scope::Base(value) => value,
            _Scope::Global(value) => &mut value.base,
            _Scope::Function(value) => &mut value.base,
            _Scope::With(value) => &mut value.base,
        }
    }

    pub fn is_strict(&self) -> bool {
        self.base().is_strict
    }

    pub fn id(&self) -> Id<Self> {
        self.base().id
    }

    pub fn taints_mut(&mut self) -> &mut HashMap<String, bool> {
        &mut self.base_mut().taints
    }

    pub fn references(&self) -> &[Id<_Reference<'a>>] {
        &self.base().references
    }

    pub fn references_mut(&mut self) -> &mut Vec<Id<_Reference<'a>>> {
        &mut self.base_mut().references
    }

    pub fn function_expression_scope(&self) -> bool {
        self.base().function_expression_scope
    }
}

#[derive(Debug)]
pub struct Scope<'a, 'b> {
    scope: Ref<'b, _Scope<'a>>,
    scope_manager: &'b ScopeManager<'a>,
}

impl<'a, 'b> Scope<'a, 'b> {
    pub fn new(scope: Ref<'b, _Scope<'a>>, scope_manager: &'b ScopeManager<'a>) -> Self {
        Self {
            scope,
            scope_manager,
        }
    }

    pub fn type_(&self) -> ScopeType {
        self.scope.type_()
    }

    pub fn variables(&self) -> impl Iterator<Item = Variable<'a, 'b>> + '_ {
        self.scope
            .base()
            .variables
            .iter()
            .map(|variable| self.scope_manager.borrow_variable(*variable))
    }

    pub fn references(&self) -> impl Iterator<Item = Reference<'a, 'b>> + '_ {
        self.scope
            .references()
            .into_iter()
            .map(|reference| self.scope_manager.borrow_reference(*reference))
    }

    pub fn is_arguments_materialized(&self) -> bool {
        self.scope.is_arguments_materialized(&self.scope_manager.arena.variables.borrow())
    }

    pub fn child_scopes(&self) -> impl Iterator<Item = Scope<'a, 'b>> + '_ {
        self.scope
            .base()
            .child_scopes
            .iter()
            .map(|scope| self.scope_manager.borrow_scope(*scope))
    }

    pub fn block(&self) -> Node<'a> {
        self.scope.base().block
    }

    pub fn variable_scope(&self) -> Self {
        self.scope_manager.borrow_scope(self.scope.base().variable_scope)
    }

    pub fn maybe_upper(&self) -> Option<Self> {
        self.scope.maybe_upper().map(|upper| self.scope_manager.borrow_scope(upper))
    }

    pub fn upper(&self) -> Self {
        self.maybe_upper().unwrap()
    }

    pub fn is_strict(&self) -> bool {
        self.scope.is_strict()
    }

    pub fn function_expression_scope(&self) -> bool {
        self.scope.function_expression_scope()
    }

    pub fn set(&self) -> HashMap<Cow<'a, str>, Variable<'a, 'b>> {
        self.scope.set().into_iter().map(|(key, value)| {
            (key.clone(), self.scope_manager.borrow_variable(*value))
        }).collect()
    }
}

impl<'a, 'b> PartialEq for Scope<'a, 'b> {
    fn eq(&self, other: &Self) -> bool {
        self.scope.id() == other.scope.id()
    }
}

impl<'a, 'b> Eq for Scope<'a, 'b> {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScopeType {
    Global,
    Module,
    Function,
    FunctionExpressionName,
    Block,
    Switch,
    Catch,
    With,
    For,
    Class,
    ClassFieldInitializer,
    ClassStaticBlock,
}

type Set<'a> = HashMap<Cow<'a, str>, Id<_Variable<'a>>>;

#[derive(Debug)]
pub struct ScopeBase<'a> {
    id: Id<_Scope<'a>>,
    type_: ScopeType,
    set: Set<'a>,
    taints: HashMap<String, bool>,
    dynamic: bool,
    block: Node<'a>,
    through: Vec<Id<_Reference<'a>>>,
    variables: Vec<Id<_Variable<'a>>>,
    references: Vec<Id<_Reference<'a>>>,
    variable_scope: Id<_Scope<'a>>,
    function_expression_scope: bool,
    direct_call_to_eval_scope: bool,
    this_found: bool,
    __left: Option<Vec<Id<_Reference<'a>>>>,
    upper: Option<Id<_Scope<'a>>>,
    is_strict: bool,
    child_scopes: Vec<Id<_Scope<'a>>>,
}

impl<'a> ScopeBase<'a> {
    fn __add_declared_variables_of_node(
        &self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<_Variable<'a>>>>,
        variable: Id<_Variable<'a>>,
        node: Option<Node>,
    ) {
        let node = return_if_none!(node);

        let variables = __declared_variables.entry(node.id()).or_default();
        if !variables.contains(&variable) {
            variables.push(variable);
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[instrument(level = "trace", skip_all, fields(?name))]
    fn __define_generic(
        &mut self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<_Variable<'a>>>>,
        variable_arena: &RefCell<Arena<_Variable<'a>>>,
        definition_arena: &RefCell<Arena<Definition<'a>>>,
        name: Cow<'a, str>,
        set: Option<&mut Set<'a>>,
        variables: Option<&mut Vec<Id<_Variable<'a>>>>,
        node: Option<Node<'a>>,
        def: Option<Id<Definition<'a>>>,
    ) {
        let mut did_insert = false;
        let id = self.id();
        let variable = *set
            .unwrap_or(&mut self.set)
            .entry(name.clone())
            .or_insert_with(|| {
                trace!("new variable");

                did_insert = true;
                _Variable::new(&mut variable_arena.borrow_mut(), name, id)
            });
        if did_insert {
            variables.unwrap_or(&mut self.variables).push(variable);
        }

        if let Some(def) = def {
            variable_arena.borrow_mut()[variable].defs.push(def);
            let definition_arena = definition_arena.borrow();
            let def = &definition_arena[def];
            self.__add_declared_variables_of_node(__declared_variables, variable, Some(def.node()));
            self.__add_declared_variables_of_node(__declared_variables, variable, def.parent());
        }
        if let Some(node) = node {
            variable_arena.borrow_mut()[variable].identifiers.push(node);
        }
    }

    fn id(&self) -> Id<_Scope<'a>> {
        self.id
    }
}

#[derive(Debug)]
pub struct GlobalScope<'a> {
    base: ScopeBase<'a>,
    implicit: GlobalScopeImplicit<'a>,
}

impl<'a> GlobalScope<'a> {
    pub fn new(base: ScopeBase<'a>) -> Self {
        Self {
            base,
            implicit: Default::default(),
        }
    }
}

#[derive(Debug, Default)]
pub struct GlobalScopeImplicit<'a> {
    set: Set<'a>,
    variables: Vec<Id<_Variable<'a>>>,
    left: Vec<Id<_Reference<'a>>>,
}

#[derive(Debug)]
pub struct FunctionScope<'a> {
    base: ScopeBase<'a>,
}

impl<'a> FunctionScope<'a> {
    pub fn new(
        __declared_variables: &mut HashMap<NodeId, Vec<Id<_Variable<'a>>>>,
        variable_arena: &RefCell<Arena<_Variable<'a>>>,
        definition_arena: &RefCell<Arena<Definition<'a>>>,
        base: ScopeBase<'a>,
    ) -> Self {
        let mut ret = Self { base };
        if ret.base.block.kind() != ArrowFunction {
            ret.__define_arguments(__declared_variables, variable_arena, definition_arena);
        }
        ret
    }

    #[instrument(level = "trace", skip_all)]
    fn __define_arguments(
        &mut self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<_Variable<'a>>>>,
        variable_arena: &RefCell<Arena<_Variable<'a>>>,
        definition_arena: &RefCell<Arena<Definition<'a>>>,
    ) {
        self.base.__define_generic(
            __declared_variables,
            variable_arena,
            definition_arena,
            "arguments".into(),
            None,
            None,
            None,
            None,
        );
        self.base.taints.insert("arguments".to_owned(), true);
    }
}

#[derive(Debug)]
pub struct WithScope<'a> {
    base: ScopeBase<'a>,
}

impl<'a> WithScope<'a> {
    pub fn new(base: ScopeBase<'a>) -> Self {
        Self { base }
    }
}
