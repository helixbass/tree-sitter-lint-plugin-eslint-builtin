use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
};

use id_arena::Id;
use tree_sitter_lint::tree_sitter::Node;

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

    pub fn __nest_catch_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_catch_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }
}
