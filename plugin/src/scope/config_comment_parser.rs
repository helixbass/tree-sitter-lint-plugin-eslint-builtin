use std::collections::HashMap;

use squalid::regex;
use tree_sitter_lint::tree_sitter::Node;

pub fn parse_string_config<'a>(
    string: &'a str,
    comment: Node<'a>,
) -> HashMap<String, StringConfig<'a>> {
    let mut items: HashMap<String, StringConfig<'a>> = Default::default();

    let trimmed_string = regex!(r#"\s*([:,])\s*"#).replace_all(string, "$1");

    regex!(r#"\s|,+"#).split(&trimmed_string).for_each(|name| {
        if name.is_empty() {
            return;
        }

        let mut split = name.split(':');
        let key = split.next().unwrap().to_owned();
        let value = split.next().map(ToOwned::to_owned);

        items.insert(key, StringConfig { value, comment });
    });
    items
}

#[derive(Debug, PartialEq, Eq)]
pub struct StringConfig<'a> {
    pub value: Option<String>,
    pub comment: Node<'a>,
}

#[cfg(test)]
mod tests {
    use once_cell::sync::Lazy;
    use speculoos::prelude::*;
    use tree_sitter_lint::tree_sitter::Tree;

    use super::*;
    use crate::tests::helpers::parse;

    fn get_comment() -> Node<'static> {
        static TREE: Lazy<Tree> = Lazy::new(|| parse("// whee"));
        TREE.root_node().child(0).unwrap()
    }

    fn check_parse_string_config<const RESULT_LEN: usize>(
        code: &str,
        expected: [(&str, Option<&str>); RESULT_LEN],
    ) {
        let comment = get_comment();
        let result = parse_string_config(code, comment);

        assert_that!(&result).is_equal_to(
            expected
                .into_iter()
                .map(|(key, value)| {
                    (
                        key.to_owned(),
                        StringConfig {
                            value: value.map(ToOwned::to_owned),
                            comment,
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
        );
    }

    #[test]
    fn test_parse_string_config_should_parse_string_config_with_one_item() {
        check_parse_string_config("a: true", [("a", Some("true"))]);
    }

    #[test]
    fn test_parse_string_config_should_parse_string_config_with_one_item_and_no_value() {
        check_parse_string_config("a", [("a", None)]);
    }

    #[test]
    fn test_parse_string_config_should_parse_string_config_with_two_items() {
        check_parse_string_config(
            "a: five b:three",
            [("a", Some("five")), ("b", Some("three"))],
        );
    }

    #[test]
    fn test_parse_string_config_should_parse_string_config_with_two_comma_separated_items() {
        check_parse_string_config(
            "a: seventy, b:ELEVENTEEN",
            [("a", Some("seventy")), ("b", Some("ELEVENTEEN"))],
        );
    }

    #[test]
    fn test_parse_string_config_should_parse_string_config_with_two_comma_separated_items_and_no_values(
    ) {
        check_parse_string_config("a , b", [("a", None), ("b", None)]);
    }
}
