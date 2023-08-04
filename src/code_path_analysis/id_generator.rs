use std::cell::Cell;

pub struct IdGenerator {
    prefix: String,
    n: Cell<u32>,
}

impl IdGenerator {
    pub fn new(prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();

        Self {
            prefix,
            n: Default::default(),
        }
    }

    pub fn next(&self) -> String {
        self.n.set(self.n.get() + 1);

        format!("{}{}", self.prefix, self.n.get())
    }
}
