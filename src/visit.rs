use tree_sitter_lint::tree_sitter::Node;

use crate::kind::{
    is_declaration_kind, is_statement_kind, AssignmentExpression, AugmentedAssignmentExpression,
    BreakStatement, CallExpression, CatchClause, Class, ClassDeclaration, ClassStaticBlock,
    Comment, ComputedPropertyName, ContinueStatement, DebuggerStatement, DoStatement,
    EmptyStatement, ExportStatement, ExpressionStatement, FieldDefinition, ForInStatement,
    ForStatement, FormalParameters, FunctionDeclaration, GeneratorFunctionDeclaration,
    HashBangLine, Identifier, IfStatement, ImportStatement, LabeledStatement, LexicalDeclaration,
    MemberExpression, MethodDefinition, Pair, PrivatePropertyIdentifier, Program, ReturnStatement,
    StatementBlock, SubscriptExpression, SwitchStatement, This, ThrowStatement, TryStatement,
    UpdateExpression, VariableDeclaration, WhileStatement, WithStatement,
};

pub trait Visit<'a> {
    fn visit(&mut self, node: Node<'a>) {
        match node.kind() {
            Program => self.visit_program(node),
            _ => unimplemented!(),
        }
    }

    fn visit_program(&mut self, node: Node<'a>) {
        visit_program(self, node);
    }

    fn visit_statement(&mut self, node: Node<'a>) {
        visit_statement(self, node);
    }

    fn visit_declaration(&mut self, node: Node<'a>) {
        visit_declaration(self, node);
    }

    fn visit_export_statement(&mut self, node: Node<'a>) {
        visit_export_statement(self, node);
    }

    fn visit_import_statement(&mut self, node: Node<'a>) {
        visit_import_statement(self, node);
    }

    fn visit_debugger_statement(&mut self, node: Node<'a>) {
        visit_debugger_statement(self, node);
    }

    fn visit_expression_statement(&mut self, node: Node<'a>) {
        visit_expression_statement(self, node);
    }

    fn visit_function_declaration(&mut self, node: Node<'a>) {
        visit_function_declaration(self, node);
    }

    fn visit_generator_function_declaration(&mut self, node: Node<'a>) {
        visit_generator_function_declaration(self, node);
    }

    fn visit_class_declaration(&mut self, node: Node<'a>) {
        visit_class_declaration(self, node);
    }

    fn visit_lexical_declaration(&mut self, node: Node<'a>) {
        visit_lexical_declaration(self, node);
    }

    fn visit_variable_declaration(&mut self, node: Node<'a>) {
        visit_variable_declaration(self, node);
    }

    fn visit_statement_block(&mut self, node: Node<'a>) {
        visit_statement_block(self, node);
    }

    fn visit_if_statement(&mut self, node: Node<'a>) {
        visit_if_statement(self, node);
    }

    fn visit_switch_statement(&mut self, node: Node<'a>) {
        visit_switch_statement(self, node);
    }

    fn visit_for_statement(&mut self, node: Node<'a>) {
        visit_for_statement(self, node);
    }

    fn visit_for_in_statement(&mut self, node: Node<'a>) {
        visit_for_in_statement(self, node);
    }

    fn visit_while_statement(&mut self, node: Node<'a>) {
        visit_while_statement(self, node);
    }

    fn visit_do_statement(&mut self, node: Node<'a>) {
        visit_do_statement(self, node);
    }

    fn visit_try_statement(&mut self, node: Node<'a>) {
        visit_try_statement(self, node);
    }

    fn visit_with_statement(&mut self, node: Node<'a>) {
        visit_with_statement(self, node);
    }

    fn visit_break_statement(&mut self, node: Node<'a>) {
        visit_break_statement(self, node);
    }

    fn visit_continue_statement(&mut self, node: Node<'a>) {
        visit_continue_statement(self, node);
    }

    fn visit_return_statement(&mut self, node: Node<'a>) {
        visit_return_statement(self, node);
    }

    fn visit_throw_statement(&mut self, node: Node<'a>) {
        visit_throw_statement(self, node);
    }

    fn visit_empty_statement(&mut self, node: Node<'a>) {
        visit_empty_statement(self, node);
    }

    fn visit_labeled_statement(&mut self, node: Node<'a>) {
        visit_labeled_statement(self, node);
    }

    fn visit_assignment_expression(&mut self, node: Node<'a>) {
        visit_assignment_expression(self, node);
    }

    fn visit_augmented_assignment_expression(&mut self, node: Node<'a>) {
        visit_augmented_assignment_expression(self, node);
    }

    fn visit_expression(&mut self, node: Node<'a>) {
        visit_expression(self, node);
    }

    fn visit_catch_clause(&mut self, node: Node<'a>) {
        visit_catch_clause(self, node);
    }

    fn visit_identifier(&mut self, node: Node<'a>) {
        visit_identifier(self, node);
    }

    fn visit_private_property_identifier(&mut self, node: Node<'a>) {
        visit_private_property_identifier(self, node);
    }

    fn visit_update_expression(&mut self, node: Node<'a>) {
        visit_update_expression(self, node);
    }

    fn visit_member_expression(&mut self, node: Node<'a>) {
        visit_member_expression(self, node);
    }

    fn visit_subscript_expression(&mut self, node: Node<'a>) {
        visit_subscript_expression(self, node);
    }

    fn visit_pair(&mut self, node: Node<'a>) {
        visit_pair(self, node);
    }

    fn visit_computed_property_name(&mut self, node: Node<'a>) {
        visit_computed_property_name(self, node);
    }

    fn visit_method_definition(&mut self, node: Node<'a>) {
        visit_method_definition(self, node);
    }

    fn visit_formal_parameters(&mut self, node: Node<'a>) {
        visit_formal_parameters(self, node);
    }

    fn visit_field_definition(&mut self, node: Node<'a>) {
        visit_field_definition(self, node);
    }

    fn visit_class_static_block(&mut self, node: Node<'a>) {
        visit_class_static_block(self, node);
    }

    fn visit_class(&mut self, node: Node<'a>) {
        visit_class(self, node);
    }

    fn visit_call_expression(&mut self, node: Node<'a>) {
        visit_call_expression(self, node);
    }

    fn visit_this(&mut self, node: Node<'a>) {
        visit_this(self, node);
    }
}

