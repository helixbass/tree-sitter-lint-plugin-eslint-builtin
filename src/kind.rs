#![allow(dead_code)]

pub type Kind = &'static str;

pub const FunctionDeclaration: &str = "function_declaration";
pub const Function: &str = "function";
pub const ArrowFunction: &str = "arrow_function";
pub const DoStatement: &str = "do_statement";
pub const ForInStatement: &str = "for_in_statement";
pub const WhileStatement: &str = "while_statement";
pub const ForStatement: &str = "for_statement";
pub const MethodDefinition: &str = "method_definition";
pub const AssignmentExpression: &str = "assignment_expression";
pub const IfStatement: &str = "if_statement";
pub const TernaryExpression: &str = "ternary_expression";
pub const ParenthesizedExpression: &str = "parenthesized_expression";
pub const ExpressionStatement: &str = "expression_statement";
pub const AugmentedAssignmentExpression: &str = "augmented_assignment_expression";
