use std::{
    borrow::Cow,
    cell::{Ref, RefCell, RefMut},
    collections::{HashMap, HashSet},
    fmt, ops,
};

use derive_builder::Builder;
use id_arena::Id;
use itertools::{Either, Itertools};
use serde::Deserialize;
use squalid::{break_if_none, EverythingExt, NonEmpty};
use tracing::trace;
use tree_sitter_lint::{
    better_any::tid, tree_sitter::Node, tree_sitter_grep::RopeOrSlice, FileRunContext,
    FromFileRunContext, NodeExt, SourceTextProvider,
};

use super::{
    analyze,
    arena::AllArenas,
    definition::_Definition,
    reference::{Reference, _Reference},
    scope::{Scope, ScopeType, _Scope},
    variable::{Variable, _Variable},
    Definition,
};
use crate::{
    conf::globals::{self, Globals},
    directive_comments::DirectiveComments,
    kind::Program,
};

pub type NodeId = usize;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Script,
    Module,
    CommonJS,
}

pub type EcmaVersion = u32;

#[derive(Builder, Clone, Debug, Deserialize)]
#[builder(default, setter(strip_option))]
#[serde(default)]
pub struct ScopeManagerOptions {
    optimistic: bool,
    directive: bool,
    ignore_eval: bool,
    nodejs_scope: bool,
    implied_strict: bool,
    source_type: SourceType,
    ecma_version: EcmaVersion,
    globals: HashMap<Cow<'static, str>, globals::Visibility>,
    env: HashMap<String, bool>,
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
            globals: Default::default(),
            env: Default::default(),
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
    cached_scopes: RefCell<HashMap<NodeId, Id<_Scope<'a>>>>,
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
            cached_scopes: Default::default(),
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

    pub fn get_declared_variables<'b>(&'b self, node: Node) -> DeclaredVariables<'a, 'b> {
        DeclaredVariables::new(self, node)
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
        trace!(target: "scope_analysis", ?scope, type_ = ?self.arena.scopes.borrow()[scope].type_(), "nesting scope");

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

    pub fn get_scope<'b>(&'b self, mut node: Node<'a>) -> Scope<'a, 'b> {
        self.borrow_scope(
            *self
                .cached_scopes
                .borrow_mut()
                .entry(node.id())
                .or_insert_with(|| {
                    let inner = node.kind() != Program;

                    loop {
                        let scope = self.acquire(node, Some(inner));

                        if let Some(scope) = scope {
                            if scope.type_() == ScopeType::FunctionExpressionName {
                                return scope.child_scopes().next().unwrap().id();
                            }

                            return scope.id();
                        }

                        node = break_if_none!(node.parent());
                    }

                    self.scopes[0]
                }),
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
        let options: ScopeManagerOptions = serde_json::from_value(serde_json::Value::Object(
            file_run_context.environment.clone(),
        ))
        .unwrap();
        let scope_manager = analyze(
            file_run_context.tree,
            file_run_context.file_contents,
            options.clone(),
        );

        let comment_directives = file_run_context.retrieve::<DirectiveComments<'a>>();
        let mut configured_globals = get_globals_for_ecma_version(options.ecma_version);
        if options.source_type == SourceType::CommonJS {
            configured_globals.extend(globals::COMMONJS.clone());
        }
        let resolved_env_config = &options.env;
        let enabled_envs = resolved_env_config
            .keys()
            .filter(|&env_name| resolved_env_config[env_name])
            .filter_map(|env_name| get_env(env_name))
            .collect_vec();
        for enabled_env in enabled_envs {
            configured_globals.extend(enabled_env.into_iter().map(|(global_name, visibility)| (global_name.clone(), *visibility)));
        }
        configured_globals.extend(options.globals.clone());
        add_declared_globals(&scope_manager, &configured_globals, comment_directives);

        scope_manager
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
            // .field("cached_scopes", &self.cached_scopes)
            .finish()
    }
}

pub enum DeclaredVariables<'a, 'b> {
    Present(DeclaredVariablesPresent<'a, 'b>),
    Missing,
}

impl<'a, 'b> DeclaredVariables<'a, 'b> {
    pub fn new(scope_manager: &'b ScopeManager<'a>, node: Node) -> Self {
        let __declared_variables = scope_manager.__declared_variables.borrow();
        if __declared_variables.contains_key(&node.id()) {
            Self::Present(DeclaredVariablesPresent::new(
                Ref::map(__declared_variables, |__declared_variables| {
                    &__declared_variables[&node.id()]
                }),
                scope_manager,
            ))
        } else {
            Self::Missing
        }
    }
}

