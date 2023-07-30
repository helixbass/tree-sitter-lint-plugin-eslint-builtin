use std::cell::{Ref, RefMut};

use id_arena::Id;

use super::{arena::AllArenas, scope::Scope};

pub struct ScopeManager<'a> {
    scopes: Vec<Id<Scope>>,
    __current_scope: Option<Id<Scope>>,
    pub arena: AllArenas<'a>,
}

impl<'a> ScopeManager<'a> {
    pub fn new() -> Self {
        Self {
            scopes: Default::default(),
            __current_scope: Default::default(),
            arena: Default::default(),
        }
    }

    pub fn maybe_current_scope(&self) -> Option<Ref<Scope>> {
        self.__current_scope.map(|__current_scope| {
            Ref::map(self.arena.scopes.borrow(), |scopes| {
                scopes.get(__current_scope).unwrap()
            })
        })
    }

    pub fn maybe_current_scope_mut(&self) -> Option<RefMut<Scope>> {
        self.__current_scope.map(|__current_scope| {
            RefMut::map(self.arena.scopes.borrow_mut(), |scopes| {
                scopes.get_mut(__current_scope).unwrap()
            })
        })
    }

    pub fn __current_scope(&self) -> Ref<Scope> {
        self.maybe_current_scope().unwrap()
    }

    pub fn __current_scope_mut(&self) -> RefMut<Scope> {
        self.maybe_current_scope_mut().unwrap()
    }
}
