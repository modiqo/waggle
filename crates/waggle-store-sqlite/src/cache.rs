//! The hot-read cache layer, isolated so it can be model-checked
//! (design doc `15 §5.2`: "the loom suite scoped to the cache layer").
//!
//! Semantics the store depends on — and loom verifies over every
//! interleaving: a `drop_key` that completes is never overtaken by a
//! stale value it raced with (readers between drop and re-put miss and
//! fall through to the anchor), and no interleaving deadlocks. The cache
//! accelerates; the anchor answers; staleness is bounded to the commit
//! path that always drops/puts strictly AFTER the transaction commits.

#[cfg(loom)]
use loom::sync::RwLock;
#[cfg(not(loom))]
use std::sync::RwLock;

use std::collections::HashMap;
use std::hash::Hash;

/// A read-through cache: `RwLock<HashMap>` with poison-tolerant access
/// (a poisoned cache degrades to misses, never to wrong answers).
#[derive(Debug)]
pub struct Cache<K, V> {
    inner: RwLock<HashMap<K, V>>,
}

impl<K: Eq + Hash + Clone, V: Clone> Cache<K, V> {
    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    /// Fetch a clone of the cached value, if present.
    pub fn get(&self, key: &K) -> Option<V> {
        self.inner.read().ok().and_then(|map| map.get(key).cloned())
    }

    /// Insert/replace a value (the post-commit publish).
    pub fn put(&self, key: K, value: V) {
        if let Ok(mut map) = self.inner.write() {
            map.insert(key, value);
        }
    }

    /// Invalidate a key (the post-commit drop for lifecycle changes).
    pub fn drop_key(&self, key: &K) {
        if let Ok(mut map) = self.inner.write() {
            map.remove(key);
        }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> Default for Cache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