impl<'a, 'b> Iterator for DeclaredVariables<'a, 'b> {
    type Item = Variable<'a, 'b>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            DeclaredVariables::Present(value) => {
                if value.next_index >= value.variables.len() {
                    return None;
                }
                let ret = value
                    .scope_manager
                    .borrow_variable(value.variables[value.next_index]);
                value.next_index += 1;
                Some(ret)
            }
            DeclaredVariables::Missing => None,
        }
    }
}

pub struct DeclaredVariablesPresent<'a, 'b> {
    variables: Ref<'b, Vec<Id<_Variable<'a>>>>,
    scope_manager: &'b ScopeManager<'a>,
    next_index: usize,
}

impl<'a, 'b> DeclaredVariablesPresent<'a, 'b> {
    fn new(
        variables: Ref<'b, Vec<Id<_Variable<'a>>>>,
        scope_manager: &'b ScopeManager<'a>,
    ) -> Self {
        Self {
            variables,
            scope_manager,
            next_index: Default::default(),
        }
    }
}

fn add_declared_globals(
    scope_manager: &ScopeManager,
    config_globals: &Globals,
    comment_directives: &DirectiveComments,
) {
    let enabled_globals = &comment_directives.enabled_globals;
    let global_scope = &mut scope_manager.arena.scopes.borrow_mut()[scope_manager.scopes[0]];

    let mut keys: HashSet<&str> = config_globals.keys().map(|key| &**key).collect();
    for key in enabled_globals.keys() {
        keys.insert(&**key);
    }
    for id in keys {
        let value = enabled_globals
            .get(id)
            .map(|enabled_global| enabled_global.value)
            .unwrap_or_else(|| config_globals[id]);
        if value == globals::Visibility::Off {
            continue;
        }

        let mut did_insert = false;
        let global_scope_id = global_scope.id();
        let variable = if global_scope.set().contains_key(id) {
            *global_scope.set().get(id).unwrap()
        } else {
            *global_scope
                .set_mut()
                .entry(Cow::Owned(id.to_owned()))
                .or_insert_with(|| {
                    did_insert = true;
                    let variable = _Variable::new(
                        &mut scope_manager.arena.variables.borrow_mut(),
                        Cow::Owned(id.to_owned()),
                        global_scope_id,
                    );

                    scope_manager.arena.variables.borrow_mut()[variable].writeable =
                        Some(value == globals::Visibility::Writable);
                    variable
                })
        };
        if did_insert {
            global_scope.variables_mut().push(variable);
        }
    }

    let through = global_scope.through().to_owned();
    *global_scope.through_mut() = through
        .iter()
        .filter(|&&reference| {
            let reference = &mut scope_manager.arena.references.borrow_mut()[reference];
            let name = reference.identifier.text(scope_manager);
            let variable = global_scope.set().get(&name).copied();

            if let Some(variable) = variable {
                reference.resolved = Some(variable);
                scope_manager.arena.variables.borrow_mut()[variable]
                    .references
                    .push(reference.id);

                return false;
            }

            true
        })
        .copied()
        .collect();
}

fn get_globals_for_ecma_version(ecma_version: EcmaVersion) -> Globals {
    match ecma_version {
        3 => globals::ES3.clone(),
        5 => globals::ES5.clone(),
        6 => globals::ES2015.clone(),
        2015 => globals::ES2015.clone(),
        7 => globals::ES2016.clone(),
        2016 => globals::ES2016.clone(),
        8 => globals::ES2017.clone(),
        2017 => globals::ES2017.clone(),
        9 => globals::ES2018.clone(),
        2018 => globals::ES2018.clone(),
        10 => globals::ES2019.clone(),
        2019 => globals::ES2019.clone(),
        11 => globals::ES2020.clone(),
        2020 => globals::ES2020.clone(),
        12 => globals::ES2021.clone(),
        2021 => globals::ES2021.clone(),
        13 => globals::ES2022.clone(),
        2022 => globals::ES2022.clone(),
        14 => globals::ES2023.clone(),
        2023 => globals::ES2023.clone(),
        15 => globals::ES2024.clone(),
        2024 => globals::ES2024.clone(),
        _ => unreachable!(),
    }
}

fn get_env(env_name: &str) -> Option<&'static Globals> {
    match env_name {
        "builtin" => Some(&globals::ES5),
        "es6" => Some(&globals::ES2015),
        "es2015" => Some(&globals::ES2015),
        "es2016" => Some(&globals::ES2016),
        "es2017" => Some(&globals::ES2017),
        "es2018" => Some(&globals::ES2018),
        "es2019" => Some(&globals::ES2019),
        "es2020" => Some(&globals::ES2020),
        "es2021" => Some(&globals::ES2021),
        "es2022" => Some(&globals::ES2022),
        "es2023" => Some(&globals::ES2023),
        "es2024" => Some(&globals::ES2024),
        "browser" => Some(&globals::BROWSER),
        _ => None,
    }
}
