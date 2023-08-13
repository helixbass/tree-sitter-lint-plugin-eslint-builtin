use std::{
    borrow::Cow,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    ops,
};

use id_arena::Id;
use tree_sitter_lint::{tree_sitter::Node, SourceTextProvider};

use super::{
    arena::AllArenas,
    scope::{Scope, ScopeType},
    variable::Variable,
};

pub type NodeId = usize;

pub struct ScopeManager<'a> {
    pub scopes: Vec<Id<Scope<'a>>>,
    global_scope: Option<Id<Scope<'a>>>,
    pub __node_to_scope: HashMap<NodeId, Vec<Id<Scope<'a>>>>,
    pub __current_scope: Option<Id<Scope<'a>>>,
    pub arena: AllArenas<'a>,
    pub __declared_variables: RefCell<HashMap<NodeId, Vec<Id<Variable<'a>>>>>,
    pub source_text: &'a [u8],
}

impl<'a> ScopeManager<'a> {
    pub fn new(source_text: &'a [u8]) -> Self {
        Self {
            scopes: Default::default(),
            global_scope: Default::default(),
            __node_to_scope: Default::default(),
            __current_scope: Default::default(),
            arena: Default::default(),
            __declared_variables: Default::default(),
            source_text,
        }
    }

    pub fn __use_directive(&self) -> bool {
        unimplemented!()
    }

    pub fn __ignore_eval(&self) -> bool {
        unimplemented!()
    }

    pub fn is_global_return(&self) -> bool {
        unimplemented!()
    }

    pub fn is_module(&self) -> bool {
        unimplemented!()
    }

    pub fn is_implied_strict(&self) -> bool {
        unimplemented!()
    }

    pub fn is_strict_mode_supported(&self) -> bool {
        unimplemented!()
    }

    pub fn maybe_current_scope(&self) -> Option<Ref<Scope<'a>>> {
        self.__current_scope.map(|__current_scope| {
            Ref::map(self.arena.scopes.borrow(), |scopes| {
                scopes.get(__current_scope).unwrap()
            })
        })
    }

    pub fn maybe_current_scope_mut(&self) -> Option<RefMut<Scope<'a>>> {
        self.__current_scope.map(|__current_scope| {
            RefMut::map(self.arena.scopes.borrow_mut(), |scopes| {
                scopes.get_mut(__current_scope).unwrap()
            })
        })
    }

    pub fn __current_scope(&self) -> Ref<Scope<'a>> {
        self.maybe_current_scope().unwrap()
    }

    pub fn __current_scope_mut(&self) -> RefMut<Scope<'a>> {
        self.maybe_current_scope_mut().unwrap()
    }

    fn __nest_scope(&mut self, scope: Id<Scope<'a>>) -> Id<Scope<'a>> {
        if self.arena.scopes.borrow().get(scope).unwrap().type_() == ScopeType::Global {
            assert!(self.__current_scope.is_none());
            self.global_scope = Some(scope);
        }
        self.__current_scope = Some(scope);
        scope
    }

    pub fn __nest_global_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_global_scope(self, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_block_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_block_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_function_scope(
        &mut self,
        node: Node<'a>,
        is_method_definition: bool,
    ) -> Id<Scope<'a>> {
        let scope =
            Scope::new_function_scope(self, self.__current_scope, node, is_method_definition);
        self.__nest_scope(scope)
    }

    pub fn __nest_for_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_for_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_catch_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_catch_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_with_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_with_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_field_initializer_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_class_field_initializer_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_class_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_static_block_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_class_static_block_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_switch_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_switch_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_module_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_module_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_function_expression_name_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_function_expression_name_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __is_es6(&self) -> bool {
        unimplemented!()
    }
}

impl<'a> SourceTextProvider<'a> for ScopeManager<'a> {
    fn node_text(&self, node: Node) -> Cow<'a, str> {
        self.source_text.node_text(node)
    }

    fn slice(&self, range: ops::Range<usize>) -> Cow<'a, str> {
        self.source_text.slice(range)
    }
}
