# tlru-rs

A time-aware LRU (Least Recently Used) cache implementation in Rust with automatic TTL-based eviction.

This crate provides a thread-safe, async-friendly cache that combines LRU eviction with time-based expiration. Items are automatically removed when they expire, and when the cache reaches capacity, the least recently used items are evicted first.

## Features

- **Time-aware**: Automatic expiration based on configurable TTL
- **LRU eviction**: Least recently used items are evicted when capacity is reached
- **Thread-safe**: Uses `Arc<Mutex<>>` for concurrent access

## Dependencies

- [indexmap](https://github.com/indexmap-rs/indexmap) - For maintaining insertion order and efficient LRU operations

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
tlru-rs = "0.1.0"
```

### Basic Usage

```rust
use tlru_rs::TLRUCache;
use std::time::Duration;

fn main() {
    // Create a cache with capacity 100 and 5-minute TTL
    let mut cache = TLRUCache::new(100, Duration::from_secs(300));
    
    // Insert items
    cache.insert("key1", "value1");
    cache.insert("key2", "value2");
    
    // Retrieve items (accessing marks them as recently used)
    if let Some(value) = cache.get(&"key1") {
        println!("Found: {}", value);
    }
    
    // Items automatically expire after TTL
    // LRU eviction occurs when capacity is reached
}
```

## API Reference

### `TLRUCache<K, V>`

A time-aware LRU cache that automatically evicts items based on both capacity limits and time-to-live (TTL) expiration.

#### Type Constraints

- `K`: Must implement `std::hash::Hash + Eq + Clone + Send + 'static`
- `V`: Must implement `Clone + Send + 'static`

#### Methods

##### `new(capacity: usize, ttl: Duration) -> Self`

Creates a new TLRU cache with the specified capacity and TTL.

- **capacity**: Maximum number of items the cache can hold
- **ttl**: Time-to-live duration for cache entries

##### `get(&mut self, key: &K) -> Option<V>`

Retrieves a value from the cache.

- Returns `None` if the key doesn't exist or has expired
- Accessing an item marks it as recently used (moves it to the end of the LRU list)
- Automatically removes expired entries

##### `insert(&mut self, key: K, value: V) -> Option<V>`

Inserts a key-value pair into the cache.

- Returns the previous value if the key already existed
- If the cache is at capacity, the least recently used item is evicted
- Sets the expiration time to current time + TTL

## Thread Safety

The cache is thread-safe and can be shared between threads using `Arc<TLRUCache<K, V>>`. The internal mutex ensures safe concurrent access.

## License

MIT License - see [LICENSE](LICENSE) file for details. 
