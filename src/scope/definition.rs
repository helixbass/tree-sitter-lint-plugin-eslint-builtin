use tree_sitter_lint::tree_sitter::Node;

use super::variable::VariableType;

pub enum Definition<'a> {
    Base(DefinitionBase<'a>),
    Parameter(ParameterDefinition<'a>),
}

impl<'a> Definition<'a> {
    pub fn new(
        type_: VariableType,
        name: Node<'a>,
        node: Node<'a>,
        parent: Option<Node<'a>>,
        index: Option<usize>,
        kind: Option<String>,
    ) -> Self {
        Self::Base(DefinitionBase::new(type_, name, node, parent, index, kind))
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
