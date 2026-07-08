# Performance — measured, not promised

Criterion benchmarks (`just bench`), release mode. Latest run:
**2026-07-07, Apple Silicon (darwin 25.5), Rust 1.8x.** Numbers are the
criterion midpoint estimate. Budgets from
the engineering standards (design workspace).

| Path | Measured | Budget | Headroom |
|---|---:|---:|---:|
| `select_variant` (5 variants, sealed matcher) | 6.9 ns | — | — |
| `resolve()` (pure: disposition + match + freshness) | 7.4 ns | — | — |
| `token_parse` | 36 ns | — | — |
| **`manifest` read, cache hit** (SQLite store) | **39 ns** | < 1 µs | 25× |
| `token_generate` (8 chars, rejection-sampled) | 85 ns | — | — |
| `query` slice, deep path (120 KB doc) | 624 ns | — | — |
| `mint` (2 variants + catch-all synthesis + size check) | 876 ns | — | — |
| `query` slice, root shape of a 120 KB doc, 4 KB budget | 30 µs | — | — |
| **`event` append, durable** (WAL + `synchronous=FULL` — real fsync) | **39 µs** | — | ~25k events/s/writer |
| **funnel fold over 1,000,000 events** (SoA scan) | **334 µs** | < 10 ms | 30× |
| **socket round-trip** (shim → waggled → resolve → back, p50) | **323 µs** (p99 806 µs) | p50 < 2 ms | 6× |
| **edge resolve** (HTTP → worker → Durable Object → engine, p50, local-Miniflare) | **2.08 ms** | < 10 ms | 5× |
| event flood, 10k burst (in-memory store, per-event acks) | 280 ms (~36k/s) | — | — |

Reading the table:

- **The resolve hot path is nanoseconds.** A cache-hit manifest read plus
  the sealed matcher plus freshness stamping is well under 100 ns — the
  daemon's per-call cost is dominated by JSON and the socket, not waggle.
- **The durable append is honest.** 39 µs includes the fsync
  (`synchronous=FULL`): an acked write survives power loss. That is
  ~25k durable events/second on one writer — far beyond agent chatter —
  and the two-lane committer (G-6) batches beyond it when needed.
- **Analytics never need a database query.** A million-event funnel folds
  in a third of a millisecond from the in-memory SoA columns.

Not yet measured here: the **handoff benchmark** (context-forwarding vs.
token-referenced orchestration across ≥2 model families — the CP-9 public
number, needs live model APIs) and the loom/crash-point suites. Tracked in
the execution plan (design workspace).