macro_rules! assert_node_kind {
    ($node: expr, $kind:expr) => {
        debug_assert_eq!($node.kind(), $kind);
    };
}

macro_rules! return_if_false {
    ($expr:expr) => {
        if (!$expr) {
            return;
        }
    };
}

pub fn visit_program<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, Program);

    let mut cursor = node.walk();
    return_if_false!(cursor.goto_first_child());

    loop {
        let current_child = cursor.node();
        match current_child.kind() {
            HashBangLine => unimplemented!(),
            kind if is_statement_kind(kind) => visitor.visit_statement(current_child),
            Comment => unimplemented!(),
            _ => unreachable!(),
        }
        return_if_false!(cursor.goto_next_sibling());
    }
}

pub fn visit_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    debug_assert!(is_statement_kind(node.kind()));

    match node.kind() {
        ExportStatement => visitor.visit_export_statement(node),
        ImportStatement => visitor.visit_import_statement(node),
        DebuggerStatement => visitor.visit_debugger_statement(node),
        ExpressionStatement => visitor.visit_expression_statement(node),
        kind if is_declaration_kind(kind) => visitor.visit_declaration(node),
        StatementBlock => visitor.visit_statement_block(node),
        IfStatement => visitor.visit_if_statement(node),
        SwitchStatement => visitor.visit_switch_statement(node),
        ForStatement => visitor.visit_for_statement(node),
        ForInStatement => visitor.visit_for_in_statement(node),
        WhileStatement => visitor.visit_while_statement(node),
        DoStatement => visitor.visit_do_statement(node),
        TryStatement => visitor.visit_try_statement(node),
        WithStatement => visitor.visit_with_statement(node),
        BreakStatement => visitor.visit_break_statement(node),
        ContinueStatement => visitor.visit_continue_statement(node),
        ReturnStatement => visitor.visit_return_statement(node),
        ThrowStatement => visitor.visit_throw_statement(node),
        EmptyStatement => visitor.visit_empty_statement(node),
        LabeledStatement => visitor.visit_labeled_statement(node),
        _ => unreachable!(),
    }
}

