use std::{borrow::Cow, cell::Ref, hash};

use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::{
    definition::_Definition,
    reference::{Reference, _Reference},
    scope::{Scope, _Scope},
    Definition, ScopeManager,
};

#[derive(Debug)]
pub struct _Variable<'a> {
    pub name: Cow<'a, str>,
    pub identifiers: Vec<Node<'a>>,
    pub references: Vec<Id<_Reference<'a>>>,
    pub defs: Vec<Id<_Definition<'a>>>,
    pub tainted: bool,
    pub stack: bool,
    pub scope: Id<_Scope<'a>>,
    id: Id<Self>,
    pub writeable: Option<bool>,
    pub explicit_global_comments: Option<Vec<Node<'a>>>,
}

impl<'a> _Variable<'a> {
    pub fn new(arena: &mut Arena<Self>, name: Cow<'a, str>, scope: Id<_Scope<'a>>) -> Id<Self> {
        arena.alloc_with_id(|id| Self {
            name,
            identifiers: Default::default(),
            references: Default::default(),
            defs: Default::default(),
            tainted: Default::default(),
            stack: true,
            scope,
            id,
            writeable: Default::default(),
            explicit_global_comments: Default::default(),
        })
    }
}

#[derive(Debug)]
pub struct Variable<'a, 'b> {
    variable: Ref<'b, _Variable<'a>>,
    scope_manager: &'b ScopeManager<'a>,
}

impl<'a, 'b> Variable<'a, 'b> {
    pub fn new(variable: Ref<'b, _Variable<'a>>, scope_manager: &'b ScopeManager<'a>) -> Self {
        Self {
            variable,
            scope_manager,
        }
    }

    pub fn name(&self) -> &str {
        &self.variable.name
    }

    pub fn scope(&self) -> Scope<'a, 'b> {
        self.scope_manager.borrow_scope(self.variable.scope)
    }

    pub fn references(&self) -> impl Iterator<Item = Reference<'a, 'b>> + '_ {
        self.variable
            .references
            .iter()
            .map(|&reference| self.scope_manager.borrow_reference(reference))
    }

    pub fn defs(&self) -> impl Iterator<Item = Definition<'a, 'b>> + '_ {
        self.variable
            .defs
            .iter()
            .map(|&def| self.scope_manager.borrow_definition(def))
    }

    pub fn identifiers(&self) -> impl Iterator<Item = Node<'a>> + '_ {
        self.variable.identifiers.iter().copied()
    }

    pub fn explicit_global_comments(&self) -> Option<impl Iterator<Item = Node<'a>> + '_> {
        self.variable
            .explicit_global_comments
            .as_ref()
            .map(|explicit_global_comments| explicit_global_comments.iter().copied())
    }
}

impl<'a, 'b> PartialEq for Variable<'a, 'b> {
    fn eq(&self, other: &Self) -> bool {
        self.variable.id == other.variable.id
    }
}

impl<'a, 'b> Eq for Variable<'a, 'b> {}

impl<'a, 'b> hash::Hash for Variable<'a, 'b> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.variable.id.hash(state);
    }
}

impl<'a, 'b> Clone for Variable<'a, 'b> {
    fn clone(&self) -> Self {
        Self {
            variable: Ref::clone(&self.variable),
            scope_manager: self.scope_manager,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VariableType {
    CatchClause,
    Parameter,
    FunctionName,
    ClassName,
    Variable,
    ImportBinding,
    ImplicitGlobalVariable,
}
