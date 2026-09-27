use std::{
    collections::{HashMap, VecDeque},
    hash::Hash,
    time::{Duration, Instant},
};

struct Entry<V> {
    value: V,
    expires_at: Instant,
}

pub struct Cache<K, V> {
    entries: HashMap<K, Entry<V>>,
    order: VecDeque<K>,
    capacity: usize,
    ttl: Duration,
}

impl<K: Eq + Hash + Clone, V> Cache<K, V> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            capacity,
            ttl,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        let expired = self
            .entries
            .get(key)
            .map(|entry| entry.expires_at <= Instant::now())
            .unwrap_or(false);
        if expired {
            self.remove(key);
            return None;
        }
        if self.entries.contains_key(key) {
            self.order.retain(|item| item != key);
            self.order.push_back(key.clone());
        }
        self.entries.get(key).map(|entry| &entry.value)
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.remove(&key);
        self.entries.insert(
            key.clone(),
            Entry {
                value,
                expires_at: Instant::now() + self.ttl,
            },
        );
        self.order.push_back(key);
        while self.entries.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.order.retain(|item| item != key);
        self.entries.remove(key).map(|entry| entry.value)
    }

    pub fn contains(&mut self, key: &K) -> bool {
        self.get(key).is_some()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::Cache;
    use std::time::Duration;

    #[test]
    fn cache_expires_and_respects_capacity() {
        let mut cache = Cache::new(1, Duration::from_millis(1));
        cache.insert("a", 1);
        std::thread::sleep(Duration::from_millis(2));
        assert!(cache.get(&"a").is_none());
        cache.insert("a", 1);
        cache.insert("b", 2);
        assert_eq!(cache.len(), 1);
    }
}
