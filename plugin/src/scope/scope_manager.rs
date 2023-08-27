use std::{
    borrow::Cow,
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    fmt, ops,
};

use derive_builder::Builder;
use id_arena::Id;
use itertools::Either;
use squalid::{EverythingExt, NonEmpty};
use tracing::trace;
use tree_sitter_lint::{
    better_any::tid, tree_sitter::Node, tree_sitter_grep::RopeOrSlice, FileRunContext,
    FromFileRunContext, SourceTextProvider,
};

use super::{
    analyze,
    arena::AllArenas,
    reference::{Reference, _Reference},
    scope::{Scope, ScopeType, _Scope},
    variable::{Variable, _Variable}, Definition, definition::_Definition,
};

pub type NodeId = usize;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SourceType {
    Script,
    Module,
    CommonJS,
}

pub type EcmaVersion = u32;

#[derive(Builder, Copy, Clone, Debug)]
#[builder(default, setter(strip_option))]
pub struct ScopeManagerOptions {
    optimistic: bool,
    directive: bool,
    ignore_eval: bool,
    nodejs_scope: bool,
    implied_strict: bool,
    source_type: SourceType,
    ecma_version: EcmaVersion,
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
    pub scopes: Vec<Id<_Scope<'a>>>,
    global_scope: Option<Id<_Scope<'a>>>,
    pub __node_to_scope: HashMap<NodeId, Vec<Id<_Scope<'a>>>>,
    pub __current_scope: Option<Id<_Scope<'a>>>,
    pub arena: AllArenas<'a>,
    pub __declared_variables: RefCell<HashMap<NodeId, Vec<Id<_Variable<'a>>>>>,
    pub source_text: RopeOrSlice<'a>,
    __options: ScopeManagerOptions,
}

impl<'a> ScopeManager<'a> {
    pub fn new(source_text: RopeOrSlice<'a>, options: ScopeManagerOptions) -> Self {
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

    pub fn __get(&self, node: Node) -> Option<&Vec<Id<_Scope<'a>>>> {
        self.__node_to_scope.get(&node.id())
    }

