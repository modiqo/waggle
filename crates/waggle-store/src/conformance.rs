//! The conformance suite: the contract's teeth (design doc `07 §5`).
//! Generic over any [`Store`]; backend crates call [`run_all`] from their
//! CI. Test names cite the contract clauses and gap-fixes they defend
//! (docs `07 §3`, `15 §4`).

use waggle_core::{
    mint, reconstruct, ActorClass, CanonicalUrl, Change, Channel, MintOptions, MintSpec,
    ResolverContext, Seq, Sharer, Stage, Timestamp,
};

use crate::error::StoreError;
use crate::traits::Store;
use crate::types::{AppendIntent, Appended, MintNonce};

/// Everything [`run_all`] needs: a way to build a fresh, empty store per
/// check (checks must not share state).
pub struct Harness<S, F: Fn() -> S> {
    fresh: F,
}

impl<S: Store, F: Fn() -> S> Harness<S, F> {
    /// Build a harness from a fresh-store factory.
    pub fn new(fresh: F) -> Self {
        Self { fresh }
    }
}

fn manifest_for(seed: u8, parent: Option<waggle_core::Token>) -> waggle_core::AttributionManifest {
    let mut entropy = move |buf: &mut [u8]| {
        for (i, b) in buf.iter_mut().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            {
                *b = seed.wrapping_mul(31).wrapping_add(i as u8);
            }
        }
        Ok(())
    };
    let mut spec = MintSpec::new(
        CanonicalUrl::new("ws://conformance/artifact").unwrap(),
        Sharer::new("lead").unwrap(),
        Channel::subagent_general(),
    );
    if let Some(p) = parent {
        spec = spec.child_of(p);
    }
    mint(
        spec,
        &MintOptions::default(),
        &mut entropy,
        Timestamp::from_unix_ms(u64::from(seed)),
    )
    .unwrap()
}

fn event_intent(token: waggle_core::Token, stage: Stage) -> AppendIntent {
    AppendIntent::Event {
        token,
        stage,
        actor: ActorClass::from_context(&ResolverContext::anonymous_agent()),
        variant: None,
        regions: None,
        at: Timestamp::from_unix_ms(7),
    }
}

/// Run every conformance check. Panics (test-style) on the first failure —
/// call from a `#[test]`.
pub fn run_all<S: Store, F: Fn() -> S>(harness: &Harness<S, F>) {
    pollster::block_on(async {
        c3_seq_monotonic_per_token(&(harness.fresh)()).await;
        c6_read_your_mint(&(harness.fresh)()).await;
        c7_revoked_parent_rejected(&(harness.fresh)()).await;
        g5_mint_nonce_idempotent(&(harness.fresh)()).await;
        g4_cas_lifecycle_mutations(&(harness.fresh)()).await;
        c2_append_only_scan_grows(&(harness.fresh)()).await;
        r4_views_agree_with_fold(&(harness.fresh)()).await;
    });
}

async fn c3_seq_monotonic_per_token<S: Store>(store: &S) {
    let m = manifest_for(1, None);
    let token = m.token;
    store
        .append(AppendIntent::Mint {
            manifest: Box::new(m),
            nonce: MintNonce(1),
        })
        .await
        .unwrap();
    let mut last = Seq(0);
    for _ in 0..5 {
        let Appended::Event { seq } = store
            .append(event_intent(token, Stage::resolve()))
            .await
            .unwrap()
        else {
            panic!("event intent must yield event receipt")
        };
        assert!(seq > last, "C-3: per-token seq must be monotonic");
        last = seq;
    }
}

async fn c6_read_your_mint<S: Store>(store: &S) {
    let m = manifest_for(2, None);
    let token = m.token;
    store
        .append(AppendIntent::Mint {
            manifest: Box::new(m),
            nonce: MintNonce(2),
        })
        .await
        .unwrap();
    assert!(
        store.manifest(token).await.unwrap().is_some(),
        "C-6: an acked mint must be observable"
    );
}

async fn c7_revoked_parent_rejected<S: Store>(store: &S) {
    let parent = manifest_for(3, None);
    let parent_token = parent.token;
    let Appended::Minted { view, .. } = store
        .append(AppendIntent::Mint {
            manifest: Box::new(parent),
            nonce: MintNonce(3),
        })
        .await
        .unwrap()
    else {
        panic!("mint intent must yield mint receipt")
    };
    // A child before revocation is fine.
    let early_child = manifest_for(4, Some(parent_token));
    let early = early_child.token;
    store
        .append(AppendIntent::Mint {
            manifest: Box::new(early_child),
            nonce: MintNonce(4),
        })
        .await
        .unwrap();

    store
        .append(AppendIntent::Mutate {
            token: parent_token,
            change: Change::Revoked,
            expected_version: Some(view.version()),
            at: Timestamp::from_unix_ms(50),
        })
        .await
        .unwrap();

    let late_child = manifest_for(5, Some(parent_token));
    let err = store
        .append(AppendIntent::Mint {
            manifest: Box::new(late_child),
            nonce: MintNonce(5),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::ParentRevoked(t) if t == parent_token),
        "C-7: children cannot be minted under a tombstone"
    );
    let children = store.children(parent_token).await.unwrap();
    assert_eq!(
        children,
        vec![early],
        "children minted before revocation remain visible"
    );
}

