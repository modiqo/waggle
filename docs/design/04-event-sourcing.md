# 04 — Event Sourcing and the Reconstruct Guarantee

*"Counters are cache; the log is truth." This document makes that sentence
precise enough to test.*

> **Revision 2 note.** Unchanged by the MCP pivot — strengthened by it: with
> consumption flowing through one server implementation rather than N embedded
> libraries, the log has a single writer discipline per deployment and the
> reconstruct guarantee is enforced at one point. The `Event.variant` field
> added in 02 rides the existing record layout (one optional byte; SoA gains a
> sixth column, Parquet schema likewise).

## 1. The log is the system of record

Two record kinds share one append-only log:

```text
LogRecord
├── Event            { token, stage, actor, at, seq }          (02 §2, I-1)
└── ManifestMutation { token, at, seq, change: Change }
      Change ::= Minted(manifest)          — the birth record
               | Revoked
               | Superseded { by: Token }
               | CampaignSet(Option<…>)
               | LabelSet { key, value } | LabelUnset { key }
               | ExpirySet(Option<Timestamp>)
```

**Minting itself is a log record.** There is no manifest table that the log
merely annotates — the manifest *table* is a fold over `Minted` + subsequent
mutations. Backends may (and do) maintain a materialized manifest view for
read speed, but the view is derived state, rebuildable, and the conformance
suite (07 §5) verifies it.

Consequences worth saying out loud:

- **Time travel is a query.** "What did a Codex agent resolving `/x/9fk2`
  see last Tuesday?" = fold mutations to Tuesday's version, run the (pure,
  deterministic) variant selector against a Codex context. For an attribution
  system that agents *act on*, this auditability is the differentiator — no
  shortener, including Dub, offers it.
- **Deletion is a mutation.** Revocation appends; nothing is removed. GDPR-
  style erasure applies to *targets and metadata* (a `Redacted` change is the
  v2 answer **[open]**), never to counts — which contain no personal data by
  I-1.

## 2. Ordering: per-token sequences, no global clock

Distributed hosts (edge PoPs) cannot maintain a global order cheaply, and the
domain doesn't need one:

- Every record carries `seq: u32`, **monotonic per token**, assigned by the
  store at append (07 contract C-3).
- Cross-token order is by `at` timestamp, acknowledged as approximate.
- All folds are defined to be **order-insensitive across tokens** and
  **seq-ordered within a token** — funnel counts commute; manifest state is a
  last-writer-wins fold over (token, seq).

This is the weakest ordering that keeps every promised query exact, which is
why it's the contract — anything stronger would make the Cloudflare backend
(08) either wrong or slow.

## 3. Delivery semantics: at-least-once + idempotent append

Edge queues (and real life) duplicate. The contract embraces it:

- Every record's identity is `(token, seq)`; stores MUST deduplicate on it
  (C-4). Producers that can't pre-assign `seq` (fire-and-forget edge events)
  attach a random 64-bit `nonce`, and the store assigns `seq` at ingest while
  deduplicating on `(token, nonce)` within a bounded window.
- Folds are therefore exactly-once *in effect* over at-least-once transport.

## 4. Write path discipline: event first, state second

Mutation flows one direction:

```text
host intent ──► append ManifestMutation ──► ack ──► update materialized view
                                                     └─► invalidate caches
```

A mutation that updated the view but missed the log would break reconstruct;
the reverse (logged, view lags) merely delays visibility. Choose the safe
failure mode structurally: **the append is the commit point.**

## 5. Snapshots and compaction

Replay-from-genesis is the correctness definition, not the operational plan:

- **Snapshot** = serialized `WorldState` (manifest views + intern tables +
  funnel accumulators) at a log position `(watermark: per-token seq vector)`.
  Reconstruct = load snapshot + replay the suffix. Snapshots are *disposable*
  — deleting them costs recompute, never correctness.
- **Compaction** (filesystem backend, 07 §4; Cloudflare cron, 08 §5) rewrites
  the JSONL/NDJSON journal into **Parquet** with the exact SoA column schema
  from 03 §4 — `token_id · stage_id · actor · at_ms · seq` plus dictionary
  pages for the intern tables. The [`parquet`]/[`arrow`] crates are the
  production-proven choice (this exact journal→Parquet cron pattern runs
  today in rote's Cloudflare telemetry pipeline).
- Analytical reads (big funnels, per-channel matrices, sharer reports) scan
  Parquet with predicate pushdown on `token_id`/`stage_id`; hot reads never
  touch it.

## 6. The reconstruct algorithm (normative)

```text
reconstruct(log) -> WorldState:
    state ← empty (intern tables seeded with well-known stages)
    for rec in log grouped by token, seq-ascending within token:
        match rec:
            Minted(m)          → state.manifests[m.token] ← version 1 of m
            other mutation     → state.manifests[token].apply(change)  # LWW by seq
            Event(e)           → state.log.push_soa(e)                 # 03 §4 layout
    state.funnels ← fold_funnel over state.log partitions              # commutative
    return state
```

Properties the test suite asserts:

- **R-1 Determinism**: same multiset of records (any arrival order respecting
  per-token seq) → byte-identical serialized `WorldState`.
- **R-2 Snapshot equivalence**: snapshot(t) + replay(t..) ≡ replay(0..).
- **R-3 Duplicate immunity**: log ∪ duplicates ≡ log.
- **R-4 View agreement**: every backend's materialized answers ≡ reconstruct
  answers (the conformance suite runs both and diffs).

## 7. Why event sourcing here is cheap, not enterprise-y

Event sourcing has a reputation for ceremony. It is nearly free in waggle
because the domain conspires:

- Events are **tiny and fixed-width** (no payload — I-1 again doing double
  duty): 19 bytes/row in SoA, dictionary-friendly, append-only.
- Folds are **commutative counts** — no aggregates that need ordering beyond
  per-token seq, no sagas, no process managers.
- Mutations are **rare and small** (a token gets a handful in its life).

The reconstruct guarantee — normally the expensive promise — falls out of
three cheap decisions: no payloads, per-token seq, LWW manifest folds.

[`parquet`]: https://crates.io/crates/parquet
[`arrow`]: https://crates.io/crates/arrow
