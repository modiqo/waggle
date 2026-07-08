//! The loom suite, scoped to the cache layer (design doc `15 §5.2`):
//! model-check every interleaving of the reader/committer race on the
//! REAL cache type. Run via `just loom` (needs `RUSTFLAGS="--cfg loom"`).

#![cfg(loom)]

use loom::sync::Arc;
use loom::thread;
use waggle_store_sqlite::cache::Cache;

/// The race that matters (G-2 at the cache layer): a committer invalidates
/// while a reader reads. Every interleaving must end with the reader
/// having seen either the old value or a miss — NEVER a torn state — and
/// once the drop completes, fresh readers miss until the re-put.
#[test]
fn invalidation_never_leaves_stale_after_drop() {
    loom::model(|| {
        let cache = Arc::new(Cache::new());
        cache.put(1u8, 10u32);

        let committer = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || {
                cache.drop_key(&1); // lifecycle commit invalidates
            })
        };
        let reader = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || cache.get(&1))
        };

        let seen = reader.join().unwrap();
        committer.join().unwrap();
        // Racing reader: old value or miss — both honest.
        assert!(matches!(seen, None | Some(10)));
        // Post-drop, the cache MUST miss (the anchor answers next).
        assert_eq!(cache.get(&1), None, "a completed drop is never overtaken");
    });
}

/// Two committers publishing different post-commit states plus a reader:
/// the reader sees one of the published values or a miss — never a
/// mixture, never a deadlock (loom explores all schedules).
#[test]
fn concurrent_puts_and_reads_are_atomic() {
    loom::model(|| {
        let cache = Arc::new(Cache::new());
        let a = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || cache.put(1u8, 111u32))
        };
        let b = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || cache.put(1u8, 222u32))
        };
        let reader = {
            let cache = Arc::clone(&cache);
            thread::spawn(move || cache.get(&1))
        };
        let seen = reader.join().unwrap();
        a.join().unwrap();
        b.join().unwrap();
        assert!(matches!(seen, None | Some(111) | Some(222)));
        // Afterward: exactly one of the two published values.
        assert!(matches!(cache.get(&1), Some(111) | Some(222)));
    });
}
