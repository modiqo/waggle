# 15 — The Concurrency & Consistency Model

*New in revision 2.1. Born from an adversarial scenario review (multi-read,
multi-write, stale reads, retries, edge propagation) that found eight genuine
gaps in the rev-2 design. This document (a) names the consistency model
precisely, (b) records the scenario catalog with verdicts, (c) tracks the
eight gap-fixes G-1..G-8 to completion, and (d) specifies the unit,
property, model-checking, and integration tests that verify each. 14's
checkpoints cite these test IDs as acceptance gates.*

> **Revision 2.2 note (SQLite pivot — see 07 §4, 13 §8).** The correctness
> anchor for the local backend is now SQLite/WAL, which **provides several
> fixes by construction** rather than by our code: G-2's no-skew guarantee =
> WAL snapshot read transactions; G-1's interning safety = the single-writer
> transaction; G-4's CAS = `UPDATE … WHERE version = ?`; G-5's dedupe = a
> UNIQUE index. The gap table below is annotated accordingly. What remains
> ours to prove: the **hot cache layer** (arc-swap manifest cache over the
> anchor — loom suite §5.2 now scopes to its publication/invalidation
> ordering), the two-lane intake (G-6), and every semantic contract test in
> §5 — which run unchanged against the SQLite backend, because the contract
> outranks the mechanism. §2's `ReadState` structure survives as the
> **cache's** design; the anchor's snapshots come from WAL.

## 1. The model, stated precisely

> **Single total commit order** — all writes serialize through one committer
> per deployment. **Prefix-consistent snapshots** — every read observes one
> atomic `ReadState` representing a prefix of the commit order: never holes,
> never reordering, never skew between manifests, events, and intern tables.
> **Read-your-write** holds within one server; at the edge it is preserved by
> authoritative read-through (never by cache luck). **Resolutions are
> point-in-time** and say so (`as_of`, `revalidate_after`). **Lifecycle
> mutations are CAS; only cosmetic mutations are LWW. Every write tool is
> idempotent under retry.** Cross-PoP reads are eventual by default with a
> `strict` escape hatch that manifests can mandate.

## 2. The keystone structure: one atomic `ReadState`

Rev-2 published the manifest map, watermark, and (implicitly) intern tables
as separate atomics — allowing read skew (§3 A4). Rev-2.1 unifies:

```rust
/// The entire readable world at one commit prefix. Swapped atomically by the
/// committer (arc-swap); readers grab one Arc and see one consistent prefix.
/// Upholds G-2; the snapshot writer serializes exactly this (D2).
pub struct ReadState {
    segments: Arc<[Arc<Segment>]>,   // sealed, immutable (A5)
    tail: Arc<Segment>,              // fixed-capacity; rows < watermark valid
    watermark: u32,                  // rows visible in `tail` at this prefix
    manifests: ManifestMap,          // persistent map, versioned per token
    interns: InternTables,           // committer-owned; immutable in snapshot (G-1)
    prefix_seq: u64,                 // global commit counter — the prefix id
}
```

The committer is the only mutator of any of it. Interning (StageId/TokenId
assignment) happens **only** in the committer (G-1); a record referencing a
new stage is interned as part of its commit.

Committer intake is **two-lane** (G-6): a durable lane (mint, lifecycle
mutations, durable records) and a relaxed lane (analytics events). Drain
policy: durable-first with an anti-starvation quantum for the relaxed lane
(e.g., at least 1 relaxed batch per N durable batches). A mint never queues
behind an analytics storm.

**Load-bearing coupling, engraved:** tail torn-read safety relies on events
being **fixed-width**, which is a consequence of invariant I-1 (no payload).
If any variable-length field is ever proposed for `Event`, this proof
obligation reopens. (A3)

## 3. Scenario catalog (the adversarial review, recorded)

