use std::borrow::Cow;

use tree_sitter_lint::{tree_sitter::Node, NodeExt, QueryMatchContext};

use crate::{
    kind::{
        ComputedPropertyName, MemberExpression, Pair, PrivatePropertyIdentifier,
        SubscriptExpression,
    },
    scope::Scope,
};

pub fn get_property_name<'a>(
    node: Node<'a>,
    initial_scope: Scope<'a, '_>,
    context: &QueryMatchContext<'a, '_>,
) -> Option<Cow<'a, str>> {
    match node.kind() {
        SubscriptExpression => get_string_if_constant(node.field("index"), initial_scope, context),
        MemberExpression => {
            let property = node.field("property");
            if property.kind() == PrivatePropertyIdentifier {
                return None;
            }
            Some(property.text(context))
        }
        Pair => {
            let key = node.field("key");
            if key.kind() == ComputedPropertyName {
                return get_string_if_constant(key, initial_scope, context);
            }
        }
    }
}
