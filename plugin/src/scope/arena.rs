use std::cell::{Ref, RefCell, RefMut};

use id_arena::{Arena, Id};

use super::{definition::Definition, reference::_Reference, scope::_Scope, variable::_Variable};

#[derive(Default)]
pub struct AllArenas<'a> {
    pub references: RefCell<Arena<_Reference<'a>>>,
    pub scopes: RefCell<Arena<_Scope<'a>>>,
    pub variables: RefCell<Arena<_Variable<'a>>>,
    pub definitions: RefCell<Arena<Definition<'a>>>,
}

impl<'a> AllArenas<'a> {
    pub fn alloc_reference(&mut self, reference: _Reference<'a>) -> Id<_Reference<'a>> {
        self.references.borrow_mut().alloc(reference)
    }

    pub fn get_variable(&self, id: Id<_Variable<'a>>) -> Ref<_Variable<'a>> {
        Ref::map(self.variables.borrow(), |variables| {
            variables.get(id).unwrap()
        })
    }

    pub fn get_scope(&self, id: Id<_Scope<'a>>) -> Ref<_Scope<'a>> {
        Ref::map(self.scopes.borrow(), |scopes| scopes.get(id).unwrap())
    }

    pub fn get_scope_mut(&mut self, id: Id<_Scope<'a>>) -> RefMut<_Scope<'a>> {
        RefMut::map(self.scopes.borrow_mut(), |scopes| {
            scopes.get_mut(id).unwrap()
        })
    }
}
