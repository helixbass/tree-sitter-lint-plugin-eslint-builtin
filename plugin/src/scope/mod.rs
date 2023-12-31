use tree_sitter_lint::{tree_sitter::Tree, tree_sitter_grep::RopeOrSlice};

use crate::visit::Visit;

mod arena;
pub mod config_comment_parser;
mod definition;
mod pattern_visitor;
mod reference;
mod referencer;
#[allow(clippy::module_inception)]
mod scope;
mod scope_manager;
mod variable;

pub use definition::Definition;
pub use reference::Reference;
use referencer::Referencer;
pub use scope::{Scope, ScopeType};
pub use scope_manager::{
    EcmaVersion, ScopeManager, ScopeManagerOptions, ScopeManagerOptionsBuilder, SourceType,
};
pub use variable::{Variable, VariableType};

pub fn analyze<'a>(
    tree: &'a Tree,
    source_text: impl Into<RopeOrSlice<'a>>,
    options: ScopeManagerOptions,
) -> ScopeManager<'a> {
    let source_text = source_text.into();

    let mut scope_manager = ScopeManager::new(source_text, options.clone());
    let mut referencer = Referencer::new(options, &mut scope_manager);

    referencer.visit(tree.root_node());

    assert!(
        scope_manager.maybe_current_scope().is_none(),
        "current_scope should be null."
    );

    scope_manager
}
