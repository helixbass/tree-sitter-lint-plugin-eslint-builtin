use tree_sitter_lint::tree_sitter::{Node, TreeCursor};

use crate::kind::{
    Comment, DebuggerStatement, ExportStatement, ExpressionStatement, FunctionDeclaration,
    GeneratorFunctionDeclaration, HashBangLine, ImportStatement, Program,
};

pub trait Visit<'a> {
    fn visit_program(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_program(self, cursor);
    }

    fn visit_export_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_export_statement(self, cursor);
    }

    fn visit_import_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_import_statement(self, cursor);
    }

    fn visit_debugger_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_debugger_statement(self, cursor);
    }

    fn visit_expression_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_expression_statement(self, cursor);
    }

    fn visit_function_declaration(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_function_declaration(self, cursor);
    }

    fn visit_generator_function_declaration(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_generator_function_declaration(self, cursor);
    }
}

macro_rules! assert_cursor_node_kind {
    ($cursor: expr, $kind:expr) => {
        debug_assert_eq!($cursor.node().kind(), $kind);
    };
}

macro_rules! return_if_false {
    ($expr:expr) => {
        if (!$expr) {
            return;
        }
    };
}

pub fn visit_program<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, Program);

    return_if_false!(cursor.goto_first_child());

    loop {
        let current_child = cursor.node();
        match current_child.kind() {
            HashBangLine => unimplemented!(),
            ExportStatement => visitor.visit_export_statement(cursor),
            ImportStatement => visitor.visit_import_statement(cursor),
            DebuggerStatement => visitor.visit_debugger_statement(cursor),
            ExpressionStatement => visitor.visit_expression_statement(cursor),
            FunctionDeclaration => visitor.visit_function_declaration(cursor),
            GeneratorFunctionDeclaration => visitor.visit_generator_function_declaration(cursor),
            Comment => unimplemented!(),
            _ => unreachable!(),
        }
        return_if_false!(cursor.goto_next_sibling());
    }
}

pub fn visit_export_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ExportStatement);
    unimplemented!()
}

pub fn visit_import_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ImportStatement);
    unimplemented!()
}

pub fn visit_debugger_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, DebuggerStatement);
    unimplemented!()
}

pub fn visit_expression_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ExpressionStatement);
    unimplemented!()
}

pub fn visit_function_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, FunctionDeclaration);
    unimplemented!()
}

pub fn visit_generator_function_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, GeneratorFunctionDeclaration);
    unimplemented!()
}
