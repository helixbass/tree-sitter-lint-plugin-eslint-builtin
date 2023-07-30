use super::scope::Scope;

pub struct ScopeManager {
    scopes: Vec<Scope>,
    __current_scope_index: Option<usize>,
}

impl ScopeManager {
    pub fn new() -> Self {
        Self {
            scopes: Default::default(),
            __current_scope_index: Default::default(),
        }
    }

    pub fn __current_scope(&self) -> Option<&Scope> {
        self.__current_scope_index
            .map(|__current_scope_index| &self.scopes[__current_scope_index])
    }
}
