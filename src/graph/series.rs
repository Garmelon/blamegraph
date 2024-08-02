use std::fmt;

use serde::Serialize;

#[derive(Serialize)]
pub struct Series {
    pub name: String,
    pub values: Vec<i64>,
}

impl Series {
    pub fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            values: vec![],
        }
    }

    pub fn push<N>(&mut self, n: N)
    where
        N: TryInto<i64>,
        N::Error: fmt::Debug,
    {
        self.values.push(n.try_into().unwrap())
    }

    pub fn reverse(&mut self) {
        self.values.reverse();
    }

    pub fn add(&mut self, other: &Series) {
        assert!(self.values.len() == other.values.len());
        for (i, v) in other.values.iter().enumerate() {
            self.values[i] += v;
        }
    }
}