async fn g5_mint_nonce_idempotent<S: Store>(store: &S) {
    let m = manifest_for(6, None);
    let original = m.token;
    let first = store
        .append(AppendIntent::Mint {
            manifest: Box::new(m),
            nonce: MintNonce(6),
        })
        .await
        .unwrap();
    let Appended::Minted {
        replayed: false, ..
    } = first
    else {
        panic!("first mint must be fresh")
    };
    // A retry mints a *different* manifest under the *same* nonce.
    let retry = manifest_for(7, None);
    let Appended::Minted {
        view,
        replayed: true,
    } = store
        .append(AppendIntent::Mint {
            manifest: Box::new(retry),
            nonce: MintNonce(6),
        })
        .await
        .unwrap()
    else {
        panic!("G-5/C-8: duplicate nonce must replay, not error, not duplicate")
    };
    assert_eq!(
        view.manifest.token, original,
        "replay returns the ORIGINAL token"
    );
    // Distinct nonce mints fresh.
    let other = manifest_for(8, None);
    let Appended::Minted {
        replayed: false, ..
    } = store
        .append(AppendIntent::Mint {
            manifest: Box::new(other),
            nonce: MintNonce(7),
        })
        .await
        .unwrap()
    else {
        panic!("distinct nonce must mint fresh")
    };
}

async fn g4_cas_lifecycle_mutations<S: Store>(store: &S) {
    let m = manifest_for(9, None);
    let token = m.token;
    store
        .append(AppendIntent::Mint {
            manifest: Box::new(m),
            nonce: MintNonce(9),
        })
        .await
        .unwrap();

    // Lifecycle without a version: refused, fix named.
    let err = store
        .append(AppendIntent::Mutate {
            token,
            change: Change::Revoked,
            expected_version: None,
            at: Timestamp::from_unix_ms(1),
        })
        .await
        .unwrap_err();
    assert!(
        matches!(err, StoreError::LifecycleRequiresVersion(_)),
        "C-9: CAS is mandatory"
    );

    // Cosmetic without a version: fine (LWW).
    store
        .append(AppendIntent::Mutate {
            token,
            change: Change::LabelSet {
                key: "campaign".into(),
                value: "q3".into(),
            },
            expected_version: None,
            at: Timestamp::from_unix_ms(2),
        })
        .await
        .unwrap();

    // Stale CAS: conflict carries both versions.
    let err = store
        .append(AppendIntent::Mutate {
            token,
            change: Change::ExpirySet {
                expires_at: Some(Timestamp::from_unix_ms(99)),
            },
            expected_version: Some(41),
            at: Timestamp::from_unix_ms(3),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        StoreError::Conflict {
            expected: 41,
            actual: 1,
            ..
        }
    ));

    // Correct CAS: version bumps.
    let Appended::Mutated { version, .. } = store
        .append(AppendIntent::Mutate {
            token,
            change: Change::ExpirySet {
                expires_at: Some(Timestamp::from_unix_ms(99)),
            },
            expected_version: Some(1),
            at: Timestamp::from_unix_ms(4),
        })
        .await
        .unwrap()
    else {
        panic!("mutate intent must yield mutate receipt")
    };
    assert_eq!(version, 2);
}

async fn c2_append_only_scan_grows<S: Store>(store: &S) {
    let m = manifest_for(10, None);
    let token = m.token;
    store
        .append(AppendIntent::Mint {
            manifest: Box::new(m),
            nonce: MintNonce(10),
        })
        .await
        .unwrap();
    let before = store.scan_token(token, Seq(0)).await.unwrap().len();
    store
        .append(event_intent(token, Stage::run()))
        .await
        .unwrap();
    let after = store.scan_token(token, Seq(0)).await.unwrap();
    assert_eq!(after.len(), before + 1, "C-2: the log only grows");
    assert!(
        after.windows(2).all(|w| w[0].seq() <= w[1].seq()),
        "scan is seq-ordered"
    );
}

async fn r4_views_agree_with_fold<S: Store>(store: &S) {
    let m = manifest_for(11, None);
    let token = m.token;
    store
        .append(AppendIntent::Mint {
            manifest: Box::new(m),
            nonce: MintNonce(11),
        })
        .await
        .unwrap();
    for stage in [
        Stage::resolve(),
        Stage::run(),
        Stage::run(),
        Stage::repeat(),
    ] {
        store.append(event_intent(token, stage)).await.unwrap();
    }
    store
        .append(AppendIntent::Mutate {
            token,
            change: Change::Revoked,
            expected_version: Some(1),
            at: Timestamp::from_unix_ms(20),
        })
        .await
        .unwrap();

    // The materialized answers must equal the fold over the log (R-4).
    let world = reconstruct(store.scan_all().await.unwrap());
    let view = store.manifest(token).await.unwrap().unwrap();
    assert_eq!(
        *view.manifest, world.manifests[&token],
        "manifest view ≡ fold"
    );
    let funnel = store.funnel(token).await.unwrap();
    assert_eq!(funnel, world.funnels[&token], "funnel view ≡ fold");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryStore;

    #[test]
    fn memory_backend_passes_conformance() {
        run_all(&Harness::new(MemoryStore::default));
    }
}
