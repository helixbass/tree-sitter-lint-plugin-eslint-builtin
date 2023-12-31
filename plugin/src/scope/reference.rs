use std::cell::Ref;

use bitflags::bitflags;
use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::{
    arena::AllArenas,
    referencer::PatternAndNode,
    scope::{Scope, _Scope},
    variable::{Variable, _Variable},
    ScopeManager,
};

bitflags! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct ReadWriteFlags: u32 {
        const NONE = 0;

        const READ = 0x1;
        const WRITE = 0x2;
        const RW = Self::READ.bits() | Self::WRITE.bits();
    }
}

#[derive(Debug)]
pub struct _Reference<'a> {
    pub identifier: Node<'a>,
    pub from: Id<_Scope<'a>>,
    pub tainted: bool,
    pub resolved: Option<Id<_Variable<'a>>>,
    flag: ReadWriteFlags,
    pub write_expr: Option<Node<'a>>,
    pub partial: bool,
    pub init: Option<bool>,
    pub __maybe_implicit_global: Option<PatternAndNode<'a>>,
    pub id: Id<Self>,
}

impl<'a> _Reference<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        arena: &mut Arena<Self>,
        ident: Node<'a>,
        scope: Id<_Scope<'a>>,
        flag: ReadWriteFlags,
        write_expr: Option<Node<'a>>,
        maybe_implicit_global: Option<PatternAndNode<'a>>,
        partial: bool,
        init: bool,
    ) -> Id<Self> {
        arena.alloc_with_id(|id| Self {
            identifier: ident,
            from: scope,
            tainted: Default::default(),
            resolved: Default::default(),
            flag,
            write_expr: if flag.intersects(ReadWriteFlags::WRITE) {
                write_expr
            } else {
                None
            },
            partial: if flag.intersects(ReadWriteFlags::WRITE) {
                partial
            } else {
                false
            },
            init: flag.intersects(ReadWriteFlags::WRITE).then_some(init),
            __maybe_implicit_global: maybe_implicit_global,
            id,
        })
    }

    pub fn is_static(&self, arena: &AllArenas<'a>) -> bool {
        !self.tainted
            && matches!(
                self.resolved.as_ref(),
                Some(&resolved) if arena.get_scope(arena.get_variable(resolved).scope).is_static()
            )
    }

    pub fn is_write(&self) -> bool {
        self.flag.intersects(ReadWriteFlags::WRITE)
    }

    pub fn is_read(&self) -> bool {
        self.flag.intersects(ReadWriteFlags::READ)
    }

    pub fn is_read_only(&self) -> bool {
        self.flag == ReadWriteFlags::READ
    }

    pub fn is_write_only(&self) -> bool {
        self.flag == ReadWriteFlags::WRITE
    }

    pub fn is_read_write(&self) -> bool {
        self.flag == ReadWriteFlags::RW
    }
}

#[derive(Debug)]
pub struct Reference<'a, 'b> {
    reference: Ref<'b, _Reference<'a>>,
    scope_manager: &'b ScopeManager<'a>,
}

impl<'a, 'b> Reference<'a, 'b> {
    pub fn new(reference: Ref<'b, _Reference<'a>>, scope_manager: &'b ScopeManager<'a>) -> Self {
        Self {
            reference,
            scope_manager,
        }
    }

    pub fn resolved(&self) -> Option<Variable<'a, 'b>> {
        self.reference
            .resolved
            .map(|resolved| self.scope_manager.borrow_variable(resolved))
    }

    pub fn identifier(&self) -> Node<'a> {
        self.reference.identifier
    }

    pub fn from(&self) -> Scope<'a, 'b> {
        self.scope_manager.borrow_scope(self.reference.from)
    }

    pub fn is_write_only(&self) -> bool {
        self.reference.is_write_only()
    }

    pub fn is_read_only(&self) -> bool {
        self.reference.is_read_only()
    }

    pub fn write_expr(&self) -> Option<Node<'a>> {
        self.reference.write_expr
    }

    pub fn is_write(&self) -> bool {
        self.reference.is_write()
    }

    pub fn is_read(&self) -> bool {
        self.reference.is_read()
    }

    pub fn partial(&self) -> bool {
        self.reference.partial
    }

    pub fn init(&self) -> Option<bool> {
        self.reference.init
    }

    pub fn is_read_write(&self) -> bool {
        self.reference.is_read_write()
    }
}

impl<'a, 'b> PartialEq for Reference<'a, 'b> {
    fn eq(&self, other: &Self) -> bool {
        self.reference.id == other.reference.id
    }
}

impl<'a, 'b> Eq for Reference<'a, 'b> {}

impl<'a, 'b> Clone for Reference<'a, 'b> {
    fn clone(&self) -> Self {
        Self {
            reference: Ref::clone(&self.reference),
            scope_manager: self.scope_manager,
        }
    }
}
