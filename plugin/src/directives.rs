use once_cell::sync::Lazy;
use regex::Regex;

pub static directives_pattern: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"^(eslint(?:-env|-enable|-disable(?:(?:-next)?-line)?)?|exported|globals?)(?:\s|$)"#,
    )
    .unwrap()
});