pub fn visit_declaration<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    debug_assert!(is_declaration_kind(node.kind()));

    match node.kind() {
        FunctionDeclaration => visitor.visit_function_declaration(node),
        GeneratorFunctionDeclaration => visitor.visit_generator_function_declaration(node),
        ClassDeclaration => visitor.visit_class_declaration(node),
        LexicalDeclaration => visitor.visit_lexical_declaration(node),
        VariableDeclaration => visitor.visit_variable_declaration(node),
        _ => unreachable!(),
    }
}

pub fn visit_export_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ExportStatement);
    unimplemented!()
}

pub fn visit_import_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ImportStatement);
    unimplemented!()
}

pub fn visit_debugger_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, DebuggerStatement);
    unimplemented!()
}

pub fn visit_expression_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ExpressionStatement);
    unimplemented!()
}

pub fn visit_function_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, FunctionDeclaration);
    unimplemented!()
}

pub fn visit_generator_function_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, GeneratorFunctionDeclaration);
    unimplemented!()
}

pub fn visit_class_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ClassDeclaration);
    unimplemented!()
}

pub fn visit_lexical_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, LexicalDeclaration);
    unimplemented!()
}

pub fn visit_variable_declaration<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, VariableDeclaration);
    unimplemented!()
}

pub fn visit_statement_block<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, StatementBlock);
    unimplemented!()
}

pub fn visit_if_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, IfStatement);
    unimplemented!()
}

pub fn visit_switch_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, SwitchStatement);
    unimplemented!()
}

pub fn visit_for_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, ForStatement);
    unimplemented!()
}

pub fn visit_for_in_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ForInStatement);
    unimplemented!()
}

pub fn visit_while_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, WhileStatement);
    unimplemented!()
}

pub fn visit_do_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, DoStatement);
    unimplemented!()
}

pub fn visit_try_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, TryStatement);
    unimplemented!()
}

pub fn visit_with_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, WithStatement);
    unimplemented!()
}

pub fn visit_break_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, BreakStatement);
    unimplemented!()
}

pub fn visit_continue_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ContinueStatement);
    unimplemented!()
}

pub fn visit_return_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ReturnStatement);
    unimplemented!()
}

pub fn visit_throw_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, ThrowStatement);
    unimplemented!()
}

pub fn visit_empty_statement<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, EmptyStatement);
    unimplemented!()
}

pub fn visit_labeled_statement<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, LabeledStatement);
    unimplemented!()
}

pub fn visit_assignment_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, AssignmentExpression);
    unimplemented!()
}

pub fn visit_augmented_assignment_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, AugmentedAssignmentExpression);
    unimplemented!()
}

pub fn visit_expression<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    unimplemented!()
}

pub fn visit_expressions<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    unimplemented!()
}

pub fn visit_catch_clause<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, CatchClause);
    unimplemented!()
}

pub fn visit_identifier<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, Identifier);
    unimplemented!()
}

pub fn visit_private_property_identifier<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, PrivatePropertyIdentifier);
    unimplemented!()
}

pub fn visit_update_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, UpdateExpression);
    unimplemented!()
}

pub fn visit_member_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, MemberExpression);
    unimplemented!()
}

pub fn visit_subscript_expression<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, SubscriptExpression);
    unimplemented!()
}

pub fn visit_pair<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, Pair);
    unimplemented!()
}

pub fn visit_computed_property_name<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ComputedPropertyName);
    unimplemented!()
}

pub fn visit_method_definition<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, MethodDefinition);
    unimplemented!()
}

pub fn visit_formal_parameters<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, FormalParameters);
    unimplemented!()
}

pub fn visit_field_definition<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, FieldDefinition);
    unimplemented!()
}

pub fn visit_class_static_block<'a, TVisit: Visit<'a> + ?Sized>(
    visitor: &mut TVisit,
    node: Node<'a>,
) {
    assert_node_kind!(node, ClassStaticBlock);
    unimplemented!()
}

pub fn visit_class<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, Class);
    unimplemented!()
}

pub fn visit_call_expression<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, CallExpression);
    unimplemented!()
}

pub fn visit_this<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    assert_node_kind!(node, This);
    unimplemented!()
}