| ID | Scenario | Example | Verdict → resolution |
|---|---|---|---|
| A1 | read–read, same token | 8 subagents resolve `wg:7Kp2` | ✅ immutable Arcs |
| A2 | reader vs interning | funnel fold reads stage table while a new custom stage is interned | ❌ → **G-1** committer-owned interning, tables in snapshot |
| A3 | torn tail read | fold scans tail during append | ✅ watermark release/acquire + fixed-width rows (I-1 coupling, §2) |
| A4 | manifest/event read skew | funnel sees "revoked" manifest but no revocation event | ❌ → **G-2** unified `ReadState` |
| A5 | reader during segment seal | tail seals mid-fold | ✅ given G-2 (segment directory is in the snapshot) |
| A6 | long-held stale resolution | agent acts 12 min after resolve; token revoked at minute 3 | ❌ contract gap → **G-3** `as_of` + `revalidate_after` |
| B1 | multi-write, disjoint tokens | 50 subagents record on 50 tokens | ✅ queue + committer |
| B2 | multi-write, same token | two `record(run)` race | ✅ committer seq; counts commutative |
| B3 | lost update on lifecycle | two `superseded_by` writes; LWW drops one silently | ❌ → **G-4** CAS (`expected_version`) for lifecycle mutations |
| B4 | retry after lost ack | crash between fsync and ack; agent retries mint → duplicate token | ❌ → **G-5** idempotent mint via `mint_nonce` |
| B5 | backpressure starvation | analytics storm queues ahead of a mint | ❌ → **G-6** two-lane intake |
| C1 | read-your-mint cross-PoP | mint in Frankfurt; CI subagent resolves in Oregon 200 ms later; KV miss | ❌ → **G-7** authoritative read-through before `UnknownToken` |
| C2 | stale revocation at edge | Oregon serves Active seconds after revoke | ❌ → **G-8** `strict\|eventual` resolve levels, manifest-mandatable |
| C3 | cross-PoP mutation race | revoke (Frankfurt) vs mint_child (Oregon) | resolved by G-4 + two-path writes (08): lifecycle → origin CAS, never the queue |
| D1 | compaction vs readers/writers | Parquet rewrite during live traffic | ✅ two-phase commit, Arc-held segments — now stated for fs too |
| D2 | snapshot-while-writing | snapshot during burst | ✅ given G-2 (serialize one ReadState) |
| D3 | second process | accidental second server | ✅ advisory lock, explicit error |

## 4. Gap tracking table

*Status legend: design ✅ = specified in docs · tests ✅ = test spec below ·
impl/verified ☐ = flips during the mapped checkpoint (14).*

| Gap | Fix (one line) | Mechanism (rev 2.2) | Design | Tests spec'd | Impl | Verified | CP |
|---|---|---|---|---|---|---|---|
| **G-1** | committer-owned interning; tables immutable to readers | SQLite single-writer txn *(by construction)* + cache | ✅ | ✅ | ☐ | ☐ | CP-3/5 |
| **G-2** | one consistent snapshot per read | WAL snapshot txns *(by construction)*; `ReadState` survives as the cache design | ✅ | ✅ | ☐ | ☐ | CP-5 |
| **G-3** | `Resolution.as_of` + `revalidate_after` (variant-configurable) | core contract (02, 09) | ✅ | ✅ | ☐ | ☐ | CP-2 |
| **G-4** | CAS lifecycle mutations (`expected_version` → `Conflict`); LWW only cosmetic | `UPDATE … WHERE version = ?` *(by construction)* | ✅ | ✅ | ☐ | ☐ | CP-4/5 |
| **G-5** | idempotent mint via `mint_nonce` (retry returns original) | `UNIQUE(sharer, nonce)` *(by construction)* | ✅ | ✅ | ☐ | ☐ | CP-4/5/6 |
| **G-6** | two-lane committer intake, durable-first + anti-starvation | ours — the committer task (13 §8) | ✅ | ✅ | ☐ | ☐ | CP-5 |
| **G-7** | KV miss never authoritative — origin read-through | ours — edge worker (08) | ✅ | ✅ | ☐ | ☐ | CP-10 |
| **G-8** | `strict\|eventual` resolve consistency, manifest-mandatable | ours — edge worker (08) | ✅ | ✅ | ☐ | ☐ | CP-10 |

*"By construction" never waives a test: the §5 suites run against the SQLite
backend regardless — the contract outranks the mechanism.*

## 5. Test specification (normative — names are the implementation's names)

### 5.1 Unit & property tests

