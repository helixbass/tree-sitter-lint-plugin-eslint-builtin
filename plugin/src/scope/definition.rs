use std::cell::{Ref, RefCell};

use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::{variable::VariableType, ScopeManager};

#[derive(Debug)]
pub enum _Definition<'a> {
    Base(DefinitionBase<'a>),
    Parameter(ParameterDefinition<'a>),
}

impl<'a> _Definition<'a> {
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

#[derive(Debug)]
pub struct DefinitionBase<'a> {
    type_: VariableType,
    name: Node<'a>,
    node: Node<'a>,
    parent: Option<Node<'a>>,
    pub index: Option<usize>,
    pub kind: Option<String>,
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

#[derive(Debug)]
pub struct ParameterDefinition<'a> {
    base: DefinitionBase<'a>,
    pub rest: bool,
}

impl<'a> ParameterDefinition<'a> {
    pub fn new(name: Node<'a>, node: Node<'a>, index: Option<usize>, rest: bool) -> Self {
        Self {
            base: DefinitionBase::new(VariableType::Parameter, name, node, None, index, None),
            rest,
        }
    }
}

#[derive(Debug)]
pub struct Definition<'a, 'b> {
    definition: Ref<'b, _Definition<'a>>,
    #[allow(dead_code)]
    scope_manager: &'b ScopeManager<'a>,
}

impl<'a, 'b> Definition<'a, 'b> {
    pub fn new(definition: Ref<'b, _Definition<'a>>, scope_manager: &'b ScopeManager<'a>) -> Self {
        Self {
            definition,
            scope_manager,
        }
    }

    pub fn type_(&self) -> VariableType {
        self.definition.type_()
    }

    pub fn name(&self) -> Node<'a> {
        self.definition.name()
    }

    pub fn rest(&self) -> bool {
        match &*self.definition {
            _Definition::Parameter(value) => value.rest,
            _ => unreachable!(),
        }
    }

    pub fn node(&self) -> Node<'a> {
        self.definition.node()
    }
}
