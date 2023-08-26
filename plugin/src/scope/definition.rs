use std::cell::RefCell;

use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::variable::VariableType;

pub enum Definition<'a> {
    Base(DefinitionBase<'a>),
    Parameter(ParameterDefinition<'a>),
}

impl<'a> Definition<'a> {
    pub fn new(
        arena: &RefCell<Arena<Self>>,
        type_: VariableType,
        name: Node<'a>,
        node: Node<'a>,
        parent: Option<Node<'a>>,
        index: Option<usize>,
        kind: Option<String>,
    ) -> Id<Self> {
        arena.borrow_mut().alloc(Self::Base(DefinitionBase::new(
            type_, name, node, parent, index, kind,
        )))
    }

    pub fn new_parameter(
        arena: &RefCell<Arena<Self>>,
        name: Node<'a>,
        node: Node<'a>,
        index: Option<usize>,
        rest: bool,
    ) -> Id<Self> {
        arena
            .borrow_mut()
            .alloc(Self::Parameter(ParameterDefinition::new(
                name, node, index, rest,
            )))
    }

    pub fn node(&self) -> Node<'a> {
        match self {
            Self::Base(value) => value.node,
            Self::Parameter(value) => value.base.node,
        }
    }

    pub fn parent(&self) -> Option<Node<'a>> {
        match self {
            Self::Base(value) => value.parent,
            Self::Parameter(value) => value.base.parent,
        }
    }

    pub fn type_(&self) -> VariableType {
        match self {
            Self::Base(value) => value.type_,
            Self::Parameter(value) => value.base.type_,
        }
    }

    pub fn name(&self) -> Node<'a> {
        match self {
            Self::Base(value) => value.name,
            Self::Parameter(value) => value.base.name,
        }
    }
}

pub struct DefinitionBase<'a> {
    type_: VariableType,
    name: Node<'a>,
    node: Node<'a>,
    parent: Option<Node<'a>>,
    index: Option<usize>,
    kind: Option<String>,
}

impl<'a> DefinitionBase<'a> {
    pub fn new(
        type_: VariableType,
        name: Node<'a>,
        node: Node<'a>,
        parent: Option<Node<'a>>,
        index: Option<usize>,
        kind: Option<String>,
    ) -> Self {
        Self {
            type_,
            name,
            node,
            parent,
            index,
            kind,
        }
    }
}

pub struct ParameterDefinition<'a> {
    base: DefinitionBase<'a>,
    rest: bool,
}

impl<'a> ParameterDefinition<'a> {
    pub fn new(name: Node<'a>, node: Node<'a>, index: Option<usize>, rest: bool) -> Self {
        Self {
            base: DefinitionBase::new(VariableType::Parameter, name, node, None, index, None),
            rest,
        }
    }
}
