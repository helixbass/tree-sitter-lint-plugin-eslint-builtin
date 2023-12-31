use squalid::OptionExt;
use tracing::instrument;
use tree_sitter_lint::{tree_sitter::Node, tree_sitter_grep::SupportedLanguage, NodeExt};

use super::scope_manager::ScopeManagerOptions;
use crate::{
    kind::{
        ArrayPattern, AssignmentPattern, ComputedPropertyName, Identifier, ObjectAssignmentPattern,
        ObjectPattern, RestPattern, SpreadElement, Undefined,
    },
    visit::Visit,
};

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
            | RestPattern
            | AssignmentPattern
            | ObjectAssignmentPattern
            | Undefined
    )
}

pub struct PatternVisitor<'a, TCallback> {
    #[allow(dead_code)]
    options: ScopeManagerOptions,
    root_pattern: Node<'a>,
    callback: TCallback,
    assignments: Vec<Node<'a>>,
    pub right_hand_nodes: Vec<Node<'a>>,
    rest_elements: Vec<Node<'a>>,
}

impl<'a, TCallback: FnMut(Node<'a>, PatternInfo<'a, '_>)> PatternVisitor<'a, TCallback> {
    pub fn new(options: ScopeManagerOptions, root_pattern: Node<'a>, callback: TCallback) -> Self {
        Self {
            options,
            root_pattern,
            callback,
            assignments: Default::default(),
            right_hand_nodes: Default::default(),
            rest_elements: Default::default(),
        }
    }

    #[instrument(target = "scope_analysis", level = "trace", skip(self))]
    fn _visit_identifier(&mut self, pattern: Node<'a>) {
        let last_rest_element = self.rest_elements.last().copied();

        (self.callback)(
            pattern,
            PatternInfo {
                top_level: pattern == self.root_pattern,
                rest: last_rest_element.matches(|last_rest_element| {
                    last_rest_element.first_non_comment_named_child(SupportedLanguage::Javascript)
                        == pattern
                }),
                assignments: &self.assignments,
            },
        );
    }

    fn _visit_assignment_pattern(&mut self, pattern: Node<'a>) {
        self.assignments.push(pattern);
        self.visit(pattern.field("left"));
        self.right_hand_nodes.push(pattern.field("right"));
        self.assignments.pop().unwrap();
    }
}

impl<'a, TCallback: FnMut(Node<'a>, PatternInfo<'a, '_>)> Visit<'a>
    for PatternVisitor<'a, TCallback>
{
    fn visit_identifier(&mut self, pattern: Node<'a>) {
        self._visit_identifier(pattern);
    }

    fn visit_shorthand_property_identifier_pattern(&mut self, pattern: Node<'a>) {
        self._visit_identifier(pattern);
    }

    fn visit_undefined(&mut self, pattern: Node<'a>) {
        self._visit_identifier(pattern);
    }

    fn visit_pair_pattern(&mut self, property: Node<'a>) {
        let key = property.field("key");
        if key.kind() == ComputedPropertyName {
            self.right_hand_nodes.push(key);
        }

        self.visit(property.field("value"));
    }

    fn visit_array_pattern(&mut self, pattern: Node<'a>) {
        for element in pattern.non_comment_named_children(SupportedLanguage::Javascript) {
            self.visit(element);
        }
    }

    fn visit_assignment_pattern(&mut self, pattern: Node<'a>) {
        self._visit_assignment_pattern(pattern);
    }

    fn visit_object_assignment_pattern(&mut self, pattern: Node<'a>) {
        self._visit_assignment_pattern(pattern);
    }

    fn visit_rest_pattern(&mut self, pattern: Node<'a>) {
        self.rest_elements.push(pattern);
        self.visit(pattern.first_non_comment_named_child(SupportedLanguage::Javascript));
        self.rest_elements.pop().unwrap();
    }

    fn visit_member_expression(&mut self, node: Node<'a>) {
        self.right_hand_nodes.push(node.field("object"));
    }

    fn visit_subscript_expression(&mut self, node: Node<'a>) {
        self.right_hand_nodes.push(node.field("index"));

        self.right_hand_nodes.push(node.field("object"));
    }

    fn visit_spread_element(&mut self, node: Node<'a>) {
        self.visit(node.first_non_comment_named_child(SupportedLanguage::Javascript));
    }

    fn visit_array(&mut self, node: Node<'a>) {
        for child in node.non_comment_named_children(SupportedLanguage::Javascript) {
            self.visit(child);
        }
    }

    fn visit_assignment_expression(&mut self, node: Node<'a>) {
        self.assignments.push(node);
        self.visit(node.field("left"));
        self.right_hand_nodes.push(node.field("right"));
        self.assignments.pop().unwrap();
    }

    fn visit_call_expression(&mut self, node: Node<'a>) {
        node.field("arguments")
            .non_comment_named_children(SupportedLanguage::Javascript)
            .for_each(|a| {
                self.right_hand_nodes.push(a);
            });
        self.visit(node.field("function"));
    }
}

pub struct PatternInfo<'a, 'b> {
    pub top_level: bool,
    pub rest: bool,
    pub assignments: &'b [Node<'a>],
}
