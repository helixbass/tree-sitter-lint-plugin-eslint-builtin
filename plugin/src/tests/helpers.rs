use squalid::run_once;
use tree_sitter_lint::{
    tree_sitter::{Parser, Tree},
    tree_sitter_grep::SupportedLanguage,
};

pub fn parse(source_text: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(SupportedLanguage::Javascript.language())
        .unwrap();
    parser.parse(source_text, None).unwrap()
}

pub fn tracing_subscribe() {
    run_once! {
        tracing_subscriber::fmt::init();
    }
}
