# 07 — The Pluggable Storage Interface

*One contract, many backends. The filesystem (JSONL → Parquet) proves it on a
laptop; Cloudflare (08) proves it at the edge; the conformance suite proves
they agree.*

> **Revision 2 note.** Contract unchanged. Two additions: the `fs` backend is
> now also **the store behind `waggle serve --stdio`** — the local MCP server
> every harness talks to, which makes it a first-class production path rather
> than a test double; and the conformance suite gains the
> **revoke-vs-mint-child race** test (C-7 below) closing the lineage-cascade
> gap flagged in the judge review.
>
> **Revision 2.1 note.** The adversarial concurrency review (15) added three
> contract clauses — **C-8 idempotent mint, C-9 CAS lifecycle mutations,
> C-10 authoritative miss** — and the conformance suite grows their tests
> (15 §5). The fs backend's compaction now explicitly states the same
> two-phase commit as the Cloudflare cron (15 §3 D1).
>
> **Revision 2.2 note (the credible-store pivot).** The **primary local
> backend is now SQLite** (`waggle-store-sqlite`: rusqlite, bundled, WAL
> mode) — replacing the hand-rolled JSONL journal + custom MRMW machinery as
> the correctness anchor. Rationale: WAL gives native multi-reader/
> single-writer with consistent snapshot reads; C-1/C-3/C-8/C-9 become
> ordinary SQL (`COMMIT`, a seq statement, a UNIQUE index, `UPDATE … WHERE
> version = ?`); and the adjacent production codebase (rote) runs SQLite for
> exactly these jobs. **JSONL is demoted to the wire format** — the
> export/replay/migration encoding of `LogRecord` (the `waggle replay --to`
> path, 16) — and an optional minimalist backend. The Parquet analytical
> archive is unchanged (compaction reads from SQLite).

## 1. Design stance

Core never touches storage (03 §1). `waggle-store` defines the contract; each
backend is its own crate (`waggle-store-fs`, `waggle-store-cloudflare`, …) so
dependency trees stay honest — a laptop user never compiles `worker`; a
Workers deployment never compiles `parquet`.

The contract is intentionally **narrow and log-shaped**: backends implement
an append-only record log plus two materialized read views. Everything else
(funnels, reports, reconstruct) is core folding over what the backend returns.
A backend author implements ~7 methods, not an ORM.

## 2. The traits

Two flavors, one meaning. Async is the primary (edge reality); sync is
derived for native simplicity:

```rust
/// Async, `?Send` — Workers futures are !Send; native wrappers add Send.
pub trait Store {
    // ── the log (system of record) ─────────────────────────────────────
    async fn append(&self, rec: LogRecord) -> Result<Appended, StoreError>;
    //     Appended { seq: u32 }  — store assigns per-token seq (C-3)
    async fn scan_token(&self, t: Token, from_seq: u32)
        -> Result<RecordStream, StoreError>;
    async fn scan_all(&self, watermark: Option<Watermark>)
        -> Result<RecordStream, StoreError>;          // reconstruct path

    // ── materialized views (derived, rebuildable) ──────────────────────
    async fn manifest(&self, t: Token) -> Result<Option<ManifestView>, StoreError>;
    //     ManifestView { manifest: Arc<AttributionManifest>, version: u32 }
    async fn tokens_for_target(&self, url: &CanonicalUrl)
        -> Result<Vec<Token>, StoreError>;
    async fn children(&self, t: Token) -> Result<Vec<Token>, StoreError>; // lineage

    // ── analytics acceleration (optional; None = "fold it yourself") ───
    async fn funnel_hint(&self, t: Token) -> Result<Option<FunnelReport>, StoreError>;
}
```

`funnel_hint` is the honesty valve: backends with native counters (Analytics
Engine, materialized SQL) may answer fast; the conformance suite diffs every
hint against the fold (R-4), so a hint can be *stale-bounded* but never wrong
beyond its declared staleness.

## 3. The backend contract (normative clauses)

- **C-1 Durability at ack**: `append` returning `Ok` means the record survives
  process death (fsync'd, queued-with-persistence, or replicated per backend's
  documented model — each backend states its model).
- **C-2 Append-only**: no record is ever modified or deleted (redaction, when
  it comes, is a *new* record kind).
- **C-3 Per-token monotonic seq**: assigned at append, gapless not required,
  monotonic required.
- **C-4 Idempotency**: duplicate `(token, seq)` — or `(token, nonce)` within
  the backend's declared window — must dedupe (04 §3).
- **C-5 View convergence**: materialized views must equal the fold of the log
  they've ingested; lag allowed, divergence not.
- **C-6 Read-your-mint**: a `manifest()` following an acked `Minted` append on
  the same client must observe it (the one causal guarantee hosts genuinely
  need; cross-client visibility may lag).
- **C-7 Revocation/lineage race** (new, rev 2): after a `Revoked` mutation is
  acked for token *P*, an append of `Minted{parent: P}` MUST be rejected; a
  cascade walk MUST observe children whose `Minted` was acked before the
  revocation. The conformance suite exercises both orderings concurrently.
- **C-8 Idempotent mint** (rev 2.1, G-5): `Minted` records carry a client
  `mint_nonce`; a duplicate `(sharer, nonce)` within the backend's declared
  window MUST return the original token's manifest, not create a new token.
  Retrying agents are the norm, not the exception.
- **C-9 CAS lifecycle mutations** (rev 2.1, G-4): `Revoked` / `Superseded` /
  `ExpirySet` mutations carry `expected_version`; on mismatch the append MUST
  fail with `Conflict{token, seq}` and append nothing. Cosmetic mutations
  (campaign, labels) remain LWW by commit order. The documented split is
  normative.
