pub struct IdGenerator {
    prefix: String,
    n: u32,
}

impl IdGenerator {
    pub fn new(prefix: impl Into<String>) -> Self {
        let prefix = prefix.into();

        Self { prefix, n: 0 }
    }

    pub fn next(&mut self) -> String {
        self.n += 1;

        format!("{}{}", self.prefix, self.n)
    }
}
