//! A time-aware LRU cache implementation with automatic TTL-based eviction.
//! 
//! This crate provides a thread-safe, async-friendly cache that combines
//! LRU (Least Recently Used) eviction with time-based expiration.

use indexmap::map::IndexMap;
use std::time::Duration;
use std::sync::{Arc, Mutex};

#[cfg(not(test))]
use std::time::Instant;
#[cfg(test)]
use mock_instant::global::Instant;

/// A time-aware LRU cache that automatically evicts items based on both
/// capacity limits and time-to-live (TTL) expiration.
/// 
/// Items are automatically removed when they expire, and when the cache
/// reaches capacity, the least recently used items are evicted first.
pub struct TLRUCache<K, V> {
    capacity: usize,
    ttl: Duration,
    map: Arc<Mutex<IndexMap<K, (V, Instant)>>>,
}

impl<K: std::hash::Hash + Eq + Clone + Send + 'static, V: Clone + Send + 'static> TLRUCache<K, V> {
    /// Creates a new TLRU cache with the specified capacity and TTL.
    /// 
    /// # Arguments
    /// 
    /// * `capacity` - Maximum number of items the cache can hold
    /// * `ttl` - Time-to-live duration for cache entries
    /// 
    /// # Examples
    /// 
    /// ```
    /// use tlru_rs::TLRUCache;
    /// use std::time::Duration;
    /// 
    /// let cache: TLRUCache<i32, &'static str> = TLRUCache::new(100, Duration::from_secs(300));
    /// ```
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        let map = Arc::new(Mutex::new(IndexMap::new()));

        Self {
            capacity,
            ttl,
            map,
        }
    }

    /// Retrieves a value from the cache.
    /// 
    /// Returns `None` if the key doesn't exist or has expired.
    /// Accessing an item marks it as recently used.
    /// 
    /// # Examples
    /// 
    /// ```
    /// use tlru_rs::TLRUCache;
    /// use std::time::Duration;
    /// 
    /// let mut cache: TLRUCache<i32, &'static str> = TLRUCache::new(10, Duration::from_secs(60));
    /// cache.insert(1, "value");
    /// 
    /// assert_eq!(cache.get(&1), Some("value"));
    /// assert_eq!(cache.get(&2), None);
    /// ```
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Ok(mut map_guard) = self.map.lock() {
            if let Some((_, expiry)) = map_guard.get(key) {
                if Instant::now() < *expiry {
                    // Move to end to mark as recently used
                    if let Some((_, (value, expiry))) = map_guard.shift_remove_entry(key) {
                        let value_clone = value.clone();
                        map_guard.insert(key.clone(), (value, expiry));
                        return Some(value_clone);
                    }
                } else {
                    // Entry has expired, remove it and do not return it
                    map_guard.shift_remove_entry(key);
                }
            }
        }
        None
    }

    /// Inserts a key-value pair into the cache.
    /// 
    /// Returns the previous value if the key already existed.
    /// If the cache is at capacity, the least recently used item is evicted.
    /// 
    /// # Examples
    /// 
    /// ```
    /// use tlru_rs::TLRUCache;
    /// use std::time::Duration;
    /// 
    /// let mut cache: TLRUCache<i32, &'static str> = TLRUCache::new(10, Duration::from_secs(60));
    /// 
    /// // Insert new item
    /// assert_eq!(cache.insert(1, "value"), None);
    /// 
    /// // Update existing item
    /// assert_eq!(cache.insert(1, "new_value"), Some("value"));
    /// ```
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Ok(mut map_guard) = self.map.lock() {
            let mut previous_value = None;
            if map_guard.contains_key(&key) {
                previous_value = map_guard.shift_remove_entry(&key);
            }

            // Calculate expiry and insert
            let expiry = Instant::now() + self.ttl;
            map_guard.insert(key.clone(), (value, expiry));

            // Check capacity and evict if necessary
            if map_guard.len() > self.capacity {
                map_guard.shift_remove_index(0);
            }

            return previous_value.map(|(_, (value, _))| value);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use mock_instant::global::MockClock;

    use super::*;

    #[test]
    fn test_send_sync() {
        fn is_send<T: Send>() {}
        fn is_sync<T: Sync>() {}
        is_send::<TLRUCache<String, String>>();
        is_sync::<TLRUCache<String, String>>();
    }

    #[test]
    fn test_expiry() {
        let mut cache: TLRUCache<i32, &'static str> = TLRUCache::new(2, Duration::from_secs(1));
        MockClock::set_time(Duration::ZERO);
        cache.insert(1, "value1");
        cache.insert(2, "value2");
        assert_eq!(cache.get(&1), Some("value1"));
        assert_eq!(cache.get(&2), Some("value2"));
        MockClock::advance(Duration::from_secs(2));
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), None);
    }

    #[test]
    fn test_eviction() {
        let mut cache = TLRUCache::new(2, Duration::from_secs(1));
        cache.insert(1, "value1");
        cache.insert(2, "value2");
        cache.insert(3, "value3");
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some("value2"));
        assert_eq!(cache.get(&3), Some("value3"));
    }

    #[test]
    fn test_lru_eviction() {
        let mut cache = TLRUCache::new(2, Duration::from_secs(5));
        cache.insert("apple", 3);
        cache.insert("banana", 2);

        assert_eq!(cache.get(&"apple"), Some(3));
        assert_eq!(cache.get(&"banana"), Some(2));
        assert!(cache.get(&"pear").is_none());

        assert_eq!(cache.insert("banana", 4), Some(2));
        assert_eq!(cache.insert("pear", 5), None);

        assert_eq!(cache.get(&"pear"), Some(5));
        assert_eq!(cache.get(&"banana"), Some(4));
        assert!(cache.get(&"apple").is_none());
    }
}