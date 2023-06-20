# Cache in rs

## Description

This is a simple cache implementation in Rust. It is a learning project for me to get to know the language and its ecosystem.

this library includes

- a simple cache (pure hash map)
- a lru cache (double linked list + hash map)
- a ttl cache (delete expired entries on read, periodically may too heavy for rust, but we could use grean thread to do it)
- a concurrent safe cache (use RwLock to wrap the cache)
