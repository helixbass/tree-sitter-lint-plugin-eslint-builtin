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

#[macro_export]
macro_rules! return_default_if_none {
    ($expr:expr) => {
        match $expr {
            None => return Default::default(),
            Some(value) => value,
        }
    };
}
