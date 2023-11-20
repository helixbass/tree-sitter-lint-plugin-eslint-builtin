use std::sync::Arc;

use serde::Deserialize;
use tree_sitter_lint::{rule, tree_sitter::Node, violation, Rule};

const DEFAULT_MAX: usize = 10;

#[derive(Deserialize)]
#[serde(default)]
struct OptionsObject {
    #[serde(alias = "maximum")]
    max: usize,
}

impl Default for OptionsObject {
    fn default() -> Self {
        Self { max: DEFAULT_MAX }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Options {
    Usize(usize),
    Object(OptionsObject),
}

impl Options {
    pub fn max(&self) -> usize {
        match self {
            Self::Usize(value) => *value,
            Self::Object(OptionsObject { max }) => *max,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self::Usize(DEFAULT_MAX)
    }
}

pub fn max_nested_callbacks_rule() -> Arc<dyn Rule> {
    rule! {
        name => "max-nested-callbacks",
        languages => [Javascript],
        messages => [
            exceed => "Too many nested callbacks ({{num}}). Maximum allowed is {{max}}.",
        ],
        options_type => Options,
        state => {
            [per-config]
            threshold: usize = options.max(),

            [per-file-run]
            callback_stack: Vec<Node<'a>> = Default::default(),
        },
        listeners => [
            r#"
              (call_expression
                arguments: (arguments
                  [
                    (arrow_function)
                    (function)
                  ] @c
                )
              )
            "# => |node, context| {
                self.callback_stack.push(node);
                if self.callback_stack.len() > self.threshold {
                    context.report(violation! {
                        node => node,
                        message_id => "exceed",
                        data => {
                            num => self.callback_stack.len(),
                            max => self.threshold,
                        }
                    });
                }
            },
            r#"
              arrow_function:exit,
              function:exit
            "# => |node, context| {
                // TODO: the fact that it's _always_ popping (even for
                // functions that didn't meet the condition to get
                // pushed) looks to me like a bug in the ESLint version,
                // upstream?
                if Some(node) == self.callback_stack.last().copied() {
                    self.callback_stack.pop().unwrap();
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    fn nest_functions(times: usize) -> String {
        let mut openings: String = Default::default();
        let mut closings: String = Default::default();

        for _ in 0..times {
            openings.push_str("foo(function() {");
            closings.push_str("});");
        }

        format!("{openings}{closings}")
    }

    #[test]
    fn test_max_nested_callbacks_rule() {
        RuleTester::run(
            max_nested_callbacks_rule(),
            rule_tests! {
                valid => [
                    { code => "foo(function() { bar(thing, function(data) {}); });", options => 3 },
                    { code => "var foo = function() {}; bar(function(){ baz(function() { qux(foo); }) });", options => 2 },
                    { code => "fn(function(){}, function(){}, function(){});", options => 2 },
                    { code => "fn(() => {}, function(){}, function(){});", options => 2, /*parserOptions: { ecmaVersion: 6 }*/ },
                    nest_functions(10),

                    // object property options
                    { code => "foo(function() { bar(thing, function(data) {}); });", options => { max => 3 } }
                ],
                invalid => [
                    {
                        code => "foo(function() { bar(thing, function(data) { baz(function() {}); }); });",
                        options => 2,
                        errors => [{ message_id => "exceed", data => { num => 3, max => 2 }, type => "function" }]
                    },
                    {
                        code => "foo(function() { bar(thing, (data) => { baz(function() {}); }); });",
                        options => 2,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{ message_id => "exceed", data => { num => 3, max => 2 }, type => "function" }]
                    },
                    {
                        code => "foo(() => { bar(thing, (data) => { baz( () => {}); }); });",
                        options => 2,
                        // parserOptions: { ecmaVersion: 6 },
                        errors => [{ message_id => "exceed", data => { num => 3, max => 2 }, type => "arrow_function" }]
                    },
                    {
                        code => "foo(function() { if (isTrue) { bar(function(data) { baz(function() {}); }); } });",
                        options => 2,
                        errors => [{ message_id => "exceed", data => { num => 3, max => 2 }, type => "function" }]
                    },
                    {
                        code => nest_functions(11),
                        errors => [{ message_id => "exceed", data => { num => 11, max => 10 }, type => "function" }]
                    },
                    {
                        code => nest_functions(11),
                        options => {},
                        errors => [{ message_id => "exceed", data => { num => 11, max => 10 }, type => "function" }]
                    },
                    {
                        code => "foo(function() {})",
                        options => { max => 0 },
                        errors => [{ message_id => "exceed", data => { num => 1, max => 0 } }]
                    },

                    // object property options
                    {
                        code => "foo(function() { bar(thing, function(data) { baz(function() {}); }); });",
                        options => { max => 2 },
                        errors => [{ message_id => "exceed", data => { num => 3, max => 2 }, type => "function" }]
                    }
                ]
            },
        )
    }
}