| Test | Kind | Asserts |
|---|---|---|
| `g1_intern_only_in_committer` | unit + trybuild | no public interning API outside committer; snapshot tables are `&` immutable |
| `g1_new_stage_visible_at_its_prefix` | unit | a record with a novel stage and its interned ID appear in the same `ReadState` |
| `g2_snapshot_no_skew` | property | ∀ interleavings: if a snapshot's manifest shows commit *n*'s effect, its watermark covers *n* (manifest state ↔ event prefix agree) |
| `g2_prefix_seq_monotonic` | property | successive snapshots have non-decreasing `prefix_seq`; readers never observe regression |
| `g3_resolution_carries_freshness` | unit | every `Resolution` has `as_of = now-at-resolve`; `revalidate_after` echoes the matched variant's config, defaulting per sensitivity |
| `g4_cas_conflict_on_stale_version` | unit | two `Superseded` mutations with the same `expected_version`: first `Ok`, second `Err(Conflict{token, seq})` |
| `g4_lww_allowed_for_labels` | unit | label mutations without `expected_version` succeed (documented cosmetic set only) |
| `g4_lifecycle_requires_version` | unit | `Revoked`/`Superseded`/`ExpirySet` without `expected_version` → rejected |
| `g5_mint_nonce_idempotent` | property | same `(sharer, nonce)` submitted 1..k times → exactly one token, k identical responses |
| `g5_distinct_nonce_distinct_token` | unit | different nonces never collapse |
| `g6_durable_lane_priority` | unit (committer sim) | with relaxed lane pre-loaded with 10⁴ events, a durable mint commits within its batch bound |
| `g6_relaxed_lane_no_starvation` | unit | under continuous durable load, relaxed lane drains ≥ its quantum |
| `a3_i1_fixed_width_guard` | compile-time/static | `Event` layout is fixed-width; a `String` field fails a static assert citing this doc |
| `c7_parent_revoked_rejected` | unit (existing, re-cited) | `mint_child` on revoked parent → `ParentRevoked` |
| fluency suite *(rev 2.4)* | unit + integration | `map_reachability` · `map_reverse_totality` · `envelope_next_valid` · `map_state_table` · `one_call_mint` · `hint_on_every_error` (defined in 17 §5) plus `it_local_auth` and `it_version_skew` (16 §5) |
| `blob_roundtrip` *(rev 2.3)* | unit | `mint --attach` → CAS write (tmp → atomic rename) → resolve returns `{url, sha256}` → fetched bytes hash-verify; corrupted blob detected |
| `cas_dedupe` *(rev 2.3)* | unit | attaching identical bytes twice → one CAS entry, two MediaRefs, same sha256 path |
| `inline_threshold_automatic` *(rev 2.3)* | property | bodies ≤ threshold inline; above → MediaRef; manifest total-size cap enforced at mint |
| `media_variant_by_modality` *(rev 2.3)* | unit | image/audio/transcript variants (06 §2) served per `ModalitySet`, deterministic |

### 5.2 Model checking (new layer in 13 §5's pyramid: `loom`)

| Test | Asserts |
|---|---|
| `loom_watermark_publication` | across all loom-explored interleavings of committer append + reader scan: reader never observes a row ≥ its acquired watermark, never a torn row |
| `loom_readstate_swap` | reader holding an old `ReadState` Arc is unaffected by swap; no interleaving yields a mixed-prefix view (G-2 at the memory-model level) |
| `loom_two_lane_intake` | no interleaving loses a submission or acks before its lane's commit point |

### 5.3 Integration tests (fs backend + stdio MCP unless noted)

| Test | Scenario | Asserts |
|---|---|---|
| `it_retry_storm` | G-5/G-4: kill server at 3 injected crash points (post-append/pre-fsync · post-fsync/pre-publish · post-publish/pre-ack); client retries every op with same nonces | after recovery + replay: exactly one token per nonce, no duplicate seq, CAS conflicts deterministic |
| `it_revoke_mid_swarm` | G-2/C-7/G-3: orchestrator + 8 concurrent subagents resolving/recording while parent is revoked | post-revoke resolves return `Revoked`; no child minted after revoke prefix; every funnel read internally consistent (revoked-manifest ⇒ revocation event present) |
| `it_analytics_flood` | G-6: 10⁵ relaxed records flood while mints proceed | mint ack p99 within budget (13 §6); zero relaxed-lane loss |
| `it_stale_hold_guidance` | G-3: resolve, wait past `revalidate_after`, act | projection guidance instructs re-resolve; re-resolve returns current disposition |
| `it_crash_recovery_matrix` | D1 + §5.3 crash points × (journal, seal, compaction) | reconstruct after each kill ≡ pre-crash acked state (R-2/R-3); unacked ops absent or idempotently re-appliable |
| `it_cross_pop_handoff` *(CP-10, Miniflare)* | G-7: mint at origin, resolve via a worker with cold KV | resolve succeeds via read-through; negative cache only after authoritative miss; second resolve served from KV |
| `it_strict_vs_eventual_revoke` *(CP-10, Miniflare)* | G-8: revoke at origin; resolve with stale KV both ways | `strict` → `Revoked` always; `eventual` may serve stale only within the declared bound; manifest-mandated strict overrides caller's `eventual` |
| `it_scenario_a` (existing, extended) | 06 §7 walkthrough | now also asserts `as_of`/`revalidate_after` present and funnel snapshot consistency |

### 5.4 Bench additions (13 §6 suite)

`readstate_swap_cost` (target: swap < 1 µs; reader clone < 100 ns) ·
`reader_p99_under_write_saturation` (existing, now explicitly G-2/G-6
gated) · `strict_resolve_overhead` *(CP-10: strict vs eventual delta
published, not hidden)*.

## 6. Standing review discipline

Any future change touching the committer, `ReadState`, or the edge write
path must add its scenario to §3 with a verdict before merge — this table is
append-only. The adversarial review that produced this document found eight
gaps in a design that had already passed two review rounds; the lesson is
institutionalized, not remembered.
