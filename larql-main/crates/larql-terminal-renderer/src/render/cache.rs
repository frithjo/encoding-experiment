use std::collections::HashMap;
use std::hash::Hash;

pub(crate) struct LruCache<K, V> {
    max_entries: usize,
    tick: u64,
    entries: HashMap<K, (V, u64)>,
}

impl<K, V> LruCache<K, V>
where
    K: Clone + Eq + Hash,
    V: Clone,
{
    pub(crate) fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            tick: 0,
            entries: HashMap::new(),
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<V> {
        self.tick = self.tick.wrapping_add(1);
        if let Some((value, last_used)) = self.entries.get_mut(key) {
            *last_used = self.tick;
            Some(value.clone())
        } else {
            None
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V) -> Option<(K, V)> {
        self.tick = self.tick.wrapping_add(1);
        self.entries.insert(key, (value, self.tick));

        if self.entries.len() <= self.max_entries {
            return None;
        }

        let evict_key = self
            .entries
            .iter()
            .min_by_key(|(_, (_, last_used))| *last_used)
            .map(|(key, _)| key.clone())?;
        self.entries
            .remove_entry(&evict_key)
            .map(|(key, (value, _))| (key, value))
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::LruCache;

    #[test]
    fn insert_evicts_least_recently_used_entry() {
        let mut cache = LruCache::new(2);
        assert!(cache.insert("a", 1).is_none());
        assert!(cache.insert("b", 2).is_none());
        assert_eq!(cache.get(&"a"), Some(1));

        let evicted = cache.insert("c", 3).unwrap();

        assert_eq!(evicted, ("b", 2));
        assert_eq!(cache.get(&"a"), Some(1));
        assert_eq!(cache.get(&"b"), None);
        assert_eq!(cache.get(&"c"), Some(3));
    }
}
