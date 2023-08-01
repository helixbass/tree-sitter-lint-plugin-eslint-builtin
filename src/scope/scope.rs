use std::{borrow::Cow, cell::RefCell, collections::HashMap};

use id_arena::{Arena, Id};
use tree_sitter_lint::{tree_sitter::Node, tree_sitter_grep::return_if_none};

use crate::{
    break_if_none,
    kind::{ArrowFunction, Identifier},
    text::SourceTextProvider,
};

use super::{
    definition::Definition,
    reference::{ReadWriteFlags, Reference},
    referencer::PatternAndNode,
    scope_manager::{NodeId, ScopeManager},
    variable::Variable,
};

fn is_strict_scope(
    scope_upper: Option<Id<Scope>>,
    scope_type: ScopeType,
    block: Node,
    is_method_definition: bool,
    use_directive: bool,
) -> bool {
    unimplemented!()
}

fn register_scope<'a>(scope_manager: &mut ScopeManager<'a>, scope: Id<Scope<'a>>) {
    scope_manager.scopes.push(scope);

    scope_manager
        .__node_to_scope
        .entry(
            scope_manager
                .arena
                .scopes
                .borrow()
                .get(scope)
                .unwrap()
                .block()
                .id(),
        )
        .or_default()
        .push(scope);
}

