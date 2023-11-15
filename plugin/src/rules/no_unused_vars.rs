use std::sync::Arc;

use itertools::Itertools;
use regex::Regex;
use serde::Deserialize;
use squalid::{regex, return_default_if_none, EverythingExt, OptionExt};
use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::SupportedLanguage, violation, NodeExt,
    QueryMatchContext, Rule, ViolationData,
};

use crate::{
    ast_helpers::{
        get_last_expression_of_sequence_expression, get_method_definition_kind,
        is_tagged_template_expression, MethodDefinitionKind,
    },
    kind::{
        ArrayPattern, ArrowFunction, AssignmentExpression, AugmentedAssignmentExpression,
        CallExpression, EmptyStatement, ExpressionStatement, ForInStatement, FormalParameters,
        Function, MethodDefinition, NewExpression, ObjectPattern, PairPattern,
        ParenthesizedExpression, RestPattern, ReturnStatement, SequenceExpression,
        ShorthandPropertyIdentifierPattern, StatementBlock, UpdateExpression, VariableDeclarator,
        YieldExpression,
    },
    scope::{Reference, Scope, ScopeManager, ScopeType, Variable, VariableType},
    utils::ast_utils,
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Vars {
    #[default]
    All,
    Local,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Args {
    All,
    #[default]
    AfterUsed,
    None,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
enum CaughtErrors {
    All,
    #[default]
    None,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct OptionsObject {
    vars: Vars,
    #[serde(with = "serde_regex")]
    vars_ignore_pattern: Option<Regex>,
    args: Args,
    ignore_rest_siblings: bool,
    #[serde(with = "serde_regex")]
    args_ignore_pattern: Option<Regex>,
    caught_errors: CaughtErrors,
    #[serde(with = "serde_regex")]
    caught_errors_ignore_pattern: Option<Regex>,
    #[serde(with = "serde_regex")]
    destructured_array_ignore_pattern: Option<Regex>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Vars(Vars),
    Object(OptionsObject),
}

impl Options {
    pub fn vars(&self) -> Vars {
        match self {
            Self::Vars(value) => *value,
            Self::Object(OptionsObject { vars, .. }) => *vars,
        }
    }

    pub fn vars_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                vars_ignore_pattern,
                ..
            }) => vars_ignore_pattern.clone(),
            _ => None,
        }
    }

    pub fn args(&self) -> Args {
        match self {
            Self::Object(OptionsObject { args, .. }) => *args,
            _ => Default::default(),
        }
    }

    pub fn ignore_rest_siblings(&self) -> bool {
        match self {
            Self::Object(OptionsObject {
                ignore_rest_siblings,
                ..
            }) => *ignore_rest_siblings,
            _ => Default::default(),
        }
    }

    pub fn args_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                args_ignore_pattern,
                ..
            }) => args_ignore_pattern.clone(),
            _ => None,
        }
    }

    pub fn caught_errors(&self) -> CaughtErrors {
        match self {
            Self::Object(OptionsObject { caught_errors, .. }) => *caught_errors,
            _ => Default::default(),
        }
    }

    pub fn caught_errors_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                caught_errors_ignore_pattern,
                ..
            }) => caught_errors_ignore_pattern.clone(),
            _ => None,
        }
    }

    pub fn destructured_array_ignore_pattern(&self) -> Option<Regex> {
        match self {
            Self::Object(OptionsObject {
                destructured_array_ignore_pattern,
                ..
            }) => destructured_array_ignore_pattern.clone(),
            _ => None,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Object(Default::default())
    }
}

fn get_defined_message_data(
    unused_var: &Variable,
    caught_errors_ignore_pattern: Option<&Regex>,
    args_ignore_pattern: Option<&Regex>,
    vars_ignore_pattern: Option<&Regex>,
) -> ViolationData {
    let def_type = unused_var.defs().next().map(|def| def.type_());
    let (type_, pattern) =
        if def_type == Some(VariableType::CatchClause) && caught_errors_ignore_pattern.is_some() {
            (
                Some("args"),
                caught_errors_ignore_pattern.map(Regex::as_str),
            )
        } else if def_type == Some(VariableType::Parameter) && args_ignore_pattern.is_some() {
            (Some("args"), args_ignore_pattern.map(Regex::as_str))
        } else if def_type != Some(VariableType::Parameter) && vars_ignore_pattern.is_some() {
            (Some("vars"), vars_ignore_pattern.map(Regex::as_str))
        } else {
            (None, None)
        };

    let additional = match (type_, pattern) {
        (Some(type_), Some(pattern)) => format!(". Allowed unused {type_} must match /{pattern}/u"),
        _ => Default::default(),
    };

    [
        ("var_name".to_owned(), unused_var.name().to_owned()),
        ("action".to_owned(), "defined".to_owned()),
        ("additional".to_owned(), additional),
    ]
    .into()
}

fn get_assigned_message_data(
    unused_var: Variable,
    destructured_array_ignore_pattern: Option<&Regex>,
    vars_ignore_pattern: Option<&Regex>,
) -> ViolationData {
    let def = unused_var.defs().next();
    let additional = if let Some(destructured_array_ignore_pattern) =
        destructured_array_ignore_pattern
            .filter(|_| def.matches(|def| def.name().parent().unwrap().kind() == ArrayPattern))
    {
        format!(
            ". Allowed unused elements of array destructuring patterns must match {}",
            destructured_array_ignore_pattern.as_str()
        )
    } else if let Some(vars_ignore_pattern) = vars_ignore_pattern {
        format!(
            ". Allowed unused vars must match {}",
            vars_ignore_pattern.as_str()
        )
    } else {
        Default::default()
    };

    [
        ("var_name".to_owned(), unused_var.name().to_owned()),
        ("action".to_owned(), "assigned a value".to_owned()),
        ("additional".to_owned(), additional),
    ]
    .into()
}

fn is_exported(variable: &Variable) -> bool {
    let Some(definition) = variable.defs().next() else {
        return false;
    };

    let mut node = definition.node();

    if node.kind() == VariableDeclarator {
        node = node.parent().unwrap();
    } else if definition.type_() == VariableType::Parameter {
        return false;
    }

    node.parent().unwrap().kind().starts_with("export")
}

fn has_rest_sibling(node: Node) -> bool {
    matches!(
        node.kind(),
        PairPattern | ShorthandPropertyIdentifierPattern
    ) && node.parent().unwrap().thrush(|node_parent| {
        node_parent.kind() == ObjectPattern
            && node_parent
                .non_comment_named_children(SupportedLanguage::Javascript)
                .last()
                .unwrap()
                .kind()
                == RestPattern
    })
}

fn get_object_pattern_child(node: Node) -> Node {
    match node.kind() {
        ShorthandPropertyIdentifierPattern => node,
        _ => node.parent().unwrap(),
    }
}

fn has_rest_spread_sibling(variable: &Variable, ignore_rest_siblings: bool) -> bool {
    if ignore_rest_siblings {
        let has_rest_sibling_definition = variable
            .defs()
            .any(|def| has_rest_sibling(get_object_pattern_child(def.name())));
        let has_rest_sibling_reference = variable
            .references()
            .any(|ref_| has_rest_sibling(get_object_pattern_child(ref_.identifier())));

        return has_rest_sibling_definition || has_rest_sibling_reference;
    }

    false
}

fn is_read_ref(ref_: &Reference) -> bool {
    ref_.is_read()
}

fn is_self_reference(ref_: &Reference, nodes: &[Node]) -> bool {
    let mut scope = ref_.from();

    loop {
        if nodes.contains(&scope.block()) {
            return true;
        }

        scope = return_default_if_none!(scope.maybe_upper());
    }
}

fn get_function_definitions<'a>(variable: &Variable<'a, '_>) -> Vec<Node<'a>> {
    let mut function_definitions: Vec<Node> = Default::default();

    variable.defs().for_each(|def| {
        let type_ = def.type_();
        let node = def.node();

        if type_ == VariableType::FunctionName {
            function_definitions.push(node);
        }

        if type_ == VariableType::Variable {
            if let Some(node_init) = node
                .child_by_field_name("value")
                .filter(|node_init| matches!(node_init.kind(), Function | ArrowFunction))
            {
                function_definitions.push(node_init);
            }
        }
    });

    function_definitions
}

fn is_inside(inner: Node, outer: Node) -> bool {
    inner.start_byte() >= outer.start_byte() && inner.end_byte() <= outer.end_byte()
}

fn is_unused_expression(node: Node) -> bool {
    let parent = node.parent().unwrap();

    match parent.kind() {
        ExpressionStatement => true,
        SequenceExpression => {
            let is_last_expression = get_last_expression_of_sequence_expression(parent) == node;

            if !is_last_expression {
                return true;
            }
            is_unused_expression(parent)
        }
        ParenthesizedExpression => is_unused_expression(parent),
        _ => false,
    }
}

