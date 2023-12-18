use std::collections::HashMap;

use squalid::regex;
use tree_sitter_lint::{better_any::tid, tree_sitter::Node, FileRunContext, FromFileRunContext};

use crate::{
    all_comments::AllComments, ast_helpers::get_comment_contents, conf::globals,
    directives::directives_pattern, scope::config_comment_parser,
};

fn extract_directive_comment(value: &str) -> (&str, &str) {
    let Some(match_) = regex!(r#"\s-{2,}\s"#).find(value) else {
        return (value.trim(), "");
    };

    let directive = &value[..match_.start()].trim();
    let justification = &value[match_.end()..].trim();

    (directive, justification)
}

pub struct DirectiveComments<'a> {
    pub enabled_globals: HashMap<String, EnabledGlobal<'a>>,
}

tid! { impl<'a> TidAble<'a> for DirectiveComments<'a> }

impl<'a> FromFileRunContext<'a> for DirectiveComments<'a> {
    fn from_file_run_context(file_run_context: FileRunContext<'a, '_>) -> Self {
        let mut enabled_globals: HashMap<String, EnabledGlobal<'a>> = Default::default();

        file_run_context
            .retrieve::<AllComments<'a>>()
            .iter()
            .for_each(|&comment| {
                let comment_contents = get_comment_contents(comment, &file_run_context);
                let (directive_part, _justification_part) =
                    extract_directive_comment(&comment_contents);

                let Some(match_) = directives_pattern.captures(directive_part) else {
                    return;
                };
                let directive_text = match_.get(1).unwrap();
                let directive_value = &directive_part[directive_text.end()..];
                let directive_text = directive_text.as_str();

                match directive_text {
                    "globals" | "global" => {
                        for (id, string_config) in
                            config_comment_parser::parse_string_config(directive_value, comment)
                        {
                            let normalized_value = match serde_json::from_str::<globals::Visibility>(
                                string_config.value.as_deref().unwrap_or(r#""readonly""#)
                            ) {
                                Ok(visibility) => visibility,
                                Err(_) => unimplemented!("{:?}", string_config),
                            };

                            let enabled_global = enabled_globals.entry(id).or_insert_with(|| {
                                EnabledGlobal {
                                    value: normalized_value,
                                    comments: Default::default(),
                                }
                            });
                            enabled_global.value = normalized_value;
                            enabled_global.comments.push(comment);
                        }
                    }
                    _ => (),
                }
            });

        DirectiveComments { enabled_globals }
    }
}

#[derive(Debug)]
pub struct EnabledGlobal<'a> {
    pub comments: Vec<Node<'a>>,
    pub value: globals::Visibility,
}
