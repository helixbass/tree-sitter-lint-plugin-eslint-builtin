mod arena;
mod definition;
mod pattern_visitor;
mod reference;
mod referencer;
mod scope;
mod scope_manager;
mod variable;

use referencer::Referencer;
use scope_manager::ScopeManager;
use tree_sitter_lint::tree_sitter::Tree;

use self::scope_manager::ScopeManagerOptions;
use crate::visit::Visit;

pub fn analyze<'a>(
    tree: &'a Tree,
    source_text: &'a [u8],
    options: ScopeManagerOptions,
) -> ScopeManager<'a> {
    let mut scope_manager = ScopeManager::new(source_text, options);
    let mut referencer = Referencer::new(&mut scope_manager);

    referencer.visit_program(tree.root_node());

    assert!(
        scope_manager.maybe_current_scope().is_none(),
        "current_scope should be null."
    );

    scope_manager
}
