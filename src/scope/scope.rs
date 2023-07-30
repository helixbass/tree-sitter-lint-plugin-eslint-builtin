use std::collections::HashMap;

use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use crate::kind::Identifier;

use super::{
    reference::{ReadWriteFlags, Reference},
    referencer::PatternAndNode,
    scope_manager::ScopeManager,
    variable::Variable,
};

fn is_strict_scope(
    scope_upper: Option<Id<Scope>>,
    scope_type: ScopeType,
    block: Node,
    is_method_definition: bool,
    use_directive: bool,
) -> bool {
    unimplemented!()
}

fn register_scope<'a>(scope_manager: &mut ScopeManager<'a>, scope: Id<Scope<'a>>) {
    scope_manager.scopes.push(scope);

    scope_manager
        .__node_to_scope
        .entry(
            scope_manager
                .arena
                .scopes
                .borrow()
                .get(scope)
                .unwrap()
                .block()
                .id(),
        )
        .or_default()
        .push(scope);
}

pub enum Scope<'a> {
    Base(ScopeBase<'a>),
}

impl<'a> Scope<'a> {
    pub fn new_base(
        scope_manager: &mut ScopeManager<'a>,
        type_: ScopeType,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
        is_method_definition: bool,
    ) -> Id<Self> {
        let id = {
            let mut arena = scope_manager.arena.scopes.borrow_mut();
            let variable_scope_or_waiting_to_grab_id = match type_ {
                ScopeType::Global
                | ScopeType::Module
                | ScopeType::Function
                | ScopeType::ClassFieldInitializer
                | ScopeType::ClassStaticBlock => None,
                _ => Some(arena.get(upper_scope.unwrap()).unwrap().variable_scope()),
            };
            let id = arena.alloc_with_id(|id| {
                Self::Base(ScopeBase {
                    id,
                    type_,
                    set: Default::default(),
                    taints: Default::default(),
                    dynamic: matches!(type_, ScopeType::Global | ScopeType::With),
                    block,
                    through: Default::default(),
                    variables: Default::default(),
                    references: Default::default(),
                    variable_scope: variable_scope_or_waiting_to_grab_id.unwrap_or(id),
                    function_expression_scope: Default::default(),
                    direct_call_to_eval_scope: Default::default(),
                    this_found: Default::default(),
                    __left: Default::default(),
                    upper: upper_scope,
                    is_strict: if scope_manager.is_strict_mode_supported() {
                        is_strict_scope(
                            upper_scope,
                            type_,
                            block,
                            is_method_definition,
                            scope_manager.__use_directive(),
                        )
                    } else {
                        false
                    },
                    child_scopes: Default::default(),
                    // this.__declaredVariables = scopeManager.__declaredVariables
                })
            });

            if let Some(upper_scope) = upper_scope {
                arena
                    .get_mut(upper_scope)
                    .unwrap()
                    .child_scopes_mut()
                    .push(id);
            }

            id
        };

        register_scope(scope_manager, id);

        id
    }

    pub fn new_catch_scope(
        scope_manager: &mut ScopeManager<'a>,
        upper_scope: Option<Id<Scope<'a>>>,
        block: Node<'a>,
    ) -> Id<Self> {
        Self::new_base(scope_manager, ScopeType::Catch, upper_scope, block, false)
    }

    pub fn is_strict(&self) -> bool {
        unimplemented!()
    }

    pub fn id(&self) -> Id<Self> {
        unimplemented!()
    }

    pub fn __referencing(
        &mut self,
        arena: &mut Arena<Reference<'a>>,
        node: Node<'a>,
        assign: ReadWriteFlags,
        write_expr: Option<Node<'a>>,
        maybe_implicit_global: Option<PatternAndNode<'a>>,
        partial: bool,
        init: bool,
    ) {
        if node.kind() != Identifier {
            return;
        }

        let ref_ = Reference::new(
            arena,
            node,
            self.id(),
            if assign == ReadWriteFlags::NONE {
                ReadWriteFlags::READ
            } else {
                assign
            },
            write_expr,
            maybe_implicit_global,
            partial,
            init,
        );
    }

    pub fn is_static(&self) -> bool {
        unimplemented!()
    }

    pub fn block(&self) -> Node {
        unimplemented!()
    }

    pub fn child_scopes_mut(&mut self) -> &mut Vec<Id<Self>> {
        unimplemented!()
    }

    pub fn variable_scope(&self) -> Id<Self> {
        unimplemented!()
    }

    pub fn type_(&self) -> ScopeType {
        unimplemented!()
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScopeType {
    Global,
    Module,
    Function,
    FunctionExpressionName,
    Block,
    Switch,
    Catch,
    With,
    For,
    Class,
    ClassFieldInitializer,
    ClassStaticBlock,
}

pub struct ScopeBase<'a> {
    id: Id<Scope<'a>>,
    type_: ScopeType,
    set: HashMap<String, Id<Variable<'a>>>,
    taints: HashMap<String, bool>,
    dynamic: bool,
    block: Node<'a>,
    through: Vec<Id<Reference<'a>>>,
    variables: Vec<Id<Variable<'a>>>,
    references: Vec<Id<Reference<'a>>>,
    variable_scope: Id<Scope<'a>>,
    function_expression_scope: bool,
    direct_call_to_eval_scope: bool,
    this_found: bool,
    __left: Vec<Id<Reference<'a>>>,
    upper: Option<Id<Scope<'a>>>,
    is_strict: bool,
    child_scopes: Vec<Id<Scope<'a>>>,
}

impl<'a> ScopeBase<'a> {}
