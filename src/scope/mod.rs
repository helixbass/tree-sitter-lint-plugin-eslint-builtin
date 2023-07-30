mod pattern_visitor;
mod referencer;
mod scope;
mod scope_manager;

use tree_sitter_lint::tree_sitter::Tree;

use crate::visit::Visit;
use referencer::Referencer;
use scope_manager::ScopeManager;

pub fn analyze(tree: &Tree) -> ScopeManager {
    let mut scope_manager = ScopeManager::new();
    let mut referencer = Referencer::new(&mut scope_manager);

    referencer.visit_program(&mut tree.walk());

    assert!(
        scope_manager.__current_scope().is_none(),
        "current_scope should be null."
    );

    scope_manager
}
