use tree_sitter_lint::tree_sitter::{Node, TreeCursor};

use crate::kind::{
    is_declaration_kind, is_statement_kind, AssignmentExpression, AugmentedAssignmentExpression,
    BreakStatement, CatchClause, ClassDeclaration, Comment, ContinueStatement, DebuggerStatement,
    DoStatement, EmptyStatement, ExportStatement, ExpressionStatement, ForInStatement,
    ForStatement, FunctionDeclaration, GeneratorFunctionDeclaration, HashBangLine, IfStatement,
    ImportStatement, LabeledStatement, LexicalDeclaration, Program, ReturnStatement,
    StatementBlock, SwitchStatement, ThrowStatement, TryStatement, VariableDeclaration,
    WhileStatement, WithStatement,
};

pub trait Visit<'a> {
    fn visit_program(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_program(self, cursor);
    }

    fn visit_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_statement(self, cursor);
    }

    fn visit_declaration(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_declaration(self, cursor);
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

    fn visit_class_declaration(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_class_declaration(self, cursor);
    }

    fn visit_lexical_declaration(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_lexical_declaration(self, cursor);
    }

    fn visit_variable_declaration(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_variable_declaration(self, cursor);
    }

    fn visit_statement_block(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_statement_block(self, cursor);
    }

    fn visit_if_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_if_statement(self, cursor);
    }

    fn visit_switch_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_switch_statement(self, cursor);
    }

    fn visit_for_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_for_statement(self, cursor);
    }

    fn visit_for_in_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_for_in_statement(self, cursor);
    }

    fn visit_while_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_while_statement(self, cursor);
    }

    fn visit_do_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_do_statement(self, cursor);
    }

    fn visit_try_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_try_statement(self, cursor);
    }

    fn visit_with_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_with_statement(self, cursor);
    }

    fn visit_break_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_break_statement(self, cursor);
    }

    fn visit_continue_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_continue_statement(self, cursor);
    }

    fn visit_return_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_return_statement(self, cursor);
    }

    fn visit_throw_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_throw_statement(self, cursor);
    }

    fn visit_empty_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_empty_statement(self, cursor);
    }

    fn visit_labeled_statement(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_labeled_statement(self, cursor);
    }

    fn visit_assignment_expression(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_assignment_expression(self, cursor);
    }

    fn visit_augmented_assignment_expression(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_augmented_assignment_expression(self, cursor);
    }

    fn visit_expression(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_expression(self, cursor);
    }

    fn visit_catch_clause(&mut self, cursor: &mut TreeCursor<'a>) {
        visit_catch_clause(self, cursor);
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

pub fn visit<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    let mut cursor = node.walk();
    match node.kind() {
        Program => visit_program(visitor, &mut cursor),
        _ => unimplemented!(),
    }
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
            kind if is_statement_kind(kind) => visitor.visit_statement(cursor),
            Comment => unimplemented!(),
            _ => unreachable!(),
        }
        return_if_false!(cursor.goto_next_sibling());
    }
}

pub fn visit_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    debug_assert!(is_statement_kind(cursor.node().kind()));

    match cursor.node().kind() {
        ExportStatement => visitor.visit_export_statement(cursor),
        ImportStatement => visitor.visit_import_statement(cursor),
        DebuggerStatement => visitor.visit_debugger_statement(cursor),
        ExpressionStatement => visitor.visit_expression_statement(cursor),
        kind if is_declaration_kind(kind) => visitor.visit_declaration(cursor),
        StatementBlock => visitor.visit_statement_block(cursor),
        IfStatement => visitor.visit_if_statement(cursor),
        SwitchStatement => visitor.visit_switch_statement(cursor),
        ForStatement => visitor.visit_for_statement(cursor),
        ForInStatement => visitor.visit_for_in_statement(cursor),
        WhileStatement => visitor.visit_while_statement(cursor),
        DoStatement => visitor.visit_do_statement(cursor),
        TryStatement => visitor.visit_try_statement(cursor),
        WithStatement => visitor.visit_with_statement(cursor),
        BreakStatement => visitor.visit_break_statement(cursor),
        ContinueStatement => visitor.visit_continue_statement(cursor),
        ReturnStatement => visitor.visit_return_statement(cursor),
        ThrowStatement => visitor.visit_throw_statement(cursor),
        EmptyStatement => visitor.visit_empty_statement(cursor),
        LabeledStatement => visitor.visit_labeled_statement(cursor),
        _ => unreachable!(),
    }
}

pub fn visit_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    debug_assert!(is_declaration_kind(cursor.node().kind()));

    match cursor.node().kind() {
        FunctionDeclaration => visitor.visit_function_declaration(cursor),
        GeneratorFunctionDeclaration => visitor.visit_generator_function_declaration(cursor),
        ClassDeclaration => visitor.visit_class_declaration(cursor),
        LexicalDeclaration => visitor.visit_lexical_declaration(cursor),
        VariableDeclaration => visitor.visit_variable_declaration(cursor),
        _ => unreachable!(),
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

pub fn visit_class_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ClassDeclaration);
    unimplemented!()
}

pub fn visit_lexical_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, LexicalDeclaration);
    unimplemented!()
}

pub fn visit_variable_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, VariableDeclaration);
    unimplemented!()
}

pub fn visit_statement_block<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, StatementBlock);
    unimplemented!()
}

pub fn visit_if_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, IfStatement);
    unimplemented!()
}

pub fn visit_switch_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, SwitchStatement);
    unimplemented!()
}

pub fn visit_for_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ForStatement);
    unimplemented!()
}

pub fn visit_for_in_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ForInStatement);
    unimplemented!()
}

pub fn visit_while_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, WhileStatement);
    unimplemented!()
}

pub fn visit_do_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, DoStatement);
    unimplemented!()
}

pub fn visit_try_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, TryStatement);
    unimplemented!()
}

pub fn visit_with_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, WithStatement);
    unimplemented!()
}

pub fn visit_break_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, BreakStatement);
    unimplemented!()
}

pub fn visit_continue_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ContinueStatement);
    unimplemented!()
}

pub fn visit_return_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ReturnStatement);
    unimplemented!()
}

pub fn visit_throw_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, ThrowStatement);
    unimplemented!()
}

pub fn visit_empty_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, EmptyStatement);
    unimplemented!()
}

pub fn visit_labeled_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, LabeledStatement);
    unimplemented!()
}

pub fn visit_assignment_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, AssignmentExpression);
    unimplemented!()
}

pub fn visit_augmented_assignment_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, AugmentedAssignmentExpression);
    unimplemented!()
}

pub fn visit_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    unimplemented!()
}

pub fn visit_catch_clause<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    cursor: &mut TreeCursor<'a>,
) {
    assert_cursor_node_kind!(cursor, CatchClause);
    unimplemented!()
}
