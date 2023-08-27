use tree_sitter_lint::{tree_sitter::Tree, tree_sitter_grep::RopeOrSlice};

use crate::visit::Visit;

mod arena;
mod definition;
mod pattern_visitor;
mod reference;
mod referencer;
mod scope;
mod scope_manager;
mod variable;

use referencer::Referencer;
pub use scope::ScopeType;
pub use scope_manager::{
    EcmaVersion, ScopeManager, ScopeManagerOptions, ScopeManagerOptionsBuilder, SourceType,
};

pub fn analyze<'a>(
    tree: &'a Tree,
    source_text: impl Into<RopeOrSlice<'a>>,
    options: ScopeManagerOptions,
) -> ScopeManager<'a> {
    let source_text = source_text.into();

    let mut scope_manager = ScopeManager::new(source_text, options);
    let mut referencer = Referencer::new(options, &mut scope_manager);

    referencer.visit(tree.root_node());

    assert!(
        scope_manager.maybe_current_scope().is_none(),
        "current_scope should be null."
    );

    scope_manager
}
