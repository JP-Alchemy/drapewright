use soom_pattern_core::Id;

/// Hands out project-wide unique ids such as `p7` or `e42`. One counter is
/// shared by all prefixes, so ids never collide even across kinds.
#[derive(Clone, Debug)]
pub struct IdAllocator {
    next: u64,
}

impl IdAllocator {
    /// Starts above every number already used in `existing` ids.
    pub fn after<'a>(existing: impl IntoIterator<Item = &'a Id>) -> Self {
        let max = existing
            .into_iter()
            .filter_map(|id| {
                let digits = id.len() - id.trim_end_matches(|c: char| c.is_ascii_digit()).len();
                id[id.len() - digits..].parse::<u64>().ok()
            })
            .max()
            .unwrap_or(0);
        IdAllocator { next: max + 1 }
    }

    pub fn next(&mut self, prefix: &str) -> Id {
        let id = format!("{prefix}{}", self.next);
        self.next += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continues_after_the_largest_number() {
        let existing: Vec<Id> = ["p3", "e12", "custom", "n7"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut ids = IdAllocator::after(&existing);
        assert_eq!(ids.next("p"), "p13");
        assert_eq!(ids.next("e"), "e14");
    }
}
