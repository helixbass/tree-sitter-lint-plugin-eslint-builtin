use squalid::OptionExt;
use tracing::trace;
use tree_sitter_lint::tree_sitter::{Node, Tree};

use crate::kind::{self, *};

pub trait Visit<'a> {
    fn visit(&mut self, node: Node<'a>) {
        trace!(?node, "visiting node");

        match node.kind() {
            Program => self.visit_program(node),
            HashBangLine => self.visit_hash_bang_line(node),
            ExportStatement => self.visit_export_statement(node),
            NamespaceExport => self.visit_namespace_export(node),
            ExportClause => self.visit_export_clause(node),
            ExportSpecifier => self.visit_export_specifier(node),
            Import => self.visit_import(node),
            ImportStatement => self.visit_import_statement(node),
            ImportClause => self.visit_import_clause(node),
            NamespaceImport => self.visit_namespace_import(node),
            NamedImports => self.visit_named_imports(node),
            ImportSpecifier => self.visit_import_specifier(node),
            ExpressionStatement => self.visit_expression_statement(node),
            VariableDeclaration => self.visit_variable_declaration(node),
            LexicalDeclaration => self.visit_lexical_declaration(node),
            VariableDeclarator => self.visit_variable_declarator(node),
            StatementBlock => self.visit_statement_block(node),
            ElseClause => self.visit_else_clause(node),
            IfStatement => self.visit_if_statement(node),
            SwitchStatement => self.visit_switch_statement(node),
            ForStatement => self.visit_for_statement(node),
            ForInStatement => self.visit_for_in_statement(node),
            WhileStatement => self.visit_while_statement(node),
            DoStatement => self.visit_do_statement(node),
            TryStatement => self.visit_try_statement(node),
            WithStatement => self.visit_with_statement(node),
            BreakStatement => self.visit_break_statement(node),
            StatementIdentifier => self.visit_statement_identifier(node),
            ContinueStatement => self.visit_continue_statement(node),
            DebuggerStatement => self.visit_debugger_statement(node),
            ReturnStatement => self.visit_return_statement(node),
            ThrowStatement => self.visit_throw_statement(node),
            EmptyStatement => self.visit_empty_statement(node),
            LabeledStatement => self.visit_labeled_statement(node),
            SwitchBody => self.visit_switch_body(node),
            SwitchCase => self.visit_switch_case(node),
            SwitchDefault => self.visit_switch_default(node),
            CatchClause => self.visit_catch_clause(node),
            FinallyClause => self.visit_finally_clause(node),
            ParenthesizedExpression => self.visit_parenthesized_expression(node),
            YieldExpression => self.visit_yield_expression(node),
            Object => self.visit_object(node),
            ShorthandPropertyIdentifier => self.visit_shorthand_property_identifier(node),
            ObjectPattern => self.visit_object_pattern(node),
            ShorthandPropertyIdentifierPattern => {
                self.visit_shorthand_property_identifier_pattern(node)
            }
            AssignmentPattern => self.visit_assignment_pattern(node),
            ObjectAssignmentPattern => self.visit_object_assignment_pattern(node),
            Array => self.visit_array(node),
            ArrayPattern => self.visit_array_pattern(node),
            GlimmerTemplate => self.visit_glimmer_template(node),
            GlimmerOpeningTag => self.visit_glimmer_opening_tag(node),
            GlimmerClosingTag => self.visit_glimmer_closing_tag(node),
            JsxElement => self.visit_jsx_element(node),
            JsxText => self.visit_jsx_text(node),
            JsxExpression => self.visit_jsx_expression(node),
            JsxOpeningElement => self.visit_jsx_opening_element(node),
            PropertyIdentifier => self.visit_property_identifier(node),
            StringFragment => self.visit_string_fragment(node),
            JsxNamespaceName => self.visit_jsx_namespace_name(node),
            JsxClosingElement => self.visit_jsx_closing_element(node),
            JsxSelfClosingElement => self.visit_jsx_self_closing_element(node),
            JsxAttribute => self.visit_jsx_attribute(node),
            Class => self.visit_class(node),
            ClassDeclaration => self.visit_class_declaration(node),
            ClassHeritage => self.visit_class_heritage(node),
            Function => self.visit_function(node),
            FunctionDeclaration => self.visit_function_declaration(node),
            GeneratorFunction => self.visit_generator_function(node),
            GeneratorFunctionDeclaration => self.visit_generator_function_declaration(node),
            ArrowFunction => self.visit_arrow_function(node),
            OptionalChain => self.visit_optional_chain(node),
            CallExpression => self.visit_call_expression(node),
            NewExpression => self.visit_new_expression(node),
            AwaitExpression => self.visit_await_expression(node),
            MemberExpression => self.visit_member_expression(node),
            SubscriptExpression => self.visit_subscript_expression(node),
            AssignmentExpression => self.visit_assignment_expression(node),
            AugmentedAssignmentExpression => self.visit_augmented_assignment_expression(node),
            SpreadElement => self.visit_spread_element(node),
            TernaryExpression => self.visit_ternary_expression(node),
            BinaryExpression => self.visit_binary_expression(node),
            UnaryExpression => self.visit_unary_expression(node),
            UpdateExpression => self.visit_update_expression(node),
            SequenceExpression => self.visit_sequence_expression(node),
            kind::String => self.visit_string(node),
            EscapeSequence => self.visit_escape_sequence(node),
            Comment => self.visit_comment(node),
            TemplateString => self.visit_template_string(node),
            TemplateSubstitution => self.visit_template_substitution(node),
            kind::Regex => self.visit_regex(node),
            RegexPattern => self.visit_regex_pattern(node),
            RegexFlags => self.visit_regex_flags(node),
            kind::Number => self.visit_number(node),
            Identifier => self.visit_identifier(node),
            PrivatePropertyIdentifier => self.visit_private_property_identifier(node),
            MetaProperty => self.visit_meta_property(node),
            This => self.visit_this(node),
            Super => self.visit_super(node),
            True => self.visit_true(node),
            False => self.visit_false(node),
            Null => self.visit_null(node),
            Undefined => self.visit_undefined(node),
            Arguments => self.visit_arguments(node),
            Decorator => self.visit_decorator(node),
            ClassBody => self.visit_class_body(node),
            FieldDefinition => self.visit_field_definition(node),
            FormalParameters => self.visit_formal_parameters(node),
            ClassStaticBlock => self.visit_class_static_block(node),
            RestPattern => self.visit_rest_pattern(node),
            MethodDefinition => self.visit_method_definition(node),
            Pair => self.visit_pair(node),
            PairPattern => self.visit_pair_pattern(node),
            ComputedPropertyName => self.visit_computed_property_name(node),
            _ => unreachable!(),
        }
    }

    fn visit_program(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_hash_bang_line(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_export_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_import_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_debugger_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_expression_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_function_declaration(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_generator_function_declaration(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_class_declaration(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_lexical_declaration(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_variable_declaration(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_statement_block(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_if_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_switch_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_for_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_for_in_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_while_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_do_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_try_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_with_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_break_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_continue_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_return_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_throw_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_empty_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_labeled_statement(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_assignment_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_augmented_assignment_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_catch_clause(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_identifier(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_private_property_identifier(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_update_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_member_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_subscript_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_pair(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_computed_property_name(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_method_definition(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_formal_parameters(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_field_definition(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_class_static_block(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_class(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_call_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_this(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_parenthesized_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_function(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_arrow_function(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_namespace_import(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_import_specifier(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_export_clause(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_export_specifier(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_meta_property(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_class_heritage(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_class_body(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_array_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_assignment_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_rest_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_spread_element(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_array(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_namespace_export(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_import(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_import_clause(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_named_imports(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_variable_declarator(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_else_clause(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_statement_identifier(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_switch_body(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_switch_case(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_switch_default(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_finally_clause(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_yield_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_object(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_shorthand_property_identifier(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_object_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_shorthand_property_identifier_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_object_assignment_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_glimmer_template(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_glimmer_opening_tag(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_glimmer_closing_tag(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_element(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_text(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_opening_element(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_property_identifier(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_string_fragment(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_namespace_name(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_closing_element(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_self_closing_element(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_jsx_attribute(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_generator_function(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_optional_chain(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_new_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_await_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_ternary_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_binary_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_unary_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_sequence_expression(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_string(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_escape_sequence(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_comment(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_template_string(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_template_substitution(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_regex(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_regex_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_regex_flags(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_number(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_super(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_true(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_false(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_null(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_undefined(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_arguments(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_decorator(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }

    fn visit_pair_pattern(&mut self, node: Node<'a>) {
        visit_children(self, node);
    }
}

pub fn visit_children<'a, TVisit: Visit<'a> + ?Sized>(visitor: &mut TVisit, node: Node<'a>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visitor.visit(child);
    }
}

pub trait TreeEnterLeaveVisitor<'a> {
    fn enter_node(&mut self, node: Node<'a>);
    fn leave_node(&mut self, node: Node<'a>);
}

pub fn walk_tree<'a>(tree: &'a Tree, visitor: &mut impl TreeEnterLeaveVisitor<'a>) {
    let mut node_stack: Vec<Node<'a>> = Default::default();
    let mut cursor = tree.walk();
    'outer: loop {
        let node = cursor.node();
        while node_stack
            .last()
            .matches(|&last| node.end_byte() > last.end_byte())
        {
            trace!(?node, "leaving node");

            visitor.leave_node(node_stack.pop().unwrap());
        }
        trace!(?node, "entering node");

        node_stack.push(node);
        visitor.enter_node(node);

        #[allow(clippy::collapsible_if)]
        if !cursor.goto_first_child() {
            if !cursor.goto_next_sibling() {
                while cursor.goto_parent() {
                    if cursor.goto_next_sibling() {
                        continue 'outer;
                    }
                }
                break;
            }
        }
    }
    while let Some(node) = node_stack.pop() {
        trace!(?node, "leaving node");

        visitor.leave_node(node);
    }
}
