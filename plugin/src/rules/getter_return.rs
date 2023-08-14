use std::{collections::HashMap, sync::Arc};

use id_arena::Id;
use serde::Deserialize;
use squalid::{regex, return_default_if_none};
use tree_sitter_lint::{rule, tree_sitter::Node, violation, NodeExt, QueryMatchContext, Rule};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{Arguments, CallExpression, MethodDefinition, Object, Pair, StatementBlock},
    utils::ast_utils,
    CodePath, CodePathAnalyzer,
};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_implicit: bool,
}

fn is_getter(node: Node, context: &QueryMatchContext) -> bool {
    if !regex!(r#"^(?:(?:arrow_|generator_)?function|method_definition)$"#).is_match(node.kind()) {
        return false;
    }
    if node.field("body").kind() != StatementBlock {
        return false;
    }
    if node.kind() == MethodDefinition && node.has_child_of_kind("get") {
        return true;
    }
    let parent = node.parent().unwrap();
    let enclosing_object_expression = return_default_if_none!(if parent.kind() == Pair
        && ast_utils::get_static_property_name(parent, context).as_deref() == Some("get")
    {
        Some(parent.parent().unwrap())
    } else if parent.kind() == Object
        && ast_utils::get_static_property_name(node, context).as_deref() == Some("get")
    {
        Some(parent)
    } else {
        None
    });

    let parent_parent_parent = enclosing_object_expression.parent().unwrap();
    if parent_parent_parent.kind() == Arguments {
        let parent_parent_parent_parent = parent_parent_parent.parent().unwrap();
        if parent_parent_parent_parent.kind() == CallExpression {
            let call_node = parent_parent_parent_parent
                .field("function")
                .skip_parentheses();

            if ast_utils::is_specific_member_access(
                call_node,
                Some("Object"),
                Some("defineProperty"),
                context,
            ) || ast_utils::is_specific_member_access(
                call_node,
                Some("Reflect"),
                Some("defineProperty"),
                context,
            ) {
                return true;
            }
        }
    }

    if parent_parent_parent.kind() == Pair {
        let parent_parent_parent_parent_parent =
            parent_parent_parent.parent().unwrap().parent().unwrap();
        if parent_parent_parent_parent_parent.kind() == Arguments {
            let parent_parent_parent_parent_parent_parent =
                parent_parent_parent_parent_parent.parent().unwrap();
            if parent_parent_parent_parent_parent_parent.kind() == CallExpression {
                let call_node = parent_parent_parent_parent_parent_parent
                    .field("function")
                    .skip_parentheses();

                if ast_utils::is_specific_member_access(
                    call_node,
                    Some("Object"),
                    Some("defineProperties"),
                    context,
                ) || ast_utils::is_specific_member_access(
                    call_node,
                    Some("Object"),
                    Some("create"),
                    context,
                ) {
                    return true;
                }
            }
        }
    }

    false
}

