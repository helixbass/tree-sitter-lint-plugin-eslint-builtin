#![allow(dead_code)]

pub type Kind = &'static str;

pub const ArrayPattern: &str = "array_pattern";
pub const ArrowFunction: &str = "arrow_function";
pub const AssignmentExpression: &str = "assignment_expression";
pub const AssignmentPattern: &str = "assignment_pattern";
pub const AugmentedAssignmentExpression: &str = "augmented_assignment_expression";
pub const BreakStatement: &str = "break_statement";
pub const CatchClause: &str = "catch_clause";
pub const ClassDeclaration: &str = "class_declaration";
pub const Comment: &str = "comment";
pub const ComputedPropertyName: &str = "computed_property_name";
pub const ContinueStatement: &str = "continue_statement";
pub const DebuggerStatement: &str = "debugger_statement";
pub const DoStatement: &str = "do_statement";
pub const EmptyStatement: &str = "empty_statement";
pub const ExportStatement: &str = "export_statement";
pub const ExpressionStatement: &str = "expression_statement";
pub const FieldDefinition: &str = "field_definition";
pub const ForInStatement: &str = "for_in_statement";
pub const ForStatement: &str = "for_statement";
pub const Function: &str = "function";
pub const FunctionDeclaration: &str = "function_declaration";
pub const GeneratorFunctionDeclaration: &str = "generator_function_declaration";
pub const HashBangLine: &str = "hash_bang_line";
pub const Identifier: &str = "identifier";
pub const IfStatement: &str = "if_statement";
pub const ImportStatement: &str = "import_statement";
pub const LabeledStatement: &str = "labeled_statement";
pub const LexicalDeclaration: &str = "lexical_declaration";
pub const MemberExpression: &str = "member_expression";
pub const MethodDefinition: &str = "method_definition";
pub const Number: &str = "number";
pub const Null: &str = "null";
pub const ObjectPattern: &str = "object_pattern";
pub const ObjectAssignmentPattern: &str = "object_assignment_pattern";
pub const Pair: &str = "pair";
pub const ParenthesizedExpression: &str = "parenthesized_expression";
pub const Program: &str = "program";
pub const PropertyIdentifier: &str = "property_identifier";
pub const Regex: &str = "regex";
pub const RestElement: &str = "rest_element";
pub const ReturnStatement: &str = "return_statement";
pub const SpreadElement: &str = "spread_element";
pub const StatementBlock: &str = "statement_block";
pub const String: &str = "string";
pub const SubscriptExpression: &str = "subscript_expression";
pub const SwitchStatement: &str = "switch_statement";
pub const TemplateString: &str = "template_string";
pub const TernaryExpression: &str = "ternary_expression";
pub const ThrowStatement: &str = "throw_statement";
pub const TryStatement: &str = "try_statement";
pub const VariableDeclaration: &str = "variable_declaration";
pub const WhileStatement: &str = "while_statement";
pub const WithStatement: &str = "with_statement";

pub fn is_statement_kind(kind: &str) -> bool {
    match kind {
        ExportStatement | ImportStatement | DebuggerStatement | ExpressionStatement
        | StatementBlock | IfStatement | SwitchStatement | ForStatement | ForInStatement
        | WhileStatement | DoStatement | TryStatement | WithStatement | BreakStatement
        | ContinueStatement | ReturnStatement | ThrowStatement | EmptyStatement
        | LabeledStatement => true,
        kind if is_declaration_kind(kind) => true,
        _ => false,
    }
}

pub fn is_declaration_kind(kind: &str) -> bool {
    matches!(
        kind,
        FunctionDeclaration
            | GeneratorFunctionDeclaration
            | ClassDeclaration
            | LexicalDeclaration
            | VariableDeclaration
    )
}