pub enum Scope<'a> {
    Base(ScopeBase<'a>),
    Global(GlobalScope<'a>),
    Function(FunctionScope<'a>),
    With(WithScope<'a>),
}

impl<'a> Scope<'a> {
    fn _new(
        scope_manager: &mut ScopeManager<'a>,
        type_: ScopeType,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
        is_method_definition: bool,
        create_from_base: impl Fn(ScopeBase<'a>) -> Self,
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
            let id = arena.alloc_with_id(|id| {
                create_from_base(ScopeBase {
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
                    __left: Default::default(),
                    upper: upper_scope,
                    is_strict: if scope_manager.is_strict_mode_supported() {
                        is_strict_scope(
                            upper_scope,
                            type_,
                            block,
                            is_method_definition,
                            scope_manager.__use_directive(),
                        )
                    } else {
                        false
                    },
                    child_scopes: Default::default(),
                    // this.__declaredVariables = scopeManager.__declaredVariables
                })
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
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
        is_method_definition: bool,
    ) -> Id<Self> {
        Self::_new(
            scope_manager,
            type_,
            upper_scope,
            block,
            is_method_definition,
            |base| Self::Base(base),
        )
    }

    pub fn new_global_scope(scope_manager: &mut ScopeManager<'a>, block: Node<'a>) -> Id<Self> {
        Self::_new(
            scope_manager,
            ScopeType::Global,
            None,
            block,
            false,
            |base| Self::Global(GlobalScope::new(base)),
        )
    }

    pub fn new_module_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Module, upper_scope, block, false)
    }

    pub fn new_catch_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Catch, upper_scope, block, false)
    }

    pub fn new_with_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::_new(
            scope_manager,
            ScopeType::With,
            upper_scope,
            block,
            false,
            |base| Self::With(WithScope::new(base)),
        )
    }

    pub fn new_block_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Block, upper_scope, block, false)
    }

    pub fn new_switch_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Switch, upper_scope, block, false)
    }

    pub fn new_function_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
        is_method_definition: bool,
    ) -> Id<Self> {
        Self::_new(
            scope_manager,
            ScopeType::Function,
            upper_scope,
            block,
            is_method_definition,
            |base| Self::Function(FunctionScope::new(base)),
        )
    }

    pub fn new_for_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::For, upper_scope, block, false)
    }

    pub fn new_class_field_initializer_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
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
        upper_scope: Option<Id<Scope<'a>>>,
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

    pub fn is_strict(&self) -> bool {
        unimplemented!()
    }

    pub fn id(&self) -> Id<Self> {
        unimplemented!()
    }

    pub fn __close(&self, scope_manager: &ScopeManager) -> Option<Id<Self>> {
        unimplemented!()
    }

    fn __add_declared_variables_of_node(
        &self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<Variable<'a>>>>,
        variable: Id<Variable<'a>>,
        node: Option<Node>,
    ) {
        let node = return_if_none!(node);

        let variables = __declared_variables.entry(node.id()).or_default();
        if !variables.contains(&variable) {
            variables.push(variable);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn __define_generic(
        &mut self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<Variable<'a>>>>,
        variable_arena: &RefCell<Arena<Variable<'a>>>,
        definition_arena: &RefCell<Arena<Definition<'a>>>,
        name: Cow<'a, str>,
        mut get_or_create_variable: impl FnMut(&mut Self, Cow<'a, str>) -> Id<Variable<'a>>,
        node: Option<Node<'a>>,
        def: Option<Id<Definition<'a>>>,
    ) {
        let variable = get_or_create_variable(self, name);

        if let Some(def) = def {
            variable_arena
                .borrow_mut()
                .get_mut(variable)
                .unwrap()
                .defs
                .push(def);
            let definition_arena = definition_arena.borrow();
            let def = definition_arena.get(def).unwrap();
            self.__add_declared_variables_of_node(__declared_variables, variable, Some(def.node()));
            self.__add_declared_variables_of_node(__declared_variables, variable, def.parent());
        }
        if let Some(node) = node {
            variable_arena
                .borrow_mut()
                .get_mut(variable)
                .unwrap()
                .identifiers
                .push(node);
        }
    }

    pub fn __define(
        &mut self,
        __declared_variables: &mut HashMap<NodeId, Vec<Id<Variable<'a>>>>,
        variable_arena: &RefCell<Arena<Variable<'a>>>,
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
                source_text_provider.get_node_text(node),
                |this, name| {
                    let mut did_insert = false;
                    let id = this.id();
                    let ret = *this.set_mut().entry(name.clone()).or_insert_with(|| {
                        did_insert = true;
                        Variable::new(&mut variable_arena.borrow_mut(), name, id)
                    });
                    if did_insert {
                        this.variables_mut().push(ret);
                    }
                    ret
                },
                Some(node),
                Some(def),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn __referencing(
        &mut self,
        arena: &mut Arena<Reference<'a>>,
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

        let ref_ = Reference::new(
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
            current = break_if_none!(current_scope.upper());
        }
    }

    pub fn __detect_this(&mut self) {
        self.set_this_found(true);
    }

    pub fn is_static(&self) -> bool {
        unimplemented!()
    }

    pub fn block(&self) -> Node {
        unimplemented!()
    }

    pub fn child_scopes_mut(&mut self) -> &mut Vec<Id<Self>> {
        unimplemented!()
    }

    pub fn variable_scope(&self) -> Id<Self> {
        unimplemented!()
    }

    pub fn type_(&self) -> ScopeType {
        unimplemented!()
    }

    fn set_mut(&mut self) -> &mut HashMap<Cow<'a, str>, Id<Variable<'a>>> {
        unimplemented!()
    }

    fn variables_mut(&mut self) -> &mut Vec<Id<Variable<'a>>> {
        unimplemented!()
    }

    pub fn set_is_strict(&mut self, is_strict: bool) {
        unimplemented!()
    }

    pub fn set_direct_call_to_eval_scope(&mut self, direct_call_to_eval_scope: bool) {
        unimplemented!()
    }

    pub fn set_dynamic(&mut self, dynamic: bool) {
        unimplemented!()
    }

    pub fn upper(&self) -> Option<Id<Self>> {
        unimplemented!()
    }

    pub fn set_this_found(&mut self, this_found: bool) {
        unimplemented!()
    }
}

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

pub struct ScopeBase<'a> {
    id: Id<Scope<'a>>,
    type_: ScopeType,
    set: HashMap<Cow<'a, str>, Id<Variable<'a>>>,
    taints: HashMap<String, bool>,
    dynamic: bool,
    block: Node<'a>,
    through: Vec<Id<Reference<'a>>>,
    variables: Vec<Id<Variable<'a>>>,
    references: Vec<Id<Reference<'a>>>,
    variable_scope: Id<Scope<'a>>,
    function_expression_scope: bool,
    direct_call_to_eval_scope: bool,
    this_found: bool,
    __left: Vec<Id<Reference<'a>>>,
    upper: Option<Id<Scope<'a>>>,
    is_strict: bool,
    child_scopes: Vec<Id<Scope<'a>>>,
}

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

#[derive(Default)]
pub struct GlobalScopeImplicit<'a> {
    set: HashMap<&'a str, Id<Variable<'a>>>,
    variables: Vec<Id<Variable<'a>>>,
    left: Vec<Id<Reference<'a>>>,
}

pub struct FunctionScope<'a> {
    base: ScopeBase<'a>,
}

impl<'a> FunctionScope<'a> {
    pub fn new(base: ScopeBase<'a>) -> Self {
        let mut ret = Self { base };
        if ret.base.block.kind() != ArrowFunction {
            ret.__define_arguments();
        }
        ret
    }

    fn __define_arguments(&mut self) {
        unimplemented!()
    }
}

pub struct WithScope<'a> {
    base: ScopeBase<'a>,
}

impl<'a> WithScope<'a> {
    pub fn new(base: ScopeBase<'a>) -> Self {
        Self { base }
    }
}
