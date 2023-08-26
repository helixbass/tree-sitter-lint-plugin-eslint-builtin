use bitflags::bitflags;
use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::{arena::AllArenas, referencer::PatternAndNode, scope::_Scope, variable::_Variable};

bitflags! {
    #[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
    pub struct ReadWriteFlags: u32 {
        const NONE = 0;

        const READ = 0x1;
        const WRITE = 0x2;
        const RW = Self::READ.bits() | Self::WRITE.bits();
    }
}

pub struct Reference<'a> {
    pub identifier: Node<'a>,
    pub from: Id<_Scope<'a>>,
    pub tainted: bool,
    pub resolved: Option<Id<_Variable<'a>>>,
    flag: ReadWriteFlags,
    pub write_expr: Option<Node<'a>>,
    pub partial: bool,
    pub init: bool,
    pub __maybe_implicit_global: Option<PatternAndNode<'a>>,
}

impl<'a> Reference<'a> {
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
        arena.alloc(Self {
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
            init: if flag.intersects(ReadWriteFlags::WRITE) {
                init
            } else {
                false
            },
            __maybe_implicit_global: maybe_implicit_global,
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
