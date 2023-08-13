mod arena;
mod definition;
mod pattern_visitor;
mod reference;
mod referencer;
mod scope;
mod scope_manager;
mod variable;

use tree_sitter_lint::tree_sitter::Tree;

use crate::visit::Visit;
use referencer::Referencer;
use scope_manager::ScopeManager;

pub fn analyze<'a>(tree: &'a Tree, source_text: &'a [u8]) -> ScopeManager<'a> {
    let mut scope_manager = ScopeManager::new(source_text);
    let mut referencer = Referencer::new(&mut scope_manager);

    referencer.visit_program(tree.root_node());

    assert!(
        scope_manager.maybe_current_scope().is_none(),
        "current_scope should be null."
    );

    scope_manager
}