pub fn getter_return_rule() -> Arc<dyn Rule> {
    rule! {
        name => "getter-return",
        languages => [Javascript],
        messages => [
            expected => "Expected to return a value in {{name}}.",
            expected_always => "Expected {{name}} to always return a value.",
        ],
        options_type => Options,
        state => {
            [per-run]
            allow_implicit: bool = options.allow_implicit,

            [per-file-run]
            code_paths_to_check: HashMap<Id<CodePath<'a>>, bool>,
        },
        listeners => [
            r#"
              (program) @c
            "# => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                self.code_paths_to_check = code_path_analyzer
                    .code_paths
                    .iter()
                    .filter(|&&code_path| {
                        is_getter(
                            code_path_analyzer
                                .code_path_arena[code_path]
                                .root_node(
                                    &code_path_analyzer.code_path_segment_arena
                                ),
                            context,
                        )
                    })
                    .copied()
                    .map(|code_path| (code_path, false))
                    .collect();
            },
            r#"
              (return_statement) @c
            "# => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                let code_path = code_path_analyzer.get_innermost_code_path(node);

                if !self.code_paths_to_check.contains_key(&code_path) {
                    return;
                }

                *self.code_paths_to_check.get_mut(&code_path).unwrap() = true;

                if !self.allow_implicit &&
                    !node.has_non_comment_named_children() {
                    context.report(violation! {
                        node => node,
                        message_id => "expected",
                        data => {
                            name => ast_utils::get_function_name_with_kind(
                                code_path_analyzer
                                    .code_path_arena[code_path]
                                    .root_node(&code_path_analyzer.code_path_segment_arena),
                                context
                            ),
                        },
                    });
                }
            },
            "program:exit" => |node, context| {
                let code_path_analyzer = context.retrieve::<CodePathAnalyzer<'a>>();

                for &code_path in code_path_analyzer
                    .code_paths
                    .iter()
                    .filter(|&&code_path| {
                        if !self.code_paths_to_check.contains_key(&code_path) {
                            return false;
                        }
                        code_path_analyzer.code_path_arena[code_path]
                            .state
                            .head_segments(&code_path_analyzer.fork_context_arena)
                            .reachable(&code_path_analyzer.code_path_segment_arena)
                    })
                {
                    let node = code_path_analyzer
                        .code_path_arena[code_path]
                        .root_node(&code_path_analyzer.code_path_segment_arena);
                    context.report(violation! {
                        node => node,
                        range => ast_utils::get_function_head_range(
                            node,
                        ),
                        message_id => if self
                            .code_paths_to_check[&code_path] {
                            "expected_always"
                        } else {
                            "expected"
                        },
                        data => {
                            name => ast_utils::get_function_name_with_kind(
                                node,
                                context,
                            ),
                        }
                    });
                }
            }
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::CodePathAnalyzerInstanceProviderFactory;

    use super::*;

    use tree_sitter_lint::{
        rule_tests, serde_json::json, RuleTestExpectedErrorBuilder, RuleTester,
    };

    fn expected_error_builder() -> RuleTestExpectedErrorBuilder {
        RuleTestExpectedErrorBuilder::default()
            .message_id("expected")
            .data([("name".to_owned(), "getter 'bar'".to_owned())])
            .clone()
    }

    fn expected_always_error_builder() -> RuleTestExpectedErrorBuilder {
        RuleTestExpectedErrorBuilder::default()
            .message_id("expected_always")
            .data([("name".to_owned(), "getter 'bar'".to_owned())])
            .clone()
    }

    #[test]
    fn test_getter_return_rule() {
        let expected_error = expected_error_builder().build().unwrap();
        let expected_always_error = expected_always_error_builder().build().unwrap();
        let options = json!({ "allow_implicit": true });

        RuleTester::run_with_from_file_run_context_instance_provider(
            getter_return_rule(),
            rule_tests! {
                valid => [
                    /*
                     * test obj: get
                     * option: {allowImplicit: false}
                     */
                    "var foo = { get bar(){return true;} };",

                    // option: {allowImplicit: true}
                    { code => "var foo = { get bar() {return;} };", options => options },
                    { code => "var foo = { get bar(){return true;} };", options => options },
                    { code => "var foo = { get bar(){if(bar) {return;} return true;} };", options => options },

                    /*
                     * test class: get
                     * option: {allowImplicit: false}
                     */
                    "class foo { get bar(){return true;} }",
                    "class foo { get bar(){if(baz){return true;} else {return false;} } }",
                    "class foo { get(){return true;} }",

                    // option: {allowImplicit: true}
                    { code => "class foo { get bar(){return true;} }", options => options },
                    { code => "class foo { get bar(){return;} }", options => options },

                    /*
                     * test object.defineProperty(s)
                     * option: {allowImplicit: false}
                     */
                    "Object.defineProperty(foo, \"bar\", { get: function () {return true;}});",
                    "Object.defineProperty(foo, \"bar\", { get: function () { ~function (){ return true; }();return true;}});",
                    "Object.defineProperties(foo, { bar: { get: function () {return true;}} });",
                    "Object.defineProperties(foo, { bar: { get: function () { ~function (){ return true; }(); return true;}} });",

                    /*
                     * test reflect.defineProperty(s)
                     * option: {allowImplicit: false}
                     */
                    "Reflect.defineProperty(foo, \"bar\", { get: function () {return true;}});",
                    "Reflect.defineProperty(foo, \"bar\", { get: function () { ~function (){ return true; }();return true;}});",

                    /*
                     * test object.create(s)
                     * option: {allowImplicit: false}
                     */
                    "Object.create(foo, { bar: { get() {return true;} } });",
                    "Object.create(foo, { bar: { get: function () {return true;} } });",
                    "Object.create(foo, { bar: { get: () => {return true;} } });",

                    // option: {allowImplicit: true}
                    { code => "Object.defineProperty(foo, \"bar\", { get: function () {return true;}});", options => options },
                    { code => "Object.defineProperty(foo, \"bar\", { get: function (){return;}});", options => options },
                    { code => "Object.defineProperties(foo, { bar: { get: function () {return true;}} });", options => options },
                    { code => "Object.defineProperties(foo, { bar: { get: function () {return;}} });", options => options },
                    { code => "Reflect.defineProperty(foo, \"bar\", { get: function () {return true;}});", options => options },

                    // not getter.
                    "var get = function(){};",
                    "var get = function(){ return true; };",
                    "var foo = { bar(){} };",
                    "var foo = { bar(){ return true; } };",
                    "var foo = { bar: function(){} };",
                    "var foo = { bar: function(){return;} };",
                    "var foo = { bar: function(){return true;} };",
                    "var foo = { get: function () {} }",
                    "var foo = { get: () => {}};",
                    "class C { get; foo() {} }",
                    "foo.defineProperty(null, { get() {} });",
                    "foo.defineProperties(null, { bar: { get() {} } });",
                    "foo.create(null, { bar: { get() {} } });"
                ],
                invalid => [
                    /*
                     * test obj: get
                     * option: {allowImplicit: false}
                     */
                    {
                        code => "var foo = { get bar() {} };",
                        errors => [
                            expected_error_builder()
                                .line(1)
                                .column(13)
                                .end_line(1)
                                .end_column(20)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "var foo = { get\n bar () {} };",
                        errors => [
                            expected_error_builder()
                                .line(1)
                                .column(13)
                                .end_line(2)
                                .end_column(6)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "var foo = { get bar(){if(baz) {return true;}} };",
                        errors => [
                            expected_always_error_builder()
                                .line(1)
                                .column(13)
                                .end_line(1)
                                .end_column(20)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "var foo = { get bar() { ~function () {return true;}} };",
                        errors => [
                            expected_error_builder()
                                .line(1)
                                .column(13)
                                .end_line(1)
                                .end_column(20)
                                .build()
                                .unwrap()
                        ]
                    },
                    {
                        code => "var foo = { get bar() { return; } };",
                        errors => [
                            expected_error_builder()
                                .line(1)
                                .column(25)
                                .end_line(1)
                                .end_column(32)
                                .build()
                                .unwrap()
                        ]
                    },

                    // option: {allowImplicit: true}
                    { code => "var foo = { get bar() {} };", options => options, errors => [expected_error] },
                    { code => "var foo = { get bar() {if (baz) {return;}} };", options => options, errors => [expected_always_error] },

                    /*
                     * test class: get
                     * option: {allowImplicit: false}
                     */
                    {
                        code => "class foo { get bar(){} }",
                        errors => [
                            expected_error_builder()
                                .line(1)
                                .column(13)
                                .end_line(1)
                                .end_column(20)
                                .build()
                                .unwrap()
                        ]
                    },
                    // TODO: this isn't parsing correctly per https://github.com/tree-sitter/tree-sitter-javascript/issues/262
                    // {
                    //     code => "var foo = class {\n  static get\nbar(){} }",
                    //     errors => [{
                    //         message_id => "expected",
                    //         data => { name => "static getter 'bar'" },
                    //         line => 2,
                    //         column => 3,
                    //         end_line => 3,
                    //         end_column => 4
                    //     }]
                    // },
                    { code => "class foo { get bar(){ if (baz) { return true; }}}", errors => [expected_always_error] },
                    { code => "class foo { get bar(){ ~function () { return true; }()}}", errors => [expected_error] },

                    // option: {allowImplicit: true}
                    { code => "class foo { get bar(){} }", options => options, errors => [expected_error] },
                    { code => "class foo { get bar(){if (baz) {return true;} } }", options => options, errors => [expected_always_error] },

                    /*
                     * test object.defineProperty(s)
                     * option: {allowImplicit: false}
                     */
                    {
                        code => "Object.defineProperty(foo, 'bar', { get: function (){}});",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 37,
                            end_line => 1,
                            end_column => 51
                        }]
                    },
                    {
                        code => "Object.defineProperty(foo, 'bar', { get: function getfoo (){}});",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 37,
                            end_line => 1,
                            end_column => 58
                        }]
                    },
                    {
                        code => "Object.defineProperty(foo, 'bar', { get(){} });",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 37,
                            end_line => 1,
                            end_column => 40
                        }]
                    },
                    {
                        code => "Object.defineProperty(foo, 'bar', { get: () => {}});",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 37,
                            end_line => 1,
                            end_column => 42
                        }]
                    },
                    { code => "Object.defineProperty(foo, \"bar\", { get: function (){if(bar) {return true;}}});", errors => [{ message_id => "expected_always" }] },
                    { code => "Object.defineProperty(foo, \"bar\", { get: function (){ ~function () { return true; }()}});", errors => [{ message_id => "expected" }] },

                    /*
                     * test reflect.defineProperty(s)
                     * option: {allowImplicit: false}
                     */
                    {
                        code => "Reflect.defineProperty(foo, 'bar', { get: function (){}});",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 38,
                            end_line => 1,
                            end_column => 52
                        }]
                    },

                    /*
                     * test object.create(s)
                     * option: {allowImplicit: false}
                     */
                    {
                        code => "Object.create(foo, { bar: { get: function() {} } })",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 29,
                            end_line => 1,
                            end_column => 42
                        }]
                    },
                    {
                        code => "Object.create(foo, { bar: { get() {} } })",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 29,
                            end_line => 1,
                            end_column => 32
                        }]
                    },
                    {
                        code => "Object.create(foo, { bar: { get: () => {} } })",
                        errors => [{
                            message_id => "expected",
                            data => { name => "method 'get'" },
                            line => 1,
                            column => 29,
                            end_line => 1,
                            end_column => 34
                        }]
                    },

                    // option: {allowImplicit: true}
                    { code => "Object.defineProperties(foo, { bar: { get: function () {}} });", options => options, errors => [{ message_id => "expected" }] },
                    { code => "Object.defineProperties(foo, { bar: { get: function (){if(bar) {return true;}}}});", options => options, errors => [{ message_id => "expected_always" }] },
                    { code => "Object.defineProperties(foo, { bar: { get: function () {~function () { return true; }()}} });", options => options, errors => [{ message_id => "expected" }] },
                    { code => "Object.defineProperty(foo, \"bar\", { get: function (){}});", options => options, errors => [{ message_id => "expected" }] },
                    { code => "Object.create(foo, { bar: { get: function (){} } });", options => options, errors => [{ message_id => "expected" }] },
                    { code => "Reflect.defineProperty(foo, \"bar\", { get: function (){}});", options => options, errors => [{ message_id => "expected" }] },

                    // Optional chaining
                    {
                        code => "Object?.defineProperty(foo, 'bar', { get: function (){} });",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected", data => { name => "method 'get'" } }]
                    },
                    {
                        code => "(Object?.defineProperty)(foo, 'bar', { get: function (){} });",
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected", data => { name => "method 'get'" } }]
                    },
                    {
                        code => "Object?.defineProperty(foo, 'bar', { get: function (){} });",
                        options => options,
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected", data => { name => "method 'get'" } }]
                    },
                    {
                        code => "(Object?.defineProperty)(foo, 'bar', { get: function (){} });",
                        options => options,
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected", data => { name => "method 'get'" } }]
                    },
                    {
                        code => "(Object?.create)(foo, { bar: { get: function (){} } });",
                        options => options,
                        // parserOptions: { ecmaVersion: 2020 },
                        errors => [{ message_id => "expected", data => { name => "method 'get'" } }]
                    }
                ]
            },
            Box::new(CodePathAnalyzerInstanceProviderFactory),
        )
    }
}
