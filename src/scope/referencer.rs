use crate::visit::Visit;

use super::scope_manager::ScopeManager;

pub struct Referencer<'a> {
    scope_manager: &'a mut ScopeManager,
}

impl<'a> Referencer<'a> {
    pub fn new(scope_manager: &'a mut ScopeManager) -> Self {
        Self { scope_manager }
    }
}

impl<'a> Visit<'a> for Referencer<'a> {}