    fn _get_declared_variables(&self, node: Node) -> Option<Vec<Id<_Variable<'a>>>> {
        self.__declared_variables.borrow().get(&node.id()).cloned()
    }

    pub fn get_declared_variables<'b>(&'b self, node: Node) -> Option<Vec<Variable<'a, 'b>>> {
        self._get_declared_variables(node).map(|declared_variables| {
            declared_variables
                .into_iter()
                .map(|variable| self.borrow_variable(variable))
                .collect()
        })
    }

    fn _acquire(&self, node: Node, inner: Option<bool>) -> Option<Id<_Scope<'a>>> {
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

    pub fn acquire<'b>(&'b self, node: Node, inner: Option<bool>) -> Option<Scope<'a, 'b>> {
        self._acquire(node, inner)
            .map(|scope| self.borrow_scope(scope))
    }

    pub fn acquire_all(&self, node: Node) -> Option<&Vec<Id<_Scope<'a>>>> {
        self.__get(node)
    }

    fn _release(&self, node: Node, inner: Option<bool>) -> Option<Id<_Scope<'a>>> {
        let scopes = self.__get(node).non_empty()?;

        let scope = self.arena.scopes.borrow()[scopes[0]].maybe_upper()?;
        self._acquire(self.arena.scopes.borrow()[scope].block(), inner)
    }

    pub fn release<'b>(&'b self, node: Node, inner: Option<bool>) -> Option<Scope<'a, 'b>> {
        self._release(node, inner)
            .map(|scope| self.borrow_scope(scope))
    }

    fn __nest_scope(&mut self, scope: Id<_Scope<'a>>) -> Id<_Scope<'a>> {
        trace!(?scope, "nesting scope");

        if self.arena.scopes.borrow()[scope].type_() == ScopeType::Global {
            assert!(self.__current_scope.is_none());
            self.global_scope = Some(scope);
        }
        self.__current_scope = Some(scope);
        scope
    }

    pub fn __nest_global_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_global_scope(self, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_block_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_block_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_function_scope(
        &mut self,
        node: Node<'a>,
        is_method_definition: bool,
    ) -> Id<_Scope<'a>> {
        let scope =
            _Scope::new_function_scope(self, self.__current_scope, node, is_method_definition);
        self.__nest_scope(scope)
    }

    pub fn __nest_for_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_for_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_catch_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_catch_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_with_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_with_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_class_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_field_initializer_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_class_field_initializer_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_class_static_block_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_class_static_block_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_switch_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_switch_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_module_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_module_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __nest_function_expression_name_scope(&mut self, node: Node<'a>) -> Id<_Scope<'a>> {
        let scope = _Scope::new_function_expression_name_scope(self, self.__current_scope, node);
        self.__nest_scope(scope)
    }

    pub fn __is_es6(&self) -> bool {
        self.__options.ecma_version >= 6
    }

    pub fn maybe_current_scope(&self) -> Option<Ref<_Scope<'a>>> {
        self.__current_scope.map(|__current_scope| {
            Ref::map(self.arena.scopes.borrow(), |scopes| {
                scopes.get(__current_scope).unwrap()
            })
        })
    }

    pub fn maybe_current_scope_mut(&self) -> Option<RefMut<_Scope<'a>>> {
        self.__current_scope.map(|__current_scope| {
            RefMut::map(self.arena.scopes.borrow_mut(), |scopes| {
                scopes.get_mut(__current_scope).unwrap()
            })
        })
    }

    pub fn __current_scope(&self) -> Ref<_Scope<'a>> {
        self.maybe_current_scope().unwrap()
    }

    pub fn __current_scope_mut(&self) -> RefMut<_Scope<'a>> {
        self.maybe_current_scope_mut().unwrap()
    }

    pub(crate) fn borrow_scope<'b>(&'b self, scope: Id<_Scope<'a>>) -> Scope<'a, 'b> {
        Scope::new(
            Ref::map(self.arena.scopes.borrow(), |scopes| &scopes[scope]),
            self,
        )
    }

    pub fn scopes<'b>(&'b self) -> impl Iterator<Item = Scope<'a, 'b>> {
        self.scopes.iter().map(|scope| self.borrow_scope(*scope))
    }

    pub(crate) fn borrow_variable<'b>(&'b self, variable: Id<_Variable<'a>>) -> Variable<'a, 'b> {
        Variable::new(
            Ref::map(self.arena.variables.borrow(), |variables| {
                &variables[variable]
            }),
            self,
        )
    }

    pub(crate) fn borrow_reference<'b>(
        &'b self,
        reference: Id<_Reference<'a>>,
    ) -> Reference<'a, 'b> {
        Reference::new(
            Ref::map(self.arena.references.borrow(), |references| {
                &references[reference]
            }),
            self,
        )
    }

    pub fn global_scope<'b>(&'b self) -> Scope<'a, 'b> {
        self.borrow_scope(self.global_scope.unwrap())
    }

    pub(crate) fn borrow_definition<'b>(
        &'b self,
        definition: Id<_Definition<'a>>,
    ) -> Definition<'a, 'b> {
        Definition::new(
            Ref::map(self.arena.definitions.borrow(), |definitions| {
                &definitions[definition]
            }),
            self,
        )
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

tid! { impl<'a> TidAble<'a> for ScopeManager<'a> }

impl<'a> FromFileRunContext<'a> for ScopeManager<'a> {
    fn from_file_run_context(file_run_context: FileRunContext<'a, '_>) -> Self {
        analyze(
            file_run_context.tree,
            file_run_context.file_contents,
            Default::default(),
        )
    }
}

impl<'a> fmt::Debug for ScopeManager<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopeManager")
            .field("scopes", &self.scopes)
            .field("global_scope", &self.global_scope)
            .field("__node_to_scope", &self.__node_to_scope)
            .field("__current_scope", &self.__current_scope)
            // .field("arena", &self.arena)
            .field("__declared_variables", &self.__declared_variables)
            .field("source_text", &self.source_text)
            .field("__options", &self.__options)
            .finish()
    }
}
