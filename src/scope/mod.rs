mod referencer;
mod scope_manager;

use tree_sitter_lint::tree_sitter::Tree;

use crate::visit::Visit;
use referencer::Referencer;
use scope_manager::ScopeManager;

pub fn analyze(tree: &Tree) -> ScopeManager {
    let mut scope_manager = ScopeManager::new();
    let mut referencer = Referencer::new(&mut scope_manager);

    referencer.visit_program(&mut tree.walk());

    scope_manager
}
