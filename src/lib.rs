use std::time::{Duration, Instant};
use indexmap::map::IndexMap;
use std::sync::{Arc, Mutex};
use tokio::time::interval;
use tokio::task::JoinHandle;

pub struct TLRUCache<K, V> {
    capacity: usize,
    ttl: Duration,
    map: Arc<Mutex<IndexMap<K, (V, Instant)>>>, // Key -> (Value, Expiry Time)
    eviction_handle: Option<JoinHandle<()>>,
}

impl<K: std::hash::Hash + Eq + Clone + Send + 'static, V: Clone + Send + 'static> TLRUCache<K, V> {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        let map = Arc::new(Mutex::new(IndexMap::new()));
        let map_clone = Arc::clone(&map);

        // Evict every 1 second or ttl/4, whichever is smaller
        let eviction_interval = std::cmp::min(ttl / 4, Duration::from_secs(1));
        
        let handle = tokio::spawn(async move {
            let mut interval_timer = interval(eviction_interval);
            loop {
                interval_timer.tick().await;
                let now = Instant::now();
                if let Ok(mut map_guard) = map_clone.lock() {
                    map_guard.retain(|_, (_, expiry)| *expiry > now);
                }
            }
        });

        Self {
            capacity,
            ttl,
            map,
            eviction_handle: Some(handle),
        }
    }

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

    // Manual eviction method for immediate cleanup (optional)
    pub fn evict_expired(&mut self) {
        if let Ok(mut map_guard) = self.map.lock() {
            let now = Instant::now();
            map_guard.retain(|_, (_, expiry)| *expiry > now);
        }
    }
}

impl<K, V> Drop for TLRUCache<K, V> {
    fn drop(&mut self) {
        if let Some(handle) = self.eviction_handle.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_sync() {
        fn is_send<T: Send>() {}
        fn is_sync<T: Sync>() {}
        is_send::<TLRUCache<String, String>>();
        is_sync::<TLRUCache<String, String>>();
    }

    #[tokio::test]
    async fn test_expiry() {
        let mut cache = TLRUCache::new(2, Duration::from_secs(1));
        cache.insert(1, "value1");
        cache.insert(2, "value2");
        assert_eq!(cache.get(&1), Some("value1"));
        assert_eq!(cache.get(&2), Some("value2"));
        tokio::time::sleep(Duration::from_secs(2)).await;
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), None);
    }

    #[tokio::test]
    async fn test_eviction() {
        let mut cache = TLRUCache::new(2, Duration::from_secs(1));
        cache.insert(1, "value1");
        cache.insert(2, "value2");
        cache.insert(3, "value3");
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some("value2"));
        assert_eq!(cache.get(&3), Some("value3"));
    }

    #[tokio::test]
    async fn test_lru_eviction() {
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