use tree_sitter_lint::{tree_sitter::Node, SourceTextProvider, NodeExt};

use crate::scope::{Scope, Variable};

use super::get_innermost_scope;

pub fn find_variable<'a, 'b, 'c>(
    initial_scope: &Scope<'a, 'b>,
    name_or_node: impl Into<NodeOrStr<'a>>,
    source_text_provider: &impl SourceTextProvider<'c>,
) -> Option<Variable<'a, 'b>> {
    let name_or_node = name_or_node.into();

    let (name, mut scope) = match name_or_node {
        NodeOrStr::Str(value) => (value.into(), initial_scope.clone()),
        NodeOrStr::Node(value) => (
            value.text(source_text_provider),
            get_innermost_scope(initial_scope, value),
        ),
    };

    loop {
        if let Some(variable) = scope.set().get(&name) {
            return Some(variable.clone());
        }
        scope = scope.maybe_upper()?;
    }
}

pub enum NodeOrStr<'a> {
    Node(Node<'a>),
    Str(&'a str),
}

impl<'a> From<Node<'a>> for NodeOrStr<'a> {
    fn from(value: Node<'a>) -> Self {
        Self::Node(value)
    }
}

impl<'a> From<&'a str> for NodeOrStr<'a> {
    fn from(value: &'a str) -> Self {
        Self::Str(value)
    }
}
