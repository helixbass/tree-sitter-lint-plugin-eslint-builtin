use std::ops::Deref;

use tree_sitter_lint::{
    better_any::tid, tree_sitter::Node, FileRunContext, FromFileRunContext, NodeExt,
};

use crate::kind::Comment;

pub struct AllComments<'a>(Vec<Node<'a>>);

tid! { impl<'a> TidAble<'a> for AllComments<'a> }

impl<'a> FromFileRunContext<'a> for AllComments<'a> {
    fn from_file_run_context(file_run_context: FileRunContext<'a, '_>) -> Self {
        AllComments(
            file_run_context
                .tree
                .root_node()
                .tokens()
                .filter(|token| token.kind() == Comment)
                .collect(),
        )
    }
}

impl<'a> Deref for AllComments<'a> {
    type Target = Vec<Node<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
