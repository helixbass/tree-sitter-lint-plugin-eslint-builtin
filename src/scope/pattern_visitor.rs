use tree_sitter_lint::tree_sitter::Node;

use crate::{
    kind::{
        ArrayPattern, AssignmentPattern, Identifier, ObjectAssignmentPattern, ObjectPattern,
        RestElement, SpreadElement,
    },
    visit::Visit,
};

pub struct PatternVisitor<'a, TCallback> {
    root_pattern: Node<'a>,
    callback: TCallback,
    pub right_hand_nodes: Vec<Node<'a>>,
}

pub fn is_pattern(node: Node) -> bool {
    matches!(
        node.kind(),
        // TODO: Identifier looks a little suspicious here? Eg in
        // Referencer::AssignmentExpression() wouldn't it always do the this.visitPattern()
        // case?
        // TODO: maybe also need to be included here: PairPattern,
        // ShorthandPropertyIdentifierPattern, PropertyIdentifier?
        Identifier
            | ObjectPattern
            | ArrayPattern
            | SpreadElement
            | RestElement
            | AssignmentPattern
            | ObjectAssignmentPattern
    )
}

impl<'a, TCallback: FnMut((), ())> PatternVisitor<'a, TCallback> {
    pub fn new(
        // options,
        root_pattern: Node<'a>,
        callback: TCallback,
    ) -> Self {
        Self {
            root_pattern,
            callback,
            right_hand_nodes: Default::default(),
        }
    }
}

impl<'a, TCallback: FnMut((), ())> Visit<'a> for PatternVisitor<'a, TCallback> {}
