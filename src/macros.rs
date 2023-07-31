#[macro_export]
macro_rules! break_if_none {
    ($expr:expr) => {
        match $expr {
            None => break,
            Some(value) => value,
        }
    };
}

#[macro_export]
macro_rules! continue_if_none {
    ($expr:expr) => {
        match $expr {
            None => continue,
            Some(value) => value,
        }
    };
}
