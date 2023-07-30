use id_arena::{Arena, Id};
use tree_sitter_lint::tree_sitter::Node;

use crate::kind::Identifier;

use super::{
    arena::AllArenas,
    reference::{ReadWriteFlags, Reference},
    referencer::PatternAndNode,
};

pub enum Scope {}

impl Scope {
    pub fn is_strict(&self) -> bool {
        unimplemented!()
    }

    pub fn id(&self) -> Id<Self> {
        unimplemented!()
    }

    pub fn __referencing<'a>(
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
}
