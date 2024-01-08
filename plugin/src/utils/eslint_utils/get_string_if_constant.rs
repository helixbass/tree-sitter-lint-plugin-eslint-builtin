use std::borrow::Cow;

use tree_sitter_lint::tree_sitter::Node;

use crate::scope::Scope;

pub fn get_string_if_constant<'a>(
    node: Node<'a>,
    initial_scope: Option<&Scope<'a, '_>>,
) -> Option<Cow<'a, str>> {
    None
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::HashMap};

    use speculoos::prelude::*;
    use tree_sitter_lint::{rule, rule_tests, RuleTester};

    use super::*;

    #[test]
    fn test_get_string_if_constant() {
        thread_local! {
            static ACTUALS: RefCell<HashMap<String, Option<String>>> = Default::default();
        }

        let rule = rule! {
            name => "test-get-string-if-constant",
            languages => [Javascript],
            listeners => [
                r#"
                  (expression_statement
                    (_) @c
                  )
                "# => |node, context| {
                    let actual = get_string_if_constant(node, None);
                    ACTUALS.with(|actuals| {
                        actuals.borrow_mut().insert(
                            context.file_run_context.file_contents.into(),
                            actual.map(Cow::into_owned),
                        );
                    });
                },
            ],
        };

        for (code, expected) in [
            ("true", Some("true")),
            ("false", Some("false")),
            ("0x100", Some("256")),
            ("3.14e+2", Some("314")),
            ("\"test\"",Some("test")),
            ("'abc'", Some("abc")),
            ("`abc`", Some("abc")),
            ("null", Some("null")),
            ("/a/", Some("/a/")),
            ("/a/g", Some("/a/g")),
            ("id", None),
            ("tag`foo`", None),
            ("`aaa${id}bbb`", None),
            ("1 + 2", Some("3")),
            ("'a' + 'b'", Some("ab")),
            ("/(?<a>\\w+)\\k<a>/gu", Some("/(?<a>\\w+)\\k<a>/gu")),
        ] {
            RuleTester::run(
                rule.clone(),
                rule_tests! {
                    valid => [
                        { code => code }
                    ],
                    invalid => [],
                },
            );
            ACTUALS.with(|actuals| {
                let actuals = actuals.borrow();
                assert_that!(actuals[code]).is_equal_to(expected.map(ToOwned::to_owned));
            });
        }
    }
}
