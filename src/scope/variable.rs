use std::borrow::Cow;

use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::{definition::Definition, reference::Reference, scope::Scope};

pub struct Variable<'a> {
    pub name: Cow<'a, str>,
    pub identifiers: Vec<Node<'a>>,
    pub references: Vec<Id<Reference<'a>>>,
    pub defs: Vec<Id<Definition<'a>>>,
    pub tainted: bool,
    pub stack: bool,
    pub scope: Id<Scope<'a>>,
}

impl<'a> Variable<'a> {
    pub fn new(arena: &mut Arena<Self>, name: Cow<'a, str>, scope: Id<Scope<'a>>) -> Id<Self> {
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
