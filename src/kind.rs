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
pub const Pair: &str = "pair";
pub const FieldDefinition: &str = "field_definition";
pub const MemberExpression: &str = "member_expression";
pub const SubscriptExpression: &str = "subscript_expression";
pub const Identifier: &str = "identifier";
pub const Number: &str = "number";
pub const Null: &str = "null";
pub const Regex: &str = "regex";
pub const TemplateString: &str = "template_string";
pub const PropertyIdentifier: &str = "property_identifier";
pub const String: &str = "string";
