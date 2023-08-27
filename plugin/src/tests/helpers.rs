use speculoos::{AssertionFailure, Spec};
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

#[allow(dead_code)]
pub fn parse_typescript(source_text: &str) -> Tree {
    let mut parser = Parser::new();
    parser
        .set_language(SupportedLanguage::Typescript.language())
        .unwrap();
    parser.parse(source_text, None).unwrap()
}

pub fn tracing_subscribe() {
    run_once! {
        tracing_subscriber::fmt::init();
    }
}

pub trait IntoIteratorExt {
    fn has_count(&mut self, expected: usize);
    fn is_empty(&mut self);
}

impl<'s, TItem: 's, TIntoIterator> IntoIteratorExt for Spec<'s, TIntoIterator>
where
    &'s TIntoIterator: IntoIterator<Item = &'s TItem>,
{
    fn has_count(&mut self, expected: usize) {
        let subject = self.subject;

        if expected != subject.into_iter().count() {
            AssertionFailure::from_spec(self)
                .with_expected(format!("iterator with count <{expected}>"))
                .with_actual(format!("<{}>", subject.into_iter().count()))
                .fail();
        }
    }

    fn is_empty(&mut self) {
        let subject = self.subject;

        if subject.into_iter().next().is_some() {
            AssertionFailure::from_spec(self)
                .with_expected("empty iterator".to_owned())
                .with_actual(format!("iterator with <{}> items", subject.into_iter().count()))
                .fail();
        }
    }
}