fn get_rhs_node<'a>(ref_: &Reference<'a, '_>, prev_rhs_node: Option<Node<'a>>) -> Option<Node<'a>> {
    let id = ref_.identifier();
    let parent = id.parent().unwrap();
    let ref_scope = ref_.from().variable_scope();
    let var_scope = ref_.resolved().unwrap().scope().variable_scope();
    let can_be_used_later = ref_scope != var_scope || ast_utils::is_in_loop(id);
    // println!("get_rhs_node() id: {id:#?}, parent: {parent:#?}, prev_rhs_node: {prev_rhs_node:#?}, can_be_used_later: {can_be_used_later:#?}, parent kind: {:#?}, is unused: {:#?}, id is left: {:#?}",
    // matches!(
    //     parent.kind(),
    //     AssignmentExpression | AugmentedAssignmentExpression
    // ), is_unused_expression(parent),
    //     Some(id) == parent.child_by_field_name("left")
    //     );

    if prev_rhs_node.matches(|prev_rhs_node| is_inside(id, prev_rhs_node)) {
        return prev_rhs_node;
    }

    if matches!(
        parent.kind(),
        AssignmentExpression | AugmentedAssignmentExpression
    ) && is_unused_expression(parent)
        && id == parent.field("left")
        && !can_be_used_later
    {
        return Some(parent.field("right"));
    }
    None
}

fn is_storable_function(func_node: Node, rhs_node: Node) -> bool {
    let mut node = func_node;
    let mut parent = func_node.parent();

    while let Some(parent_present) = parent.filter(|&parent| is_inside(parent, rhs_node)) {
        match parent_present.kind() {
            SequenceExpression => {
                if get_last_expression_of_sequence_expression(parent_present) != node {
                    return false;
                }
            }

            CallExpression => {
                if is_tagged_template_expression(parent_present) {
                    return true;
                }
                return parent_present.field("function") != node;
            }
            NewExpression => return parent_present.field("constructor") != node,

            AssignmentExpression | AugmentedAssignmentExpression | YieldExpression => return true,

            kind => {
                if regex!(r#"(?:statement|declaration)$"#).is_match(kind) {
                    return true;
                }
            }
        }

        node = parent_present;
        parent = parent_present.parent();
    }

    false
}

fn is_inside_of_storable_function(id: Node, rhs_node: Node) -> bool {
    let Some(func_node) = ast_utils::get_upper_function(id) else {
        return false;
    };

    is_inside(func_node, rhs_node) && is_storable_function(func_node, rhs_node)
}

fn is_read_for_itself(ref_: &Reference, rhs_node: Option<Node>) -> bool {
    let id = ref_.identifier();
    let parent = id.parent().unwrap();

    // println!("is_read_for_itself() id: {id:#?}, parent: {parent:#?}, rhs_node: {rhs_node:#?}");
    ref_.is_read()
        && (((matches!(
            parent.kind(),
            AssignmentExpression | AugmentedAssignmentExpression
        ) && parent.field("left") == id
            && is_unused_expression(parent)
            && !(parent.kind() == AugmentedAssignmentExpression
                && ast_utils::is_logical_assignment_operator(parent.field("operator").kind())))
            || (parent.kind() == UpdateExpression && is_unused_expression(parent)))
            || (rhs_node.matches(|rhs_node| {
                is_inside(id, rhs_node) && !is_inside_of_storable_function(id, rhs_node)
            })))
}

fn is_for_in_of_ref(ref_: &Reference) -> bool {
    let mut target = ref_.identifier().parent().unwrap();

    if target.kind() != ForInStatement {
        return false;
    }

    target = return_default_if_none!(target.field("body").thrush(|target_body| {
        match target_body.kind() {
            EmptyStatement => None,
            StatementBlock => target_body
                .non_comment_named_children(SupportedLanguage::Javascript)
                .next(),
            _ => Some(target_body),
        }
    }));

    target.kind() == ReturnStatement
}

fn is_used_variable(variable: &Variable) -> bool {
    let function_nodes = get_function_definitions(variable);
    let is_function_definition = !function_nodes.is_empty();
    let mut rhs_node: Option<Node> = Default::default();

    variable.references().any(|ref_| {
        if is_for_in_of_ref(&ref_) {
            return true;
        }

        let for_itself = is_read_for_itself(&ref_, rhs_node);
        // println!("ref: {ref_:#?}, for_itself: {for_itself:#?}");

        rhs_node = get_rhs_node(&ref_, rhs_node);

        is_read_ref(&ref_)
            && !for_itself
            && !(is_function_definition && is_self_reference(&ref_, &function_nodes))
    })
}

