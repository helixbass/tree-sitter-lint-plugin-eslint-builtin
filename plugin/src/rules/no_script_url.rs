use std::sync::Arc;

use tree_sitter_lint::{
    rule, tree_sitter::Node, tree_sitter_grep::return_if_none, violation, QueryMatchContext, Rule,
};

use crate::{kind::CallExpression, utils::ast_utils};

fn check(node: Node, context: &QueryMatchContext) {
    let value = return_if_none!(ast_utils::get_static_string_value(node, context));

    if value.to_lowercase().starts_with("javascript:") {
        context.report(violation! {
            node => node,
            message_id => "unexpected_script_url",
        });
    }
}

pub fn no_script_url_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-script-url",
        languages => [Javascript],
        messages => [
            unexpected_script_url => "Script URL is a form of eval.",
        ],
        listeners => [
            r#"
              (string) @c
            "# => |node, context| {
                check(node, context);
            },
            r#"
              (template_string) @c
            "# => |node, context| {
                if node.parent().unwrap().kind() != CallExpression {
                    check(node, context);
                }
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use tree_sitter_lint::{rule_tests, RuleTester};

    use super::*;
    use crate::kind::{self, TemplateString};

    #[test]
    fn test_no_script_url_rule() {
        RuleTester::run(
            no_script_url_rule(),
            rule_tests! {
            valid => [
                "var a = 'Hello World!';",
                "var a = 10;",
                "var url = 'xjavascript:'",
                {
                    code => "var url = `xjavascript:`",
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => "var url = `${foo}javascript:`",
                    // parserOptions: { ecmaVersion: 6 }
                },
                {
                    code => "var a = foo`javaScript:`;",
                    // parserOptions: { ecmaVersion: 6 }
                }
            ],
            invalid => [
                {
                    code => "var a = 'javascript:void(0);';",
                    errors => [
                        { message_id => "unexpected_script_url", type => kind::String }
                    ]
                },
                {
                    code => "var a = 'javascript:';",
                    errors => [
                        { message_id => "unexpected_script_url", type => kind::String }
                    ]
                },
                {
                    code => "var a = `javascript:`;",
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        { message_id => "unexpected_script_url", type => TemplateString }
                    ]
                },
                {
                    code => "var a = `JavaScript:`;",
                    // parserOptions: { ecmaVersion: 6 },
                    errors => [
                        { message_id => "unexpected_script_url", type => TemplateString }
                    ]
                }
            ]
            },
        )
    }
}
