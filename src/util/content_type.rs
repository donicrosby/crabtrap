use crate::Error;
use mime::{self, Mime};
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct ContentType {
    inner: Mime,
}

impl ContentType {
    pub fn new(str: &str) -> Result<Self, Error> {
        let inner = str.parse()?;
        Ok(Self { inner })
    }
}

impl From<Mime> for ContentType {
    fn from(value: Mime) -> Self {
        Self { inner: value }
    }
}

impl ToString for ContentType {
    fn to_string(&self) -> String {
        self.inner.to_string()
    }
}

impl FromStr for ContentType {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}
