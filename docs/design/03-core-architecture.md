# 03 — Core Architecture: Sans-I/O, Function Passing, and the Data Structures

*How `waggle-core` is built. This is the document that must survive a senior
Rust engineer's review.*

> **Revision 2 note.** Two rationale shifts, no structural change. (1) The
> sans-I/O justification is now: deterministic testability + the Cloudflare
> Workers **build target** — the earlier "language bindings from one core"
> argument is retired, since consumption is protocol-shaped (MCP/HTTPS, 09 §3)
> and no bindings will be built. (2) The core function surface below maps
> **1:1 onto the MCP tool schema** (`mint`/`resolve`/`record`); the
> function-passing design was already tool-shaped, and 09 now pins that
> correspondence as a compatibility test.

## 1. The sans-I/O law

`waggle-core` performs **no I/O, owns no clock, and generates no entropy**.
Every effect is a parameter. This is not purism — it is the only design that
runs unchanged in:

- native binaries (the CLI and the stdio-MCP server),
- Cloudflare Workers wasm (single-threaded, no `std::time::SystemTime`
  wall-clock semantics, `!Send` futures, JS-supplied randomness) — wasm is a
  **deployment build target**, not a distribution mechanism,
- deterministic tests (fixed clock, seeded entropy, in-memory store),
- property-based and fuzz harnesses.

Concretely, the core's three effect inputs:

```rust
/// Fills `buf` with cryptographically secure random bytes.
/// Implemented by closures — function passing, not global state.
pub trait Entropy { fn fill(&mut self, buf: &mut [u8]) -> Result<(), EntropyError>; }
impl<F> Entropy for F where F: FnMut(&mut [u8]) -> Result<(), EntropyError> { /* … */ }

/// Milliseconds since the Unix epoch. A value, not a source: callers pass
/// `now` explicitly to every time-dependent function.
pub struct Timestamp(u64);
```

Native hosts pass `|b| getrandom::getrandom(b)` ([`getrandom`] is the
ecosystem-standard OS entropy crate) and `Timestamp::from_unix_ms(...)`;
Workers pass the JS crypto/`Date.now()` equivalents. Tests pass a counter and
a constant. **No `Clock` trait** — a trait implies an ambient source; a
parameter makes time explicit in every signature that needs it, which is also
what makes replay (04) trivially correct.

## 2. The core is functions; the host owns orchestration

There is no `Minter` god-object holding a store. The core exports **pure
functions over explicit state**, and thin host-side glue composes them:

```rust
// ── the entire core surface, conceptually ──────────────────────────────
pub fn mint(
    spec: MintSpec,                      // target, meta, sharer, channel, variants…
    opts: &MintOptions,                  // token length, ttl defaults
    entropy: &mut impl Entropy,
    now: Timestamp,
) -> Result<AttributionManifest, MintError>;

pub fn negotiate(hint: ConsumerHint<'_>) -> ResolverContext;
//   ConsumerHint::UserAgent(&str) | ConsumerHint::AgentCard(&AgentCard)
//                                 | ConsumerHint::Explicit(ResolverContext)

pub fn resolve(
    manifest: &AttributionManifest,
    ctx: &ResolverContext,
    now: Timestamp,
) -> Resolution<'_>;                     // borrows the variant — zero-copy

pub fn event(token: Token, stage: StageId, actor: ActorClass, now: Timestamp)
    -> Event;                            // construction only; appending is the store's job

pub fn fold_funnel(events: impl Iterator<Item = EventView>) -> FunnelReport;
pub fn reconstruct(log: impl Iterator<Item = LogRecord>) -> WorldState;   // 04
```

Notice what this buys:

- **`resolve` cannot write** (invariant I-4 is a *signature*, not a policy).
- Every function is unit-testable with zero infrastructure.
- The Workers host and the filesystem host differ only in glue.

## 3. Trait surface: minimal, sealed where it must not grow

Only three traits exist in core, and two are effectively function types:

| Trait | Role | Notes |
|---|---|---|
| `Entropy` | randomness injection | blanket-impl'd for closures |
| `VariantMatcher` | *sealed* — the deterministic selection algorithm | sealed so downstream crates cannot fork determinism (I-2); extension happens in data (MatchExpr), not code |
| `Store` (in `waggle-store`, not core) | persistence contract | 07; core never sees it |

Sealing `VariantMatcher` is a deliberate, opinionated call: if selection
semantics were pluggable, "same context → same projection" would be true only
per-deployment, and the auditability claim dies. Adaptivity extends by adding
match dimensions to the *data model* (a semver-visible change), never by
swapping algorithms.

## 4. Data structures — measured against the access patterns

The access patterns, in order of frequency:

