use std::{
    borrow::Cow,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    ops,
};

use derive_builder::Builder;
use id_arena::Id;
use itertools::Either;
use squalid::{EverythingExt, NonEmpty};
use tree_sitter_lint::{tree_sitter::Node, SourceTextProvider};

use super::{
    arena::AllArenas,
    scope::{Scope, ScopeType},
    variable::Variable,
};

pub type NodeId = usize;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SourceType {
    Script,
    Module,
    CommonJS,
}

#[derive(Builder)]
#[builder(default, setter(strip_option))]
pub struct ScopeManagerOptions {
    optimistic: bool,
    directive: bool,
    ignore_eval: bool,
    nodejs_scope: bool,
    implied_strict: bool,
    source_type: SourceType,
    ecma_version: u32,
    // child_visitor_keys: Option<HashMap<String, Vec<String>>>,
    // fallback:
}

impl Default for ScopeManagerOptions {
    fn default() -> Self {
        Self {
            optimistic: Default::default(),
            directive: Default::default(),
            nodejs_scope: Default::default(),
            implied_strict: Default::default(),
            source_type: SourceType::Script,
            ecma_version: 5,
            ignore_eval: Default::default(),
        }
    }
}

pub struct ScopeManager<'a> {
    pub scopes: Vec<Id<Scope<'a>>>,
    global_scope: Option<Id<Scope<'a>>>,
    pub __node_to_scope: HashMap<NodeId, Vec<Id<Scope<'a>>>>,
    pub __current_scope: Option<Id<Scope<'a>>>,
    pub arena: AllArenas<'a>,
    pub __declared_variables: RefCell<HashMap<NodeId, Vec<Id<Variable<'a>>>>>,
    pub source_text: &'a [u8],
    __options: ScopeManagerOptions,
}

impl<'a> ScopeManager<'a> {
    pub fn new(source_text: &'a [u8], options: ScopeManagerOptions) -> Self {
        Self {
            scopes: Default::default(),
            global_scope: Default::default(),
            __node_to_scope: Default::default(),
            __current_scope: Default::default(),
            arena: Default::default(),
            __declared_variables: Default::default(),
            source_text,
            __options: options,
        }
    }

    pub fn __is_optimistic(&self) -> bool {
        self.__options.optimistic
    }

    pub fn __ignore_eval(&self) -> bool {
        self.__options.ignore_eval
    }

    pub fn is_global_return(&self) -> bool {
        self.__options.nodejs_scope || self.__options.source_type == SourceType::CommonJS
    }

    pub fn is_module(&self) -> bool {
        self.__options.source_type == SourceType::Module
    }

    pub fn is_implied_strict(&self) -> bool {
        self.__options.implied_strict
    }

    pub fn is_strict_mode_supported(&self) -> bool {
        self.__options.ecma_version >= 5
    }

    pub fn __get(&self, node: Node) -> Option<&Vec<Id<Scope<'a>>>> {
        self.__node_to_scope.get(&node.id())
    }

    pub fn get_declared_variables(&self, node: Node) -> Option<Vec<Id<Variable<'a>>>> {
        self.__declared_variables.borrow().get(&node.id()).cloned()
    }

    pub fn acquire(&self, node: Node, inner: Option<bool>) -> Option<Id<Scope<'a>>> {
        let scopes = self.__get(node).non_empty()?;

        if scopes.len() == 1 {
            return Some(scopes[0]);
        }

        if inner == Some(true) {
            Either::Left(scopes.into_iter().rev())
        } else {
            Either::Right(scopes.into_iter())
        }
        .find(|&&scope| {
            (&self.arena.scopes.borrow()[scope]).thrush(|scope| {
                !(scope.type_() == ScopeType::Function && scope.function_expression_scope())
            })
        })
        .copied()
    }

    pub fn acquire_all(&self, node: Node) -> Option<&Vec<Id<Scope<'a>>>> {
        self.__get(node)
    }

    pub fn release(&self, node: Node, inner: Option<bool>) -> Option<Id<Scope<'a>>> {
        let scopes = self.__get(node).non_empty()?;

        let scope = self.arena.scopes.borrow()[scopes[0]].maybe_upper()?;
        self.acquire(self.arena.scopes.borrow()[scope].block(), inner)
    }

    fn __nest_scope(&mut self, scope: Id<Scope<'a>>) -> Id<Scope<'a>> {
        if self.arena.scopes.borrow()[scope].type_() == ScopeType::Global {
            assert!(self.__current_scope.is_none());
            self.global_scope = Some(scope);
        }
        self.__current_scope = Some(scope);
        scope
    }

    pub fn __nest_global_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_global_scope(self, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_block_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_block_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_function_scope(
        &mut self,
        node: Node<'a>,
        is_method_definition: bool,
    ) -> Id<Scope<'a>> {
        let scope =
            Scope::new_function_scope(self, self.__current_scope, node, is_method_definition);
        self.__nest_scope(scope)
    }

    pub fn __nest_for_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_for_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_catch_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_catch_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_with_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_with_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_class_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_field_initializer_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_class_field_initializer_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_static_block_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_class_static_block_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_switch_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_switch_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_module_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_module_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_function_expression_name_scope(&mut self, node: Node<'a>) -> Id<Scope<'a>> {
        let scope = Scope::new_function_expression_name_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __is_es6(&self) -> bool {
        self.__options.ecma_version >= 6
    }

    pub fn maybe_current_scope(&self) -> Option<Ref<Scope<'a>>> {
        self.__current_scope.map(|__current_scope| {
            Ref::map(self.arena.scopes.borrow(), |scopes| {
                scopes.get(__current_scope).unwrap()
            })
        })
    }

    pub fn maybe_current_scope_mut(&self) -> Option<RefMut<Scope<'a>>> {
        self.__current_scope.map(|__current_scope| {
            RefMut::map(self.arena.scopes.borrow_mut(), |scopes| {
                scopes.get_mut(__current_scope).unwrap()
            })
        })
    }

    pub fn __current_scope(&self) -> Ref<Scope<'a>> {
        self.maybe_current_scope().unwrap()
    }

    pub fn __current_scope_mut(&self) -> RefMut<Scope<'a>> {
        self.maybe_current_scope_mut().unwrap()
    }
}

impl<'a> SourceTextProvider<'a> for ScopeManager<'a> {
    fn node_text(&self, node: Node) -> Cow<'a, str> {
        self.source_text.node_text(node)
    }

    fn slice(&self, range: ops::Range<usize>) -> Cow<'a, str> {
        self.source_text.slice(range)
    }
}
