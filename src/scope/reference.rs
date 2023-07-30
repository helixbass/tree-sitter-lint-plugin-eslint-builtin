use bitflags::bitflags;
use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use super::{arena::AllArenas, referencer::PatternAndNode, scope::Scope, variable::Variable};

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
    identifier: Node<'a>,
    from: Id<Scope<'a>>,
    tainted: bool,
    resolved: Option<Id<Variable<'a>>>,
    flag: ReadWriteFlags,
    write_expr: Option<Node<'a>>,
    partial: bool,
    init: bool,
    __maybe_implicit_global: Option<PatternAndNode<'a>>,
}

impl<'a> Reference<'a> {
    pub fn new(
        arena: &mut Arena<Self>,
        ident: Node<'a>,
        scope: Id<Scope<'a>>,
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
            partial,
            init,
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
}
