use std::sync::Arc;

use squalid::fancy_regex;
use tree_sitter_lint::{rule, violation, Rule};

use crate::ast_helpers::NodeExtJs;

pub fn no_octal_escape_rule() -> Arc<dyn Rule> {
    rule! {
        name => "no-octal-escape",
        languages => [Javascript],
        messages => [
            octal_escape_sequence => r#"Don't use octal: '\{{sequence}}'. Use '\u....' instead."#,
        ],
        listeners => [
            r#"
              (string) @c
            "# => |node, context| {
                if let Ok(Some(captures)) = fancy_regex!(
                    r#"(?m)^(?:[^\\]|\\.)*?\\([0-3][0-7]{1,2}|[4-7][0-7]|0(?=[89])|[1-7])"#
                ).captures(&node.text(context)) {
                    context.report(violation! {
                        node => node,
                        message_id => "octal_escape_sequence",
                        data => { sequence => captures[1] }
                    });
                }
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use crate::kind;

    use super::*;

    use tree_sitter_lint::{rule_tests, RuleTester};

    #[test]
    fn test_no_octal_escape_rule() {
        RuleTester::run(
            no_octal_escape_rule(),
            rule_tests! {
            valid => [
                "var foo = \"\\x51\";",
                "var foo = \"foo \\\\251 bar\";",
                "var foo = /([abc]) \\1/g;",
                "var foo = '\\0';",
                "'\\0'",
                "'\\8'",
                "'\\9'",
                "'\\0 '",
                "' \\0'",
                "'a\\0'",
                "'\\0a'",
                "'a\\8a'",
                "'\\0\\8'",
                "'\\8\\0'",
                "'\\80'",
                "'\\81'",
                "'\\\\'",
                "'\\\\0'",
                "'\\\\08'",
                "'\\\\1'",
                "'\\\\01'",
                "'\\\\12'",
                "'\\\\\\0'",
                "'\\\\\\8'",
                "'\\0\\\\'",
                "'0'",
                "'1'",
                "'8'",
                "'01'",
                "'08'",
                "'80'",
                "'12'",
                "'\\a'",
                "'\\n'"
            ],
            invalid => [
                { code => "var foo = \"foo \\01 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "var foo = \"foo \\000 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "000" }, type => kind::String }] },
                { code => "var foo = \"foo \\377 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "377" }, type => kind::String }] },
                { code => "var foo = \"foo \\378 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "37" }, type => kind::String }] },
                { code => "var foo = \"foo \\37a bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "37" }, type => kind::String }] },
                { code => "var foo = \"foo \\381 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "3" }, type => kind::String }] },
                { code => "var foo = \"foo \\3a1 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "3" }, type => kind::String }] },
                { code => "var foo = \"foo \\251 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "251" }, type => kind::String }] },
                { code => "var foo = \"foo \\258 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "25" }, type => kind::String }] },
                { code => "var foo = \"foo \\25a bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "25" }, type => kind::String }] },
                { code => "var foo = \"\\3s51\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "3" }, type => kind::String }] },
                { code => "var foo = \"\\77\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "77" }, type => kind::String }] },
                { code => "var foo = \"\\78\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "7" }, type => kind::String }] },
                { code => "var foo = \"\\5a\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "5" }, type => kind::String }] },
                { code => "var foo = \"\\751\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "75" }, type => kind::String }] },
                { code => "var foo = \"foo \\400 bar\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "40" }, type => kind::String }] },

                { code => "var foo = \"\\t\\1\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "var foo = \"\\\\\\751\";", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "75" }, type => kind::String }] },

                { code => "'\\0\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\0 \\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\0\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\0 \\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\0a\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\0a\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\0\\08'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "0" }, type => kind::String }] },

                { code => "'\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\2'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "2" }, type => kind::String }] },
                { code => "'\\7'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "7" }, type => kind::String }] },
                { code => "'\\00'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "00" }, type => kind::String }] },
                { code => "'\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\02'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "02" }, type => kind::String }] },
                { code => "'\\07'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "07" }, type => kind::String }] },
                { code => "'\\08'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "0" }, type => kind::String }] },
                { code => "'\\09'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "0" }, type => kind::String }] },
                { code => "'\\10'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "10" }, type => kind::String }] },
                { code => "'\\12'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "12" }, type => kind::String }] },
                { code => "' \\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\1 '", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'a\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\1a'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'a\\1a'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "' \\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\01 '", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'a\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\01a'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'a\\01a'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'a\\08a'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "0" }, type => kind::String }] },
                { code => "'\\n\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\n\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\n\\08'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "0" }, type => kind::String }] },
                { code => "'\\\\\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },
                { code => "'\\\\\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\\\\\08'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "0" }, type => kind::String }] },

                // Multiline string
                { code => "'\\\n\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] },

                // Only the first one is reported
                { code => "'\\01\\02'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\02\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "02" }, type => kind::String }] },
                { code => "'\\01\\2'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "01" }, type => kind::String }] },
                { code => "'\\2\\01'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "2" }, type => kind::String }] },
                { code => "'\\08\\1'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "0" }, type => kind::String }] },
                { code => "'foo \\1 bar \\2'", errors => [{ message_id => "octal_escape_sequence", data => { sequence => "1" }, type => kind::String }] }
            ]
            },
        )
    }
}
