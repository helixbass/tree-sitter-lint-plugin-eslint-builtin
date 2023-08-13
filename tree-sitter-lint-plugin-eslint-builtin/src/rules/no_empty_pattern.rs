use std::sync::Arc;

use serde::Deserialize;
use squalid::EverythingExt;
use tree_sitter_lint::{rule, violation, NodeExt, Rule};

use crate::{
    ast_helpers::NodeExtJs,
    kind::{AssignmentPattern, FormalParameters, Object},
};

#[derive(Default, Deserialize)]
#[serde(default)]
struct Options {
    allow_object_patterns_as_parameters: bool,
}

pub fn no_empty_pattern_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-empty-pattern",
        languages => [Javascript],
        messages => [
            unexpected => "Unexpected empty {{type}} pattern.",
        ],
        options_type => Options,
        state => {
            [per-run]
            allow_object_patterns_as_parameters: bool = options.allow_object_patterns_as_parameters,
        },
        listeners => [
            r#"
              (object_pattern) @c
            "# => |node, context| {
                if node.has_non_comment_named_children() {
                    return;
                }
                if self.allow_object_patterns_as_parameters && {
                    let parent = node.parent().unwrap();
                    parent.kind() == FormalParameters ||
                        parent.kind() == AssignmentPattern &&
                            parent.parent().unwrap().kind() == FormalParameters &&
                            parent.field("right").thrush(|right| {
                                right.kind() == Object &&
                                !right.has_non_comment_named_children()
                            })
                } {
                    return;
                }
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                    data => {
                        type => "object",
                    },
                });
            },
            r#"
              (array_pattern) @c
            "# => |node, context| {
                if node.has_non_comment_named_children() {
                    return;
                }
                context.report(violation! {
                    node => node,
                    message_id => "unexpected",
                    data => {
                        type => "array",
                    },
                });
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind::{ArrayPattern, ObjectPattern};

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_empty_pattern_rule() {
        RuleTester::run(
            no_empty_pattern_rule(),
            rule_tests! {
                // Examples of code that should not trigger the rule
                valid => [
                    { code => "var {a = {}} = foo;", /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "var {a, b = {}} = foo;", /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "var {a = []} = foo;", /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "function foo({a = {}}) {}", /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "function foo({a = []}) {}", /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "var [a] = foo", /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "function foo({}) {}", options => { allow_object_patterns_as_parameters => true }, /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "var foo = function({}) {}", options => { allow_object_patterns_as_parameters => true }, /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "var foo = ({}) => {}", options => { allow_object_patterns_as_parameters => true }, /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "function foo({} = {}) {}", options => { allow_object_patterns_as_parameters => true }, /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "var foo = function({} = {}) {}", options => { allow_object_patterns_as_parameters => true }, /*parserOptions => { ecmaVersion: 6 }*/ },
                    { code => "var foo = ({} = {}) => {}", options => { allow_object_patterns_as_parameters => true }, /*parserOptions => { ecmaVersion: 6 }*/ }
                ],
                // Examples of code that should trigger the rule
                invalid => [
                    {
                        code => "var {} = foo",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var [] = foo",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "array" },
                            type => ArrayPattern
                        }]
                    },
                    {
                        code => "var {a: {}} = foo",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var {a, b: {}} = foo",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var {a: []} = foo",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "array" },
                            type => ArrayPattern
                        }]
                    },
                    {
                        code => "function foo({}) {}",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "function foo([]) {}",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "array" },
                            type => ArrayPattern
                        }]
                    },
                    {
                        code => "function foo({a: {}}) {}",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "function foo({a: []}) {}",
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "array" },
                            type => ArrayPattern
                        }]
                    },
                    {
                        code => "function foo({}) {}",
                        options => {},
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = function({}) {}",
                        options => {},
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = ({}) => {}",
                        options => {},
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "function foo({} = {}) {}",
                        options => {},
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = function({} = {}) {}",
                        options => {},
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = ({} = {}) => {}",
                        options => {},
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = ({a: {}}) => {}",
                        options => { allow_object_patterns_as_parameters => true },
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = ({} = bar) => {}",
                        options => { allow_object_patterns_as_parameters => true },
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = ({} = { bar: 1 }) => {}",
                        options => { allow_object_patterns_as_parameters => true },
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "object" },
                            type => ObjectPattern
                        }]
                    },
                    {
                        code => "var foo = ([]) => {}",
                        options => { allow_object_patterns_as_parameters => true },
                        // parserOptions => { ecmaVersion: 6 },
                        errors => [{
                            message_id => "unexpected",
                            data => { type => "array" },
                            type => ArrayPattern
                        }]
                    }
                ]
            },
        )
    }
}
