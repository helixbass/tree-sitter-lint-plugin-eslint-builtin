use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use id_arena::{Arena, Id};

use super::{reference::Reference, scope::Scope, variable::Variable};

#[derive(Default)]
pub struct AllArenas<'a> {
    pub references: Rc<RefCell<Arena<Reference<'a>>>>,
    pub scopes: Rc<RefCell<Arena<Scope>>>,
    pub variables: Rc<RefCell<Arena<Variable<'a>>>>,
}

impl<'a> AllArenas<'a> {
    pub fn alloc_reference(&mut self, reference: Reference<'a>) -> Id<Reference<'a>> {
        self.references.borrow_mut().alloc(reference)
    }

    pub fn get_variable(&self, id: Id<Variable<'a>>) -> Ref<Variable<'a>> {
        Ref::map((*self.variables).borrow(), |variables| {
            variables.get(id).unwrap()
        })
    }

    pub fn get_scope(&self, id: Id<Scope>) -> Ref<Scope> {
        Ref::map((*self.scopes).borrow(), |scopes| scopes.get(id).unwrap())
    }

    pub fn get_scope_mut(&mut self, id: Id<Scope>) -> RefMut<Scope> {
        RefMut::map(self.scopes.borrow_mut(), |scopes| {
            scopes.get_mut(id).unwrap()
        })
    }
}