- **C-10 Authoritative miss** (rev 2.1, G-7): a cache/replica miss is never
  authoritative — `manifest()` MUST consult the backend's system of record
  before reporting `UnknownToken`; negative caching is permitted only after
  an authoritative miss.

## 4. Primary local backend: SQLite (WAL) → Parquet archive

The laptop-grade backend — zero external infrastructure (rusqlite with the
bundled feature compiles SQLite in), decades of durability hardening:

```text
<root>/
├── waggle.db                SQLite, WAL mode — the system of record
│   ├── records(token_id, seq, kind, stage_id, actor, variant, at_ms,
│   │           payload_json NULL for events)   -- the append-only log
│   │           UNIQUE(token_id, seq); UNIQUE(sharer_id, mint_nonce)  -- C-8
│   ├── manifests(token_id, version, doc)       -- materialized view (R-4)
│   ├── indexes: target→tokens · parent→children · intern tables
│   └── all writes in one txn: seq assignment (C-3) + CAS check (C-9,
│       `UPDATE … WHERE version = ?`) + append + view update — C-1 = COMMIT
├── archive/
│   └── events-YYYYMM.parquet   compacted history (03 §4 SoA schema, 1:1)
├── blobs/                      content-addressed store (rev 2.3): media
│   └── <sha256[0..2]>/<sha256>   bytes for MediaRefs — images, voice, any
│                               binary. Write = hash → tmp → rename (atomic);
│                               dedupe is free (same hash = same path);
│                               GC = mark-and-sweep against live MediaRefs
└── export/                     JSONL LogRecord stream on demand —
                                the wire/migration format (16); blobs sync
                                by hash alongside (rsync-like, resumable)
```

Blob rules: bytes live in the CAS, never in SQLite rows above the ~64 KB
inline threshold (02 MediaRef); the manifest's `sha256` is the integrity
contract — a resolver verifies what it fetched; tier 3's CAS is R2 (08),
same layout, presigned delivery.

**Retention (the live-set policy).** The store compounds by design —
the log appends forever (C-2) and blobs are immutable — but growth
tracks *churn*, not usage: identical bytes dedupe by construction, so
re-minting an unchanged tree adds nothing. The weight is blobs, and
their GC policy falls out of resolution semantics: a blob is **live**
iff some manifest that still *serves* references it (`content`, attach
media, derived artifacts such as outlines). Revoked and expired tokens
serve nothing by spec, so their blobs are semantically dead the moment
they tombstone — sweepable without breaking any promise. What survives
forever is deliberately cheap: the tombstone, and the payload-free
history. Superseded tokens still serve (content + pointer) and stay
live until revoked or expired. `BlobStore::gc` (mark-and-sweep against
a live set) is the shipped mechanism; the policy layer — computing the
live set from manifests and exposing a `gc` verb with a dry-run — is a
planned follow-up, alongside disk stats in `daemon status`. Log
compaction, should anyone ever want it, is already a spec guarantee:
R-2 (snapshot + suffix ≡ full) is the primitive; archive the prefix as
JSONL and keep the state snapshot.

- **Concurrency = WAL semantics**: many readers, one writer; readers get
  consistent snapshot transactions natively (this *is* the G-2 guarantee,
  provided rather than built); the daemon's committer task is the single
  writer (two-lane intake retained for QoS, G-6).
- **Hot-path cache**: an in-memory arc-swap manifest cache serves resolve
  under the µs budget — a cache over the anchor, invalidated in-process on
  write, never the correctness mechanism (13 §8).
- **Compaction** (explicit or size-triggered): rows older than the active
  window copy into monthly Parquet ([`parquet`]/[`arrow`]), two-phase
  (write → fsync → manifest pointer → DELETE), dictionary encoding from the
  intern tables.
- Single owning process (the daemon, 16) by advisory lock; **multi-process
  readers are safe** (a WAL bonus the journal design never had) but
  cross-process *writes* still go through the daemon over MCP/HTTP.
- The optional `fs-jsonl` backend (append-only JSONL + in-memory views)
  survives as the minimalist/reference implementation and conformance
  test-bed — real enough to trust, simple enough to read in an afternoon.

## 5. The conformance suite (the contract's teeth)

`waggle-store-conformance` ships as a library of generic test fns any backend
crate runs in its CI:

```rust
conformance::run_all::<MyStore>(harness);
// exercises: C-1..C-6, R-1..R-4 (04 §6), crash-recovery-by-replay,
// duplicate storms, per-token seq races, view-rebuild-equals-fold,
// 1M-event fold timing floor (documents, not gates)
```

A backend without a green conformance run is not a waggle backend — this is
how "pluggable" stays a promise instead of a hope.

## 6. Backend matrix (planned)

| Backend | Log | Views | Analytics accel | Status |
|---|---|---|---|---|
| `memory` | Vec | HashMaps | fold | v0.1 (tests, examples) |
| **`sqlite`** | records table (WAL) | tables + arc-swap cache | SQL + Parquet scans | **v0.1 — primary local (rev 2.2)** |
| `fs-jsonl` | JSONL journal | in-memory | fold | v0.1 optional/minimalist |
| `cloudflare` | Queues→R2 NDJSON | KV(+D1 later) | Analytics Engine hints | v0.2 foundation (08) |
| `postgres` | table | SQL views | matviews | community/v2 **[open]** |

JSONL's permanent role is the **wire format**: `waggle export` / `waggle
replay --to <url>` stream LogRecords as JSONL between any two backends —
idempotent by C-4/C-8, deterministic by R-1 (16 §3).

[`parquet`]: https://crates.io/crates/parquet
[`arrow`]: https://crates.io/crates/arrow
