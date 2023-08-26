use std::{borrow::Cow, cell::Ref};

use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::{definition::Definition, reference::_Reference, scope::_Scope, ScopeManager};

pub struct _Variable<'a> {
    pub name: Cow<'a, str>,
    pub identifiers: Vec<Node<'a>>,
    pub references: Vec<Id<_Reference<'a>>>,
    pub defs: Vec<Id<Definition<'a>>>,
    pub tainted: bool,
    pub stack: bool,
    pub scope: Id<_Scope<'a>>,
}

impl<'a> _Variable<'a> {
    pub fn new(arena: &mut Arena<Self>, name: Cow<'a, str>, scope: Id<_Scope<'a>>) -> Id<Self> {
        arena.alloc(Self {
            name,
            identifiers: Default::default(),
            references: Default::default(),
            defs: Default::default(),
            tainted: Default::default(),
            stack: true,
            scope,
        })
    }
}

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
