use squalid::OptionExt;

// https://stackoverflow.com/a/38406885/732366
pub fn upper_case_first(string: &str) -> String {
    let mut chars = string.chars();
    chars
        .next()
        .map_or_default(|first| first.to_uppercase().collect::<String>() + chars.as_str())
}