1. **resolve**: token → manifest (read-hot, spike-prone) → variant select.
2. **record**: append event (write-hot, tiny, latency-insensitive).
3. **fold**: scan events for one token / one target (analytical).
4. **reconstruct**: full-log replay (rare, batch).

### Token: inline, `Copy`, 24 bytes

```rust
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Token { len: u8, buf: [u8; 23] }   // base58 bytes, max 23 chars
```

No heap, no pointer chase, fits two per cache line in maps. Display/parse
validate the alphabet; `Token` is a value the way a `u64` is a value.

### Interning: stages and tokens become integers

Analytics never compares strings. Two append-only intern tables (built at
load, extended at runtime) map external names to dense IDs:

```rust
pub struct StageId(u16);   // well-known stages pre-interned as consts:
                           // StageId::CLICK, StageId::RUN, …
pub struct TokenId(u32);   // per-store dense id; Token ↔ TokenId bijection
```

Interning is the single decision that makes everything downstream fast: the
event log becomes fixed-width integers, folds become array arithmetic, and
Parquet dictionary encoding falls out for free.

### The event log: struct-of-arrays, columnar from birth

The in-memory analytical representation is SoA, not `Vec<Event>`:

```rust
pub struct EventLog {
    tokens: Vec<TokenId>,   // u32
    stages: Vec<StageId>,   // u16
    actors: Vec<ActorCode>, // u8  (packed class + family + harness)
    at_ms:  Vec<u64>,
    seqs:   Vec<u32>,       // per-token monotonic (04 §2)
    token_index: HashMap<TokenId, SmallVec<[u32; 8]>>, // row ids per token
}
```

Why SoA: a funnel fold touches `stages` (2 bytes/row) and optionally `actors`
(1 byte/row) — a million events is a ~3 MB sequential scan, which is
effectively free. And the five columns map **1:1 onto the Parquet schema**
(07): the in-memory layout *is* the archive layout, so compaction is a copy,
not a transformation. ([`smallvec`] inlines the common few-events-per-token
case; [`hashbrown`]'s default-hasher map via std is fine — token IDs are
already random, no need for exotic hashers.)

### Funnel fold: one pass, no allocation until the report

```rust
pub struct FunnelAccum { counts: [u64; WELL_KNOWN_STAGES], custom: BTreeMap<StageId, u64> }
```

Well-known stages count into a fixed array (branch-free index); the rare
custom stage falls into the tree. The report serializes from `BTreeMap` so
output ordering is deterministic (I-2's spirit applied to reports).

### Manifests: immutable `Arc`, versioned by replacement

```rust
pub struct ManifestCell(ArcSwap-like semantics);  // [open] — see below
```

A manifest is read on every resolve and written on rare mutation. v1 keeps it
simple and correct: `Arc<AttributionManifest>` replaced whole on mutation
(mutations are events first, state second — 04 §4). **[open]** whether to
take [`arc-swap`] as a dependency for lock-free reads or gate it behind a
`perf` feature; the trait contract (07) hides the choice.

### Strings: compact where they live in bulk

`CompactString` ([`compact_str`] — 24-byte inline small-string, a real,
widely used crate) for manifest text fields; plain `String` where cardinality
is low. No premature zero-copy deserialization: manifests are small and rare;
events — the hot volume — contain no strings at all *by invariant I-1*. The
performance story is won by the domain model, not by unsafe tricks.

## 5. Concurrency model: none (in core)

Core types are `Send + Sync` where derivable but core has **no threads, no
locks, no async**. Wasm's single thread and native's tokio both orchestrate
outside. The `Store` trait (07) offers sync and `?Send`-async flavors;
`EventLog` folds take `&self`. The absence of a concurrency model in core is
the feature — concurrency is a host decision, like time and randomness.

## 6. Error taxonomy

Per-crate `thiserror` enums with specific variants (`MintError::Collision`,
`MintError::InvalidChannel`, `ResolveError::UnknownToken`, …), a `Result<T>`
alias per crate, `#[from]` for layering, no `anyhow` in any library crate.
Errors carry what the caller can *act on*, never internal state dumps.

## 7. What is deliberately absent from core

HTTP, HTML, JSON-schema validation of Agent Cards (that's `waggle-agent`),
QR rendering (`waggle-social`, `qr` feature), storage, rate limiting, auth,
metrics emission. Core compiles with `serde`, `thiserror`, and — optionally —
`compact_str`/`smallvec`. Target: **compilable to wasm32 with zero cfg gymnastics.**

[`getrandom`]: https://crates.io/crates/getrandom
[`smallvec`]: https://crates.io/crates/smallvec
[`hashbrown`]: https://crates.io/crates/hashbrown
[`arc-swap`]: https://crates.io/crates/arc-swap
[`compact_str`]: https://crates.io/crates/compact_str
