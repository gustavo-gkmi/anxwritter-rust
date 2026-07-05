//! Sequential `ID{n}` minting, mirroring `builder._next_id`.
//!
//! IDs are allocated in a canonical order during the build so that streamed and
//! buffered output stay consistent. The counter starts at 0 and pre-increments,
//! so the first minted id is `ID1`.

/// Monotonic allocator of `ID{n}` strings.
#[derive(Debug, Default, Clone)]
pub struct IdCounter {
    counter: u64,
}

impl IdCounter {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    /// Mint the next `ID{n}`.
    pub fn next_id(&mut self) -> String {
        self.counter += 1;
        format!("ID{}", self.counter)
    }

    /// How many ids have been minted so far.
    pub fn count(&self) -> u64 {
        self.counter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_one_and_increments() {
        let mut c = IdCounter::new();
        assert_eq!(c.next_id(), "ID1");
        assert_eq!(c.next_id(), "ID2");
        assert_eq!(c.count(), 2);
    }
}
