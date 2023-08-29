use tree_sitter_lint::tree_sitter::Node;

use crate::scope::Scope;

pub fn get_innermost_scope<'a, 'b>(initial_scope: &Scope<'a, 'b>, node: Node<'a>) -> Scope<'a, 'b> {
    let location = node.range().start_byte;

    let mut scope = initial_scope.clone();
    let mut next_scope: Option<Scope> = Default::default();
    'outer: loop {
        if let Some(next_scope) = next_scope {
            scope = next_scope;
        }
        for child_scope in scope.child_scopes() {
            let range = child_scope.block().range();

            if range.start_byte <= location && location < range.end_byte {
                // scope = child_scope;
                next_scope = Some(child_scope);
                continue 'outer;
            }
        }
        return scope;
    }
}
