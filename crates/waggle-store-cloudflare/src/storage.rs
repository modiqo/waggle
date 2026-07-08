//! The five-verb storage seam (design doc `08 §8`): everything the edge
//! engine needs from a backing store, and nothing more. The Durable
//! Object implements it over DO storage; tests implement it over a
//! `BTreeMap` — the engine cannot tell the difference, which is the
//! point: one engine, certified natively, deployed in workerd.
//!
//! Single-writer assumption: callers guarantee appends are serialized
//! (the DO's execution model; a mutex in the fake). The engine never
//! defends against concurrent writers — the runtime shape does.

use waggle_store::StoreError;

/// Ordered key-value with prefix listing — the least storage that can
/// host the log and its views.
pub trait EdgeStorage {
    /// Read one key.
    async fn get(&self, key: &str) -> Result<Option<String>, StoreError>;
    /// Write one key.
    async fn put(&self, key: &str, value: &str) -> Result<(), StoreError>;
    /// Delete one key (views only — the log is never deleted).
    async fn delete(&self, key: &str) -> Result<(), StoreError>;
    /// All `(key, value)` pairs under a prefix, key-ordered.
    async fn list(&self, prefix: &str) -> Result<Vec<(String, String)>, StoreError>;
}

/// The in-memory fake: a mutexed `BTreeMap`. Fast conformance, the
/// differential oracle, and zero node/wrangler anywhere near CI's
/// native jobs.
#[derive(Debug, Default)]
pub struct MemoryEdgeStorage {
    inner: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
}

impl EdgeStorage for MemoryEdgeStorage {
    async fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self.lock()?.get(key).cloned())
    }
    async fn put(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.lock()?.insert(key.to_owned(), value.to_owned());
        Ok(())
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.lock()?.remove(key);
        Ok(())
    }
    async fn list(&self, prefix: &str) -> Result<Vec<(String, String)>, StoreError> {
        Ok(self
            .lock()?
            .range(prefix.to_owned()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }
}

impl MemoryEdgeStorage {
    fn lock(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, std::collections::BTreeMap<String, String>>, StoreError>
    {
        self.inner
            .lock()
            .map_err(|_| StoreError::Backend("edge storage lock poisoned".into()))
    }
}