fn is_after_last_used_arg<'a>(
    variable: &Variable<'a, '_>,
    scope_manager: &ScopeManager<'a>,
) -> bool {
    let def = variable.defs().next().unwrap();
    let params = scope_manager
        .get_declared_variables(def.node())
        .collect_vec();
    let posterior_params = &params[{
        params
            .iter()
            .position(|param| param == variable)
            .map_or_default(|index| index + 1)
    }..];

    !posterior_params.iter().any(
        |v| v.references().next().is_some(), /* || v.eslintUsed */
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_unused_variables<'a, 'b>(
    scope: Scope<'a, 'b>,
    unused_vars: &mut Vec<Variable<'a, 'b>>,
    vars: Vars,
    destructured_array_ignore_pattern: Option<&Regex>,
    caught_errors: CaughtErrors,
    caught_errors_ignore_pattern: Option<&Regex>,
    args: Args,
    args_ignore_pattern: Option<&Regex>,
    vars_ignore_pattern: Option<&Regex>,
    ignore_rest_siblings: bool,
    context: &QueryMatchContext,
    scope_manager: &ScopeManager<'a>,
) {
    if scope.type_() != ScopeType::Global || vars == Vars::All {
        for variable in scope
            .variables()
            .filter(|variable| !(
                scope.type_() == ScopeType::Class && scope.block().child_by_field_name("name") == variable.identifiers().next() ||
                scope.function_expression_scope() ||
                // variable.eslintUsed ||
                scope.type_() == ScopeType::Function && variable.name() == "arguments" && variable.identifiers().next().is_none()
            ))
        {
            let def = variable.defs().next();

            if let Some(def) = def {
                let type_ = def.type_();
                let ref_used_in_array_patterns = variable.references().any(|ref_| ref_.identifier().parent().unwrap().kind() == ArrayPattern);

                if (
                    def.name().parent().unwrap().kind() == ArrayPattern ||
                    ref_used_in_array_patterns
                ) && destructured_array_ignore_pattern.matches(|destructured_array_ignore_pattern| {
                    destructured_array_ignore_pattern.is_match(&def.name().text(context))
                }) {
                    continue;
                }

                if type_ == VariableType::CatchClause {
                    if caught_errors == CaughtErrors::None {
                        continue;
                    }

                    if caught_errors_ignore_pattern.matches(|caught_errors_ignore_pattern| {
                        caught_errors_ignore_pattern.is_match(&def.name().text(context))
                    }) {
                        continue;
                    }
                }

                #[allow(clippy::collapsible_else_if)]
                if type_ == VariableType::Parameter {
                    if def.node().thrush(|def_node| {
                        def_node.kind() == MethodDefinition &&
                            get_method_definition_kind(def_node, context) == MethodDefinitionKind::Set
                    }) {
                        continue;
                    }

                    if args == Args::None {
                        continue;
                    }

                    if args_ignore_pattern.matches(|args_ignore_pattern| {
                        args_ignore_pattern.is_match(&def.name().text(context))
                    }) {
                        continue;
                    }

                    if args == Args::AfterUsed &&
                        def.name().parent().unwrap().kind() == FormalParameters &&
                        !is_after_last_used_arg(&variable, scope_manager)
                    {
                        continue;
                    }
                } else {
                    if vars_ignore_pattern.matches(|vars_ignore_pattern| {
                        vars_ignore_pattern.is_match(&def.name().text(context))
                    }) {
                        continue;
                    }
                }
            }

            if !is_used_variable(&variable) && !is_exported(&variable) && !has_rest_spread_sibling(&variable, ignore_rest_siblings) {
                unused_vars.push(variable);
            }
        }
    }

    for child_scope in scope.child_scopes() {
        collect_unused_variables(
            child_scope,
            unused_vars,
            vars,
            destructured_array_ignore_pattern,
            caught_errors,
            caught_errors_ignore_pattern,
            args,
            args_ignore_pattern,
            vars_ignore_pattern,
            ignore_rest_siblings,
            context,
            scope_manager,
        );
    }
}

pub fn no_unused_vars_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-unused-vars",
        languages => [Javascript],
        messages => [
            unused_var => "'{{var_name}}' is {{action}} but never used{{additional}}.",
        ],
        options_type => Options,
        state => {
            [per-run]
            vars: Vars = options.vars(),
            vars_ignore_pattern: Option<Regex> = options.vars_ignore_pattern(),
            args: Args = options.args(),
            ignore_rest_siblings: bool = options.ignore_rest_siblings(),
            args_ignore_pattern: Option<Regex> = options.args_ignore_pattern(),
            caught_errors: CaughtErrors = options.caught_errors(),
            caught_errors_ignore_pattern: Option<Regex> = options.caught_errors_ignore_pattern(),
            destructured_array_ignore_pattern: Option<Regex> = options.destructured_array_ignore_pattern(),
        },
        listeners => [
            r#"program:exit"# => |node, context| {
                let program_node = node;
                let scope_manager = context.retrieve::<ScopeManager<'a>>();
                let mut unused_vars: Vec<Variable<'a, '_>> = Default::default();
                collect_unused_variables(
                    scope_manager.get_scope(program_node),
                    &mut unused_vars,
                    self.vars,
                    self.destructured_array_ignore_pattern.as_ref(),
                    self.caught_errors,
                    self.caught_errors_ignore_pattern.as_ref(),
                    self.args,
                    self.args_ignore_pattern.as_ref(),
                    self.vars_ignore_pattern.as_ref(),
                    self.ignore_rest_siblings,
                    context,
                    scope_manager,
                );

                for unused_var in unused_vars {
                    if unused_var.defs().next().is_some() {
                        let write_references = unused_var.references().filter(|ref_| {
                            ref_.is_write() && ref_.from().variable_scope() == unused_var.scope().variable_scope()
                        });

                        let reference_to_report = write_references.last();

                        context.report(violation! {
                            node => reference_to_report.map(|reference_to_report| {
                                reference_to_report.identifier()
                            }).unwrap_or_else(|| unused_var.identifiers().next().unwrap()),
                            message_id => "unused_var",
                            data => if unused_var.references().any(|ref_| ref_.is_write()) {
                                get_assigned_message_data(
                                    unused_var,
                                    self.destructured_array_ignore_pattern.as_ref(),
                                    self.vars_ignore_pattern.as_ref(),
                                )
                            } else {
                                get_defined_message_data(
                                    &unused_var,
                                    self.caught_errors_ignore_pattern.as_ref(),
                                    self.args_ignore_pattern.as_ref(),
                                    self.vars_ignore_pattern.as_ref(),
                                )
                            },
                        });
                    } else if let Some(mut unused_var_explicit_global_comments) = unused_var.explicit_global_comments() {
                        let directive_comment = unused_var_explicit_global_comments.next().unwrap();

                        context.report(violation! {
                            node => program_node,
                            range => ast_utils::get_name_location_in_global_directive_comment(
                                context,
                                directive_comment,
                                unused_var.name(),
                            ),
                            message_id => "unused_var",
                            data => get_defined_message_data(
                                &unused_var,
                                self.caught_errors_ignore_pattern.as_ref(),
                                self.args_ignore_pattern.as_ref(),
                                self.vars_ignore_pattern.as_ref(),
                            ),
                        });
                    }
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{
        rule_tests, RuleTestExpectedError, RuleTestExpectedErrorBuilder, RuleTester,
    };

    use super::*;
    use crate::{get_instance_provider_factory, kind::Identifier};

    fn defined_error_builder(
        var_name: &str,
        additional: Option<&str>,
        type_: Option<&str>,
    ) -> RuleTestExpectedErrorBuilder {
        let additional = additional.unwrap_or("");
        let type_ = type_.unwrap_or(Identifier);
        RuleTestExpectedErrorBuilder::default()
            .message_id("unused_var")
            .data([
                ("var_name".to_owned(), var_name.to_owned()),
                ("action".to_owned(), "defined".to_owned()),
                ("additional".to_owned(), additional.to_owned()),
            ])
            .type_(type_)
            .clone()
    }

    fn defined_error(
        var_name: &str,
        additional: Option<&str>,
        type_: Option<&str>,
    ) -> RuleTestExpectedError {
        defined_error_builder(var_name, additional, type_)
            .build()
            .unwrap()
    }

    fn assigned_error_builder(
        var_name: &str,
        additional: Option<&str>,
        type_: Option<&str>,
    ) -> RuleTestExpectedErrorBuilder {
        let additional = additional.unwrap_or("");
        let type_ = type_.unwrap_or(Identifier);
        RuleTestExpectedErrorBuilder::default()
            .message_id("unused_var")
            .data([
                ("var_name".to_owned(), var_name.to_owned()),
                ("action".to_owned(), "assigned a value".to_owned()),
                ("additional".to_owned(), additional.to_owned()),
            ])
            .type_(type_)
            .clone()
    }

    fn assigned_error(
        var_name: &str,
        additional: Option<&str>,
        type_: Option<&str>,
    ) -> RuleTestExpectedError {
        assigned_error_builder(var_name, additional, type_)
            .build()
            .unwrap()
    }

    #[test]
    fn test_no_unused_vars_rule() {
        RuleTester::run_with_from_file_run_context_instance_provider(
            no_unused_vars_rule(),
            rule_tests! {
                valid => [
                    "var foo = 5;\n\nlabel: while (true) {\n  console.log(foo);\n  break label;\n}",
                    "var foo = 5;\n\nwhile (true) {\n  console.log(foo);\n  break;\n}",
                    { code => "for (let prop in box) {\n        box[prop] = parseInt(box[prop]);\n}", environment => { ecma_version => 6 } },
                    "var box = {a: 2};\n    for (var prop in box) {\n        box[prop] = parseInt(box[prop]);\n}",
                    "f({ set foo(a) { return; } });",
                    { code => "a; var a;", options => "all" },
                    { code => "var a=10; alert(a);", options => "all" },
                    { code => "var a=10; (function() { alert(a); })();", options => "all" },
                    { code => "var a=10; (function() { setTimeout(function() { alert(a); }, 0); })();", options => "all" },
                    { code => "var a=10; d[a] = 0;", options => "all" },
                    { code => "(function() { var a=10; return a; })();", options => "all" },
                    { code => "(function g() {})()", options => "all" },
                    { code => "function f(a) {alert(a);}; f();", options => "all" },
                    { code => "var c = 0; function f(a){ var b = a; return b; }; f(c);", options => "all" },
                    { code => "function a(x, y){ return y; }; a();", options => "all" },
                    { code => "var arr1 = [1, 2]; var arr2 = [3, 4]; for (var i in arr1) { arr1[i] = 5; } for (var i in arr2) { arr2[i] = 10; }", options => "all" },
                    { code => "var a=10;", options => "local" },
                    { code => "var min = \"min\"; Math[min];", options => "all" },
                    { code => "Foo.bar = function(baz) { return baz; };", options => "all" },
                    "myFunc(function foo() {}.bind(this))",
                    "myFunc(function foo(){}.toString())",
                    "function foo(first, second) {\ndoStuff(function() {\nconsole.log(second);});}; foo()",
                    "(function() { var doSomething = function doSomething() {}; doSomething() }())",
                    "try {} catch(e) {}",
                    "/*global a */ a;",
                    { code => "var a=10; (function() { alert(a); })();", options => { vars => "all" } },
                    { code => "function g(bar, baz) { return baz; }; g();", options => { vars => "all" } },
                    { code => "function g(bar, baz) { return baz; }; g();", options => { vars => "all", args => "after-used" } },
                    { code => "function g(bar, baz) { return bar; }; g();", options => { vars => "all", args => "none" } },
                    { code => "function g(bar, baz) { return 2; }; g();", options => { vars => "all", args => "none" } },
                    { code => "function g(bar, baz) { return bar + baz; }; g();", options => { vars => "local", args => "all" } },
                    { code => "var g = function(bar, baz) { return 2; }; g();", options => { vars => "all", args => "none" } },
                    "(function z() { z(); })();",
                    // TODO: support this?
                    // { code => " ", globals => { a => true } },
                    { code => "var who = \"Paul\";\nmodule.exports = `Hello ${who}!`;", environment => { ecma_version => 6 } },
                    { code => "export var foo = 123;", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "export function foo () {}", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "let toUpper = (partial) => partial.toUpperCase; export {toUpper}", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "export class foo {}", environment => { ecma_version => 6, source_type => "module" } },
                    { code => "class Foo{}; var x = new Foo(); x.foo()", environment => { ecma_version => 6 } },
                    { code => "const foo = \"hello!\";function bar(foobar = foo) {  foobar.replace(/!$/, \" world!\");}\nbar();", environment => { ecma_version => 6 } },
                    "function Foo(){}; var x = new Foo(); x.foo()",
                    "function foo() {var foo = 1; return foo}; foo();",
                    "function foo(foo) {return foo}; foo(1);",
                    "function foo() {function foo() {return 1;}; return foo()}; foo();",
                    { code => "function foo() {var foo = 1; return foo}; foo();", environment => { environment => { ecma_version => 6 } } },
                    { code => "function foo(foo) {return foo}; foo(1);", environment => { environment => { ecma_version => 6 } } },
                    { code => "function foo() {function foo() {return 1;}; return foo()}; foo();", environment => { environment => { ecma_version => 6 } } },
                    { code => "const x = 1; const [y = x] = []; foo(y);", environment => { ecma_version => 6 } },
                    { code => "const x = 1; const {y = x} = {}; foo(y);", environment => { ecma_version => 6 } },
                    { code => "const x = 1; const {z: [y = x]} = {}; foo(y);", environment => { ecma_version => 6 } },
                    { code => "const x = []; const {z: [y] = x} = {}; foo(y);", environment => { ecma_version => 6 } },
                    { code => "const x = 1; let y; [y = x] = []; foo(y);", environment => { ecma_version => 6 } },
                    { code => "const x = 1; let y; ({z: [y = x]} = {}); foo(y);", environment => { ecma_version => 6 } },
                    { code => "const x = []; let y; ({z: [y] = x} = {}); foo(y);", environment => { ecma_version => 6 } },
                    { code => "const x = 1; function foo(y = x) { bar(y); } foo();", environment => { ecma_version => 6 } },
                    { code => "const x = 1; function foo({y = x} = {}) { bar(y); } foo();", environment => { ecma_version => 6 } },
                    { code => "const x = 1; function foo(y = function(z = x) { bar(z); }) { y(); } foo();", environment => { ecma_version => 6 } },
                    { code => "const x = 1; function foo(y = function() { bar(x); }) { y(); } foo();", environment => { ecma_version => 6 } },
                    { code => "var x = 1; var [y = x] = []; foo(y);", environment => { ecma_version => 6 } },
                    { code => "var x = 1; var {y = x} = {}; foo(y);", environment => { ecma_version => 6 } },
                    { code => "var x = 1; var {z: [y = x]} = {}; foo(y);", environment => { ecma_version => 6 } },
                    { code => "var x = []; var {z: [y] = x} = {}; foo(y);", environment => { ecma_version => 6 } },
                    { code => "var x = 1, y; [y = x] = []; foo(y);", environment => { ecma_version => 6 } },
                    { code => "var x = 1, y; ({z: [y = x]} = {}); foo(y);", environment => { ecma_version => 6 } },
                    { code => "var x = [], y; ({z: [y] = x} = {}); foo(y);", environment => { ecma_version => 6 } },
                    { code => "var x = 1; function foo(y = x) { bar(y); } foo();", environment => { ecma_version => 6 } },
                    { code => "var x = 1; function foo({y = x} = {}) { bar(y); } foo();", environment => { ecma_version => 6 } },
                    { code => "var x = 1; function foo(y = function(z = x) { bar(z); }) { y(); } foo();", environment => { ecma_version => 6 } },
                    { code => "var x = 1; function foo(y = function() { bar(x); }) { y(); } foo();", environment => { ecma_version => 6 } },

                    // TODO: support these?
                    // exported variables should work
                    // "/*exported toaster*/ var toaster = 'great'",
                    // "/*exported toaster, poster*/ var toaster = 1; poster = 0;",
                    // { code => "/*exported x*/ var { x } = y", environment => { ecma_version => 6 } },
                    // { code => "/*exported x, y*/  var { x, y } = z", environment => { ecma_version => 6 } },

                    // TODO: support these?
                    // Can mark variables as used via context.markVariableAsUsed()
                    // "/*eslint use-every-a:1*/ var a;",
                    // "/*eslint use-every-a:1*/ !function(a) { return 1; }",
                    // "/*eslint use-every-a:1*/ !function() { var a; return 1 }",

                    // ignore pattern
                    { code => "var _a;", options => { vars => "all", vars_ignore_pattern => "^_" } },
                    { code => "var a; function foo() { var _b; } foo();", options => { vars => "local", vars_ignore_pattern => "^_" } },
                    { code => "function foo(_a) { } foo();", options => { args => "all", args_ignore_pattern => "^_" } },
                    { code => "function foo(a, _b) { return a; } foo();", options => { args => "after-used", args_ignore_pattern => "^_" } },
                    { code => "var [ firstItemIgnored, secondItem ] = items;\nconsole.log(secondItem);", options => { vars => "all", vars_ignore_pattern => "[iI]gnored" }, environment => { ecma_version => 6 } },
                    {
                        code => "const [ a, _b, c ] = items;\nconsole.log(a+c);",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "const [ [a, _b, c] ] = items;\nconsole.log(a+c);",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "const { x: [_a, foo] } = bar;\nconsole.log(foo);",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "function baz([_b, foo]) { foo; };\nbaz()",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "function baz({x: [_b, foo]}) {foo};\nbaz()",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "function baz([{x: [_b, foo]}]) {foo};\nbaz()",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "
                        let _a, b;
                        foo.forEach(item => {
                            [_a, b] = item;
                            doSomething(b);
                        });
                        ",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "
                        // doesn't report _x
                        let _x, y;
                        _x = 1;
                        [_x, y] = foo;
                        y;

                        // doesn't report _a
                        let _a, b;
                        [_a, b] = foo;
                        _a = 1;
                        b;
                        ",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 2018 }
                    },
                    {
                        code => "
                        // doesn't report _x
                        let _x, y;
                        _x = 1;
                        [_x, y] = foo;
                        y;

                        // doesn't report _a
                        let _a, b;
                        _a = 1;
                        ({_a, ...b } = foo);
                        b;
                        ",
                        options => { destructured_array_ignore_pattern => "^_", ignore_rest_siblings => true },
                        environment => { ecma_version => 2018 },
                    },

                    // for-in loops (see #2342)
                    "(function(obj) { var name; for ( name in obj ) return; })({});",
                    "(function(obj) { var name; for ( name in obj ) { return; } })({});",
                    "(function(obj) { for ( var name in obj ) { return true } })({})",
                    "(function(obj) { for ( var name in obj ) return true })({})",

                    { code => "(function(obj) { let name; for ( name in obj ) return; })({});", environment => { ecma_version => 6 } },
                    { code => "(function(obj) { let name; for ( name in obj ) { return; } })({});", environment => { ecma_version => 6 } },
                    { code => "(function(obj) { for ( let name in obj ) { return true } })({})", environment => { ecma_version => 6 } },
                    { code => "(function(obj) { for ( let name in obj ) return true })({})", environment => { ecma_version => 6 } },

                    { code => "(function(obj) { for ( const name in obj ) { return true } })({})", environment => { ecma_version => 6 } },
                    { code => "(function(obj) { for ( const name in obj ) return true })({})", environment => { ecma_version => 6 } },

                    // For-of loops
                    { code => "(function(iter) { let name; for ( name of iter ) return; })({});", environment => { ecma_version => 6 } },
                    { code => "(function(iter) { let name; for ( name of iter ) { return; } })({});", environment => { ecma_version => 6 } },
                    { code => "(function(iter) { for ( let name of iter ) { return true } })({})", environment => { ecma_version => 6 } },
                    { code => "(function(iter) { for ( let name of iter ) return true })({})", environment => { ecma_version => 6 } },

                    { code => "(function(iter) { for ( const name of iter ) { return true } })({})", environment => { ecma_version => 6 } },
                    { code => "(function(iter) { for ( const name of iter ) return true })({})", environment => { ecma_version => 6 } },

                    // Sequence Expressions (See https://github.com/eslint/eslint/issues/14325)
                    { code => "let x = 0; foo = (0, x++);", environment => { ecma_version => 6 } },
                    { code => "let x = 0; foo = (0, x += 1);", environment => { ecma_version => 6 } },
                    { code => "let x = 0; foo = (0, x = x + 1);", environment => { ecma_version => 6 } },

                    // caughtErrors
                    {
                        code => "try{}catch(err){console.error(err);}",
                        options => { caught_errors => "all" }
                    },
                    {
                        code => "try{}catch(err){}",
                        options => { caught_errors => "none" }
                    },
                    {
                        code => "try{}catch(ignoreErr){}",
                        options => { caught_errors => "all", caught_errors_ignore_pattern => "^ignore" }
                    },

                    // caughtErrors with other combinations
                    {
                        code => "try{}catch(err){}",
                        options => { vars => "all", args => "all" }
                    },

                    // Using object rest for variable omission
                    {
                        code => "const data = { type: 'coords', x: 1, y: 2 };\nconst { type, ...coords } = data;\n console.log(coords);",
                        options => { ignore_rest_siblings => true },
                        environment => { ecma_version => 2018 }
                    },

                    // https://github.com/eslint/eslint/issues/6348
                    "var a = 0, b; b = a = a + 1; foo(b);",
                    "var a = 0, b; b = a += a + 1; foo(b);",
                    "var a = 0, b; b = a++; foo(b);",
                    "function foo(a) { var b = a = a + 1; bar(b) } foo();",
                    "function foo(a) { var b = a += a + 1; bar(b) } foo();",
                    "function foo(a) { var b = a++; bar(b) } foo();",

                    // https://github.com/eslint/eslint/issues/6576
                    [
                        "var unregisterFooWatcher;",
                        "// ...",
                        "unregisterFooWatcher = $scope.$watch( \"foo\", function() {",
                        "    // ...some code..",
                        "    unregisterFooWatcher();",
                        "});"
                    ].join("\n"),
                    [
                        "var ref;",
                        "ref = setInterval(",
                        "    function(){",
                        "        clearInterval(ref);",
                        "    }, 10);"
                    ].join("\n"),
                    [
                        "var _timer;",
                        "function f() {",
                        "    _timer = setTimeout(function () {}, _timer ? 100 : 0);",
                        "}",
                        "f();"
                    ].join("\n"),
                    "function foo(cb) { cb = function() { function something(a) { cb(1 + a); } register(something); }(); } foo();",
                    { code => "function* foo(cb) { cb = yield function(a) { cb(1 + a); }; } foo();", environment => { ecma_version => 6 } },
                    { code => "function foo(cb) { cb = tag`hello${function(a) { cb(1 + a); }}`; } foo();", environment => { ecma_version => 6 } },
                    "function foo(cb) { var b; cb = b = function(a) { cb(1 + a); }; b(); } foo();",

                    // https://github.com/eslint/eslint/issues/6646
                    [
                        "function someFunction() {",
                        "    var a = 0, i;",
                        "    for (i = 0; i < 2; i++) {",
                        "        a = myFunction(a);",
                        "    }",
                        "}",
                        "someFunction();"
                    ].join("\n"),

                    // https://github.com/eslint/eslint/issues/7124
                    {
                        code => "(function(a, b, {c, d}) { d })",
                        options => { args_ignore_pattern => "c" },
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "(function(a, b, {c, d}) { c })",
                        options => { args_ignore_pattern => "d" },
                        environment => { ecma_version => 6 }
                    },

                    // https://github.com/eslint/eslint/issues/7250
                    {
                        code => "(function(a, b, c) { c })",
                        options => { args_ignore_pattern => "c" }
                    },
                    {
                        code => "(function(a, b, {c, d}) { c })",
                        options => { args_ignore_pattern => "[cd]" },
                        environment => { ecma_version => 6 }
                    },

                    // https://github.com/eslint/eslint/issues/7351
                    {
                        code => "(class { set foo(UNUSED) {} })",
                        environment => { ecma_version => 6 }
                    },
                    {
                        code => "class Foo { set bar(UNUSED) {} } console.log(Foo)",
                        environment => { ecma_version => 6 }
                    },

                    // https://github.com/eslint/eslint/issues/8119
                    {
                        code => "(({a, ...rest}) => rest)",
                        options => { args => "all", ignore_rest_siblings => true },
                        environment => { ecma_version => 2018 }
                    },

                    // https://github.com/eslint/eslint/issues/14163
                    {
                        code => "let foo, rest;\n({ foo, ...rest } = something);\nconsole.log(rest);",
                        options => { ignore_rest_siblings => true },
                        environment => { ecma_version => 2020 }
                    },

                    // https://github.com/eslint/eslint/issues/10952
                    "/*eslint use-every-a:1*/ !function(b, a) { return 1 }",

                    // https://github.com/eslint/eslint/issues/10982
                    "var a = function () { a(); }; a();",
                    "var a = function(){ return function () { a(); } }; a();",
                    {
                        code => "const a = () => { a(); }; a();",
                        environment => { ecma_version => 2015 }
                    },
                    {
                        code => "const a = () => () => { a(); }; a();",
                        environment => { ecma_version => 2015 }
                    },

                    // export * as ns from "source"
                    {
                        code => "export * as ns from \"source\"",
                        environment => { ecma_version => 2020, source_type => "module" }
                    },

                    // import.meta
                    {
                        code => "import.meta",
                        environment => { ecma_version => 2020, source_type => "module" }
                    },

                    // https://github.com/eslint/eslint/issues/17299
                    {
                        code => "var a; a ||= 1;",
                        environment => { ecma_version => 2021 }
                    },
                    {
                        code => "var a; a &&= 1;",
                        environment => { ecma_version => 2021 }
                    },
                    {
                        code => "var a; a ??= 1;",
                        environment => { ecma_version => 2021 }
                    }
                ],
                invalid => [
                    { code => "function foox() { return foox(); }", errors => [defined_error("foox", None, None)] },
                    { code => "(function() { function foox() { if (true) { return foox(); } } }())", errors => [defined_error("foox", None, None)] },
                    { code => "var a=10", errors => [assigned_error("a", None, None)] },
                    { code => "function f() { var a = 1; return function(){ f(a *= 2); }; }", errors => [defined_error("f", None, None)] },
                    { code => "function f() { var a = 1; return function(){ f(++a); }; }", errors => [defined_error("f", None, None)] },
                    { code => "/*global a */", errors => [defined_error("a", Some(""), Some("Program"))] },
                    { code => "function foo(first, second) {\ndoStuff(function() {\nconsole.log(second);});};", errors => [defined_error("foo", None, None)] },
                    { code => "var a=10;", options => "all", errors => [assigned_error("a", None, None)] },
                    { code => "var a=10; a=20;", options => "all", errors => [assigned_error("a", None, None)] },
                    { code => "var a=10; (function() { var a = 1; alert(a); })();", options => "all", errors => [assigned_error("a", None, None)] },
                    { code => "var a=10, b=0, c=null; alert(a+b)", options => "all", errors => [assigned_error("c", None, None)] },
                    { code => "var a=10, b=0, c=null; setTimeout(function() { var b=2; alert(a+b+c); }, 0);", options => "all", errors => [assigned_error("b", None, None)] },
                    { code => "var a=10, b=0, c=null; setTimeout(function() { var b=2; var c=2; alert(a+b+c); }, 0);", options => "all", errors => [assigned_error("b", None, None), assigned_error("c", None, None)] },
                    { code => "function f(){var a=[];return a.map(function(){});}", options => "all", errors => [defined_error("f", None, None)] },
                    { code => "function f(){var a=[];return a.map(function g(){});}", options => "all", errors => [defined_error("f", None, None)] },
                    {
                        code => "function foo() {function foo(x) {\nreturn x; }; return function() {return foo; }; }",
                        errors => [{
                            message_id => "unused_var",
                            data => { var_name => "foo", action => "defined", additional => "" },
                            line => 1,
                            type => "Identifier"
                        }]
                    },
                    { code => "function f(){var x;function a(){x=42;}function b(){alert(x);}}", options => "all", errors => 3 },
                    { code => "function f(a) {}; f();", options => "all", errors => [defined_error("a", None, None)] },
                    { code => "function a(x, y, z){ return y; }; a();", options => "all", errors => [defined_error("z", None, None)] },
                    { code => "var min = Math.min", options => "all", errors => [assigned_error("min", None, None)] },
                    { code => "var min = {min: 1}", options => "all", errors => [assigned_error("min", None, None)] },
                    { code => "Foo.bar = function(baz) { return 1; };", options => "all", errors => [defined_error("baz", None, None)] },
                    { code => "var min = {min: 1}", options => { vars => "all" }, errors => [assigned_error("min", None, None)] },
                    { code => "function gg(baz, bar) { return baz; }; gg();", options => { vars => "all" }, errors => [defined_error("bar", None, None)] },
                    { code => "(function(foo, baz, bar) { return baz; })();", options => { vars => "all", args => "after-used" }, errors => [defined_error("bar", None, None)] },
                    { code => "(function(foo, baz, bar) { return baz; })();", options => { vars => "all", args => "all" }, errors => [defined_error("foo", None, None), defined_error("bar", None, None)] },
                    { code => "(function z(foo) { var bar = 33; })();", options => { vars => "all", args => "all" }, errors => [defined_error("foo", None, None), assigned_error("bar", None, None)] },
                    { code => "(function z(foo) { z(); })();", options => {}, errors => [defined_error("foo", None, None)] },
                    { code => "function f() { var a = 1; return function(){ f(a = 2); }; }", options => {}, errors => [defined_error("f", None, None), assigned_error("a", None, None)] },
                    { code => "import x from \"y\";", environment => { ecma_version => 6, source_type => "module" }, errors => [defined_error("x", None, None)] },
                    { code => "export function fn2({ x, y }) {\n console.log(x); \n};", environment => { ecma_version => 6, source_type => "module" }, errors => [defined_error("y", None, None)] },
                    { code => "export function fn2( x, y ) {\n console.log(x); \n};", environment => { ecma_version => 6, source_type => "module" }, errors => [defined_error("y", None, None)] },

                    // exported
                    { code => "/*exported max*/ var max = 1, min = {min: 1}", errors => [assigned_error("min", None, None)] },
                    { code => "/*exported x*/ var { x, y } = z", environment => { ecma_version => 6 }, errors => [assigned_error("y", None, None)] },

                    // ignore pattern
                    {
                        code => "var _a; var b;",
                        options => { vars => "all", vars_ignore_pattern => "^_" },
                        errors => [{
                            line => 1,
                            column => 13,
                            message_id => "unused_var",
                            data => {
                                var_name => "b",
                                action => "defined",
                                additional => ". Allowed unused vars must match /^_/u"
                            }
                        }]
                    },
                    {
                        code => "var a; function foo() { var _b; var c_; } foo();",
                        options => { vars => "local", vars_ignore_pattern => "^_" },
                        errors => [{
                            line => 1,
                            column => 37,
                            message_id => "unused_var",
                            data => {
                                var_name => "c_",
                                action => "defined",
                                additional => ". Allowed unused vars must match /^_/u"
                            }
                        }]
                    },
                    {
                        code => "function foo(a, _b) { } foo();",
                        options => { args => "all", args_ignore_pattern => "^_" },
                        errors => [{
                            line => 1,
                            column => 14,
                            message_id => "unused_var",
                            data => {
                                var_name => "a",
                                action => "defined",
                                additional => ". Allowed unused args must match /^_/u"
                            }
                        }]
                    },
                    {
                        code => "function foo(a, _b, c) { return a; } foo();",
                        options => { args => "after-used", args_ignore_pattern => "^_" },
                        errors => [{
                            line => 1,
                            column => 21,
                            message_id => "unused_var",
                            data => {
                                var_name => "c",
                                action => "defined",
                                additional => ". Allowed unused args must match /^_/u"
                            }
                        }]
                    },
                    {
                        code => "function foo(_a) { } foo();",
                        options => { args => "all", args_ignore_pattern => "[iI]gnored" },
                        errors => [{
                            line => 1,
                            column => 14,
                            message_id => "unused_var",
                            data => {
                                var_name => "_a",
                                action => "defined",
                                additional => ". Allowed unused args must match /[iI]gnored/u"
                            }
                        }]
                    },
                    {
                        code => "var [ firstItemIgnored, secondItem ] = items;",
                        options => { vars => "all", vars_ignore_pattern => "[iI]gnored" },
                        environment => { ecma_version => 6 },
                        errors => [{
                            line => 1,
                            column => 25,
                            message_id => "unused_var",
                            data => {
                                var_name => "secondItem",
                                action => "assigned a value",
                                additional => ". Allowed unused vars must match /[iI]gnored/u"
                            }
                        }]
                    },

                    // https://github.com/eslint/eslint/issues/15611
                    {
                        code => "
                        const array = ['a', 'b', 'c'];
                        const [a, _b, c] = array;
                        const newArray = [a, c];
                        ",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 2020 },
                        errors => [

                            // should report only `newArray`
                            assigned_error_builder("newArray", None, None).line(4).column(19).build().unwrap(),
                        ]
                    },
                    {
                        code => "
                        const array = ['a', 'b', 'c', 'd', 'e'];
                        const [a, _b, c] = array;
                        ",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 2020 },
                        errors => [
                            assigned_error_builder("a", Some(". Allowed unused elements of array destructuring patterns must match /^_/u"), None)
                                .line(3)
                                .column(20)
                                .build().unwrap(),
                            assigned_error_builder("c", Some(". Allowed unused elements of array destructuring patterns must match /^_/u"), None)
                                .line(3)
                                .column(27)
                                .build().unwrap(),
                        ]
                    },
                    {
                        code => "
                        const array = ['a', 'b', 'c'];
                        const [a, _b, c] = array;
                        const fooArray = ['foo'];
                        const barArray = ['bar'];
                        const ignoreArray = ['ignore'];
                        ",
                        options => { destructured_array_ignore_pattern => "^_", vars_ignore_pattern => "ignore" },
                        environment => { ecma_version => 2020 },
                        errors => [
                            assigned_error_builder("a", Some(". Allowed unused elements of array destructuring patterns must match /^_/u"), None)
                                .line(3)
                                .column(20)
                                .build().unwrap(),
                            assigned_error_builder("c", Some(". Allowed unused elements of array destructuring patterns must match /^_/u"), None)
                                .line(3)
                                .column(27)
                                .build().unwrap(),
                            assigned_error_builder("fooArray", Some(". Allowed unused vars must match /ignore/u"), None)
                                .line(4)
                                .column(19)
                                .build().unwrap(),
                            assigned_error_builder("barArray", Some(". Allowed unused vars must match /ignore/u"), None)
                                .line(5)
                                .column(19)
                                .build().unwrap(),
                        ]
                    },
                    {
                        code => "
                        const array = [obj];
                        const [{_a, foo}] = array;
                        console.log(foo);
                        ",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 2020 },
                        errors => [
                            assigned_error_builder("_a", None, None)
                                .line(3)
                                .column(21)
                                .build().unwrap(),
                        ]
                    },
                    {
                        code => "
                        function foo([{_a, bar}]) {
                            bar;
                        }
                        foo();
                        ",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 2020 },
                        errors => [
                            defined_error_builder("_a", None, None)
                                .line(2)
                                .column(28)
                                .build().unwrap(),
                        ]
                    },
                    {
                        code => "
                        let _a, b;

                        foo.forEach(item => {
                            [a, b] = item;
                        });
                        ",
                        options => { destructured_array_ignore_pattern => "^_" },
                        environment => { ecma_version => 2020 },
                        errors => [
                            defined_error_builder("_a", None, None)
                                .line(2)
                                .column(17)
                                .build().unwrap(),
                            assigned_error_builder("b", None, None)
                                .line(2)
                                .column(21)
                                .build().unwrap(),
                        ]
                    },

                    // for-in loops (see #2342)
                    {
                        code => "(function(obj) { var name; for ( name in obj ) { i(); return; } })({});",
                        errors => [{
                            line => 1,
                            column => 34,
                            message_id => "unused_var",
                            data => {
                                var_name => "name",
                                action => "assigned a value",
                                additional => ""
                            }
                        }]
                    },
                    {
                        code => "(function(obj) { var name; for ( name in obj ) { } })({});",
                        errors => [{
                            line => 1,
                            column => 34,
                            message_id => "unused_var",
                            data => {
                                var_name => "name",
                                action => "assigned a value",
                                additional => ""
                            }
                        }]
                    },
                    {
                        code => "(function(obj) { for ( var name in obj ) { } })({});",
                        errors => [{
                            line => 1,
                            column => 28,
                            message_id => "unused_var",
                            data => {
                                var_name => "name",
                                action => "assigned a value",
                                additional => ""
                            }
                        }]
                    },

                    // For-of loops
                    {
                        code => "(function(iter) { var name; for ( name of iter ) { i(); return; } })({});",
                        // env => { es6 => true },
                        errors => [{
                            line => 1,
                            column => 35,
                            message_id => "unused_var",
                            data => {
                                var_name => "name",
                                action => "assigned a value",
                                additional => ""
                            }
                        }]
                    },
                    {
                        code => "(function(iter) { var name; for ( name of iter ) { } })({});",
                        // env => { es6 => true },
                        errors => [{
                            line => 1,
                            column => 35,
                            message_id => "unused_var",
                            data => {
                                var_name => "name",
                                action => "assigned a value",
                                additional => ""
                            }
                        }]
                    },
                    {
                        code => "(function(iter) { for ( var name of iter ) { } })({});",
                        // env => { es6 => true },
                        errors => [{
                            line => 1,
                            column => 29,
                            message_id => "unused_var",
                            data => {
                                var_name => "name",
                                action => "assigned a value",
                                additional => ""
                            }
                        }]
                    },

                    // https://github.com/eslint/eslint/issues/3617
                    {
                        code => "\n/* global foobar, foo, bar */\nfoobar;",
                        errors => [
                            {
                                line => 2,
                                end_line => 2,
                                column => 19,
                                end_column => 22,
                                message_id => "unused_var",
                                data => {
                                    var_name => "foo",
                                    action => "defined",
                                    additional => ""
                                }
                            },
                            {
                                line => 2,
                                end_line => 2,
                                column => 24,
                                end_column => 27,
                                message_id => "unused_var",
                                data => {
                                    var_name => "bar",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ]
                    },
                    {
                        code => "\n/* global foobar,\n   foo,\n   bar\n */\nfoobar;",
                        errors => [
                            {
                                line => 3,
                                column => 4,
                                end_line => 3,
                                end_column => 7,
                                message_id => "unused_var",
                                data => {
                                    var_name => "foo",
                                    action => "defined",
                                    additional => ""
                                }
                            },
                            {
                                line => 4,
                                column => 4,
                                end_line => 4,
                                end_column => 7,
                                message_id => "unused_var",
                                data => {
                                    var_name => "bar",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // Rest property sibling without ignoreRestSiblings
                    {
                        code => "const data = { type: 'coords', x: 1, y: 2 };\nconst { type, ...coords } = data;\n console.log(coords);",
                        environment => { ecma_version => 2018 },
                        errors => [
                            {
                                line => 2,
                                column => 9,
                                message_id => "unused_var",
                                data => {
                                    var_name => "type",
                                    action => "assigned a value",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // Unused rest property with ignoreRestSiblings
                    {
                        code => "const data = { type: 'coords', x: 2, y: 2 };\nconst { type, ...coords } = data;\n console.log(type)",
                        options => { ignore_rest_siblings => true },
                        environment => { ecma_version => 2018 },
                        errors => [
                            {
                                line => 2,
                                column => 18,
                                message_id => "unused_var",
                                data => {
                                    var_name => "coords",
                                    action => "assigned a value",
                                    additional => ""
                                }
                            }
                        ]
                    },
                    {
                        code => "let type, coords;\n({ type, ...coords } = data);\n console.log(type)",
                        options => { ignore_rest_siblings => true },
                        environment => { ecma_version => 2018 },
                        errors => [
                            {
                                line => 2,
                                column => 13,
                                message_id => "unused_var",
                                data => {
                                    var_name => "coords",
                                    action => "assigned a value",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // Unused rest property without ignoreRestSiblings
                    {
                        code => "const data = { type: 'coords', x: 3, y: 2 };\nconst { type, ...coords } = data;\n console.log(type)",
                        environment => { ecma_version => 2018 },
                        errors => [
                            {
                                line => 2,
                                column => 18,
                                message_id => "unused_var",
                                data => {
                                    var_name => "coords",
                                    action => "assigned a value",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // Nested array destructuring with rest property
                    {
                        code => "const data = { vars: ['x','y'], x: 1, y: 2 };\nconst { vars: [x], ...coords } = data;\n console.log(coords)",
                        environment => { ecma_version => 2018 },
                        errors => [
                            {
                                line => 2,
                                column => 16,
                                message_id => "unused_var",
                                data => {
                                    var_name => "x",
                                    action => "assigned a value",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // Nested object destructuring with rest property
                    {
                        code => "const data = { defaults: { x: 0 }, x: 1, y: 2 };\nconst { defaults: { x }, ...coords } = data;\n console.log(coords)",
                        environment => { ecma_version => 2018 },
                        errors => [
                            {
                                line => 2,
                                column => 21,
                                message_id => "unused_var",
                                data => {
                                    var_name => "x",
                                    action => "assigned a value",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // https://github.com/eslint/eslint/issues/8119
                    {
                        code => "(({a, ...rest}) => {})",
                        options => { args => "all", ignore_rest_siblings => true },
                        environment => { ecma_version => 2018 },
                        errors => [defined_error("rest", None, None)]
                    },

                    // https://github.com/eslint/eslint/issues/3714
                    {
                        code => "/* global a$fooz,$foo */\na$fooz;",
                        errors => [
                            {
                                line => 1,
                                column => 18,
                                end_line => 1,
                                end_column => 22,
                                message_id => "unused_var",
                                data => {
                                    var_name => "$foo",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ],
                    },
                    {
                        code => "/* globals a$fooz, $ */\na$fooz;",
                        errors => [
                            {
                                line => 1,
                                column => 20,
                                end_line => 1,
                                end_column => 21,
                                message_id => "unused_var",
                                data => {
                                    var_name => "$",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ]
                    },
                    {
                        code => "/*globals $foo*/",
                        errors => [
                            {
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 15,
                                message_id => "unused_var",
                                data => {
                                    var_name => "$foo",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ],
                    },
                    {
                        code => "/* global global*/",
                        errors => [
                            {
                                line => 1,
                                column => 11,
                                end_line => 1,
                                end_column => 17,
                                message_id => "unused_var",
                                data => {
                                    var_name => "global",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ]
                    },
                    {
                        code => "/*global foo:true*/",
                        errors => [
                            {
                                line => 1,
                                column => 10,
                                end_line => 1,
                                end_column => 13,
                                message_id => "unused_var",
                                data => {
                                    var_name => "foo",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // non ascii.
                    {
                        code => "/*global 変数, 数*/\n変数;",
                        errors => [
                            {
                                line => 1,
                                column => 14,
                                end_line => 1,
                                end_column => 15,
                                message_id => "unused_var",
                                data => {
                                    var_name => "数",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // surrogate pair.
                    {
                        code => "/*global 𠮷𩸽, 𠮷*/\n\\u{20BB7}\\u{29E3D};",
                        // env => { es6 => true },
                        errors => [
                            {
                                line => 1,
                                column => 16,
                                end_line => 1,
                                end_column => 18,
                                message_id => "unused_var",
                                data => {
                                    var_name => "𠮷",
                                    action => "defined",
                                    additional => ""
                                }
                            }
                        ]
                    },

                    // https://github.com/eslint/eslint/issues/4047
                    {
                        code => "export default function(a) {}",
                        environment => { ecma_version => 6, source_type => "module" },
                        errors => [defined_error("a", None, None)]
                    },
                    {
                        code => "export default function(a, b) { console.log(a); }",
                        environment => { ecma_version => 6, source_type => "module" },
                        errors => [defined_error("b", None, None)]
                    },
                    {
                        code => "export default (function(a) {});",
                        environment => { ecma_version => 6, source_type => "module" },
                        errors => [defined_error("a", None, None)]
                    },
                    {
                        code => "export default (function(a, b) { console.log(a); });",
                        environment => { ecma_version => 6, source_type => "module" },
                        errors => [defined_error("b", None, None)]
                    },
                    {
                        code => "export default (a) => {};",
                        environment => { ecma_version => 6, source_type => "module" },
                        errors => [defined_error("a", None, None)]
                    },
                    {
                        code => "export default (a, b) => { console.log(a); };",
                        environment => { ecma_version => 6, source_type => "module" },
                        errors => [defined_error("b", None, None)]
                    },

                    // caughtErrors
                    {
                        code => "try{}catch(err){};",
                        options => { caught_errors => "all" },
                        errors => [defined_error("err", None, None)]
                    },
                    {
                        code => "try{}catch(err){};",
                        options => { caught_errors => "all", caught_errors_ignore_pattern => "^ignore" },
                        errors => [defined_error("err", Some(". Allowed unused args must match /^ignore/u"), None)]
                    },

                    // multiple try catch with one success
                    {
                        code => "try{}catch(ignoreErr){}try{}catch(err){};",
                        options => { caught_errors => "all", caught_errors_ignore_pattern => "^ignore" },
                        errors => [defined_error("err", Some(". Allowed unused args must match /^ignore/u"), None)]
                    },

                    // multiple try catch both fail
                    {
                        code => "try{}catch(error){}try{}catch(err){};",
                        options => { caught_errors => "all", caught_errors_ignore_pattern => "^ignore" },
                        errors => [
                            defined_error("error", Some(". Allowed unused args must match /^ignore/u"), None),
                            defined_error("err", Some(". Allowed unused args must match /^ignore/u"), None),
                        ]
                    },

                    // caughtErrors with other configs
                    {
                        code => "try{}catch(err){};",
                        options => { vars => "all", args => "all", caught_errors => "all" },
                        errors => [defined_error("err", None, None)]
                    },

                    // no conflict in ignore patterns
                    {
                        code => "try{}catch(err){};",
                        options => {
                            vars => "all",
                            args => "all",
                            caught_errors => "all",
                            args_ignore_pattern => "^er"
                        },
                        errors => [defined_error("err", None, None)]
                    },

                    // Ignore reads for modifications to itself: https://github.com/eslint/eslint/issues/6348
                    { code => "var a = 0; a = a + 1;", errors => [assigned_error("a", None, None)] },
                    { code => "var a = 0; a = a + a;", errors => [assigned_error("a", None, None)] },
                    { code => "var a = 0; a += a + 1;", errors => [assigned_error("a", None, None)] },
                    { code => "var a = 0; a++;", errors => [assigned_error("a", None, None)] },
                    { code => "function foo(a) { a = a + 1 } foo();", errors => [assigned_error("a", None, None)] },
                    { code => "function foo(a) { a += a + 1 } foo();", errors => [assigned_error("a", None, None)] },
                    { code => "function foo(a) { a++ } foo();", errors => [assigned_error("a", None, None)] },
                    { code => "var a = 3; a = a * 5 + 6;", errors => [assigned_error("a", None, None)] },
                    { code => "var a = 2, b = 4; a = a * 2 + b;", errors => [assigned_error("a", None, None)] },

                    // https://github.com/eslint/eslint/issues/6576 (For coverage)
                    {
                        code => "function foo(cb) { cb = function(a) { cb(1 + a); }; bar(not_cb); } foo();",
                        errors => [assigned_error("cb", None, None)]
                    },
                    {
                        code => "function foo(cb) { cb = function(a) { return cb(1 + a); }(); } foo();",
                        errors => [assigned_error("cb", None, None)]
                    },
                    {
                        code => "function foo(cb) { cb = (function(a) { cb(1 + a); }, cb); } foo();",
                        errors => [assigned_error("cb", None, None)]
                    },
                    {
                        code => "function foo(cb) { cb = (0, function(a) { cb(1 + a); }); } foo();",
                        errors => [assigned_error("cb", None, None)]
                    },

                    // https://github.com/eslint/eslint/issues/6646
                    {
                        code => "while (a) {
                            function foo(b) {
                                b = b + 1;
                            }
                            foo()
                        }",
                        errors => [assigned_error("b", None, None)]
                    },

                    // https://github.com/eslint/eslint/issues/7124
                    {
                        code => "(function(a, b, c) {})",
                        options => { args_ignore_pattern => "c" },
                        errors => [
                            defined_error("a", Some(". Allowed unused args must match /c/u"), None),
                            defined_error("b", Some(". Allowed unused args must match /c/u"), None),
                        ]
                    },
                    {
                        code => "(function(a, b, {c, d}) {})",
                        options => { args_ignore_pattern => "[cd]" },
                        environment => { ecma_version => 6 },
                        errors => [
                            defined_error("a", Some(". Allowed unused args must match /[cd]/u"), None),
                            defined_error("b", Some(". Allowed unused args must match /[cd]/u"), None),
                        ]
                    },
                    {
                        code => "(function(a, b, {c, d}) {})",
                        options => { args_ignore_pattern => "c" },
                        environment => { ecma_version => 6 },
                        errors => [
                            defined_error("a", Some(". Allowed unused args must match /c/u"), None),
                            defined_error("b", Some(". Allowed unused args must match /c/u"), None),
                            defined_error("d", Some(". Allowed unused args must match /c/u"), Some(ShorthandPropertyIdentifierPattern))
                        ]
                    },
                    {
                        code => "(function(a, b, {c, d}) {})",
                        options => { args_ignore_pattern => "d" },
                        environment => { ecma_version => 6 },
                        errors => [
                            defined_error("a", Some(". Allowed unused args must match /d/u"), None),
                            defined_error("b", Some(". Allowed unused args must match /d/u"), None),
                            defined_error("c", Some(". Allowed unused args must match /d/u"), Some(ShorthandPropertyIdentifierPattern))
                        ],
                    },
                    // TODO: support this?
                    // {
                    //     code => "/*global\rfoo*/",
                    //     errors => [{
                    //         line => 2,
                    //         column => 1,
                    //         end_line => 2,
                    //         end_column => 4,
                    //         message_id => "unused_var",
                    //         data => {
                    //             var_name => "foo",
                    //             action => "defined",
                    //             additional => ""
                    //         }
                    //     }]
                    // },

                    // https://github.com/eslint/eslint/issues/8442
                    {
                        code => "(function ({ a }, b ) { return b; })();",
                        environment => { ecma_version => 2015 },
                        errors => [
                            defined_error("a", None, Some(ShorthandPropertyIdentifierPattern))
                        ]
                    },
                    {
                        code => "(function ({ a }, { b, c } ) { return b; })();",
                        environment => { ecma_version => 2015 },
                        errors => [
                            defined_error("a", None, Some(ShorthandPropertyIdentifierPattern)),
                            defined_error("c", None, Some(ShorthandPropertyIdentifierPattern))
                        ]
                    },

                    // https://github.com/eslint/eslint/issues/14325
                    {
                        code => "let x = 0;
x++, x = 0;",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(2).column(6).build().unwrap()],
                    },
                    {
                        code => "let x = 0;
x++, x = 0;
x=3;",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(3).column(1).build().unwrap()],
                    },
                    {
                        code => "let x = 0; x++, 0;",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(12).build().unwrap()],
                    },
                    {
                        code => "let x = 0; 0, x++;",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(15).build().unwrap()],
                    },
                    {
                        code => "let x = 0; 0, (1, x++);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(19).build().unwrap()],
                    },
                    {
                        code => "let x = 0; foo = (x++, 0);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(19).build().unwrap()],
                    },
                    {
                        code => "let x = 0; foo = ((0, x++), 0);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(23).build().unwrap()],
                    },
                    {
                        code => "let x = 0; x += 1, 0;",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(12).build().unwrap()],
                    },
                    {
                        code => "let x = 0; 0, x += 1;",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(15).build().unwrap()],
                    },
                    {
                        code => "let x = 0; 0, (1, x += 1);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(19).build().unwrap()],
                    },
                    {
                        code => "let x = 0; foo = (x += 1, 0);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(19).build().unwrap()],
                    },
                    {
                        code => "let x = 0; foo = ((0, x += 1), 0);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(23).build().unwrap()],
                    },

                    // https://github.com/eslint/eslint/issues/14866
                    {
                        code => "let z = 0;
z = z + 1, z = 2;
",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("z", None, None).line(2).column(12).build().unwrap()],
                    },
                    {
                        code => "let z = 0;
z = z+1, z = 2;
z = 3;",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("z", None, None).line(3).column(1).build().unwrap()],
                    },
                    {
                        code => "let z = 0;
z = z+1, z = 2;
z = z+3;
                        ",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("z", None, None).line(3).column(1).build().unwrap()],
                    },
                    {
                        code => "let x = 0; 0, x = x+1;",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(15).build().unwrap()],
                    },
                    {
                        code => "let x = 0; x = x+1, 0;",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(12).build().unwrap()],
                    },
                    {
                        code => "let x = 0; foo = ((0, x = x + 1), 0);",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(23).build().unwrap()],
                    },
                    {
                        code => "let x = 0; foo = (x = x+1, 0);",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(19).build().unwrap()],
                    },
                    {
                        code => "let x = 0; 0, (1, x=x+1);",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("x", None, None).line(1).column(19).build().unwrap()],
                    },
                    {
                        code => "(function ({ a, b }, { c } ) { return b; })();",
                        environment => { ecma_version => 2015 },
                        errors => [
                            defined_error("a", None, Some(ShorthandPropertyIdentifierPattern)),
                            defined_error("c", None, Some(ShorthandPropertyIdentifierPattern))
                        ],
                    },
                    {
                        code => "(function ([ a ], b ) { return b; })();",
                        environment => { ecma_version => 2015 },
                        errors => [
                            defined_error("a", None, None)
                        ]
                    },
                    {
                        code => "(function ([ a ], [ b, c ] ) { return b; })();",
                        environment => { ecma_version => 2015 },
                        errors => [
                            defined_error("a", None, None),
                            defined_error("c", None, None)
                        ]
                    },
                    {
                        code => "(function ([ a, b ], [ c ] ) { return b; })();",
                        environment => { ecma_version => 2015 },
                        errors => [
                            defined_error("a", None, None),
                            defined_error("c", None, None)
                        ],
                    },

                    // https://github.com/eslint/eslint/issues/9774
                    {
                        code => "(function(_a) {})();",
                        options => { args => "all", vars_ignore_pattern => "^_" },
                        errors => [defined_error("_a", None, None)]
                    },
                    {
                        code => "(function(_a) {})();",
                        options => { args => "all", caught_errors_ignore_pattern => "^_" },
                        errors => [defined_error("_a", None, None)]
                    },

                    // https://github.com/eslint/eslint/issues/10982
                    {
                        code => "var a = function() { a(); };",
                        errors => [assigned_error("a", None, None)]
                    },
                    {
                        code => "var a = function(){ return function() { a(); } };",
                        errors => [assigned_error("a", None, None)]
                    },
                    {
                        code => "const a = () => { a(); };",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error("a", None, None)]
                    },
                    {
                        code => "const a = () => () => { a(); };",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error("a", None, None)]
                    },
                    {
                        code => "let myArray = [1,2,3,4].filter((x) => x == 0);\n    myArray = myArray.filter((x) => x == 1);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("myArray", None, None).line(2).column(5).build().unwrap()],
                    },
                    {
                        code => "const a = 1; a += 1;",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("a", None, None).line(1).column(14).build().unwrap()],
                    },
                    {
                        code => "var a = function() { a(); };",
                        errors => [assigned_error_builder("a", None, None).line(1).column(5).build().unwrap()],
                    },
                    {
                        code => "var a = function(){ return function() { a(); } };",
                        errors => [assigned_error_builder("a", None, None).line(1).column(5).build().unwrap()],
                    },
                    {
                        code => "const a = () => { a(); };",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("a", None, None).line(1).column(7).build().unwrap()],
                    },
                    {
                        code => "const a = () => () => { a(); };",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("a", None, None).line(1).column(7).build().unwrap()],
                    },

                    // https://github.com/eslint/eslint/issues/14324
                    {
                        code => "let x = [];\nx = x.concat(x);",
                        environment => { ecma_version => 2015 },
                        errors => [assigned_error_builder("x", None, None).line(2).column(1).build().unwrap()],
                    },
                    {
                        code => "let a = 'a';
            a = 10;
            function foo(){
                a = 11;
                a = () => {
                    a = 13
                }
            }",
                        environment => { ecma_version => 2020 },
                        errors => [
                            assigned_error_builder("a", None, None).line(2).column(13).build().unwrap(),
                            defined_error_builder("foo", None, None)
                                .line(3)
                                .column(22)
                                .build().unwrap(),
                        ]
                    },
                    {
                        code => "let foo;
            init();
            foo = foo + 2;
            function init() {
                foo = 1;
            }",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("foo", None, None).line(3).column(13).build().unwrap()],
                    },
                    {
                        code => "function foo(n) {
                            if (n < 2) return 1;
                            return n * foo(n - 1);
                        }",
                        environment => { ecma_version => 2020 },
                        errors => [defined_error_builder("foo", None, None).line(1).column(10).build().unwrap()]
                    },
                    {
                        code => "let c = 'c'
c = 10
function foo1() {
  c = 11
  c = () => {
    c = 13
  }
}

c = foo1",
                        environment => { ecma_version => 2020 },
                        errors => [assigned_error_builder("c", None, None).line(10).column(1).build().unwrap()],
                    }
                ]
            },
            get_instance_provider_factory(),
        )
    }
}
