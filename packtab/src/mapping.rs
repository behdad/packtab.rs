use std::collections::HashMap;

/// Bidirectional mapping that auto-assigns integer IDs to new keys.
///
/// Used during InnerLayer splitting: pairs of values like (a, b) are
/// mapped to compact integer IDs (0, 1, 2, ...) for the next level.
#[derive(Debug)]
pub struct AutoMapping {
    key_to_id: HashMap<(usize, usize), usize>,
    id_to_key: Vec<(usize, usize)>,
}

impl AutoMapping {
    pub fn new() -> Self {
        Self {
            key_to_id: HashMap::new(),
            id_to_key: Vec::new(),
        }
    }

    /// Get or insert: returns the ID for the given pair, auto-assigning
    /// the next sequential ID if the pair is new.
    pub fn get_or_insert(&mut self, key: (usize, usize)) -> usize {
        if let Some(&id) = self.key_to_id.get(&key) {
            return id;
        }
        let id = self.id_to_key.len();
        self.key_to_id.insert(key, id);
        self.id_to_key.push(key);
        id
    }

    /// Look up the ID for a given pair (read-only).
    pub fn get(&self, key: (usize, usize)) -> Option<usize> {
        self.key_to_id.get(&key).copied()
    }

    /// Look up the pair for a given ID.
    pub fn get_pair(&self, id: usize) -> (usize, usize) {
        self.id_to_key[id]
    }

    /// Number of unique pairs mapped.
    pub fn len(&self) -> usize {
        self.id_to_key.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bidirectional() {
        let mut m = AutoMapping::new();
        let v = m.get_or_insert((10, 20));
        assert_eq!(v, 0);
        assert_eq!(m.get_pair(0), (10, 20));
        assert_eq!(m.get_or_insert((10, 20)), 0);
    }

    #[test]
    fn test_sequential_ids() {
        let mut m = AutoMapping::new();
        let v0 = m.get_or_insert((1, 2));
        let v1 = m.get_or_insert((3, 4));
        assert_eq!(v0, 0);
        assert_eq!(v1, 1);
    }

    #[test]
    fn test_duplicate_key() {
        let mut m = AutoMapping::new();
        let v0 = m.get_or_insert((5, 6));
        let v1 = m.get_or_insert((5, 6));
        assert_eq!(v0, v1);
        assert_eq!(m.len(), 1);
    }
}
