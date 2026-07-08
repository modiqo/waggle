# Embedding waggle in Rust

The CLI and MCP server are projections over library crates you can use
directly. The core rule: **`waggle-core` is sans-I/O** — no clock, no
randomness, no storage. You pass effects in; everything is deterministic
under test.

```toml
[dependencies]
waggle-core  = "0.1"                 # domain: mint, match, resolve, fold
waggle-store = "0.1"                 # the storage contract + MemoryStore
waggle-store-sqlite = "0.1"          # the production local backend
```

## Mint → resolve, no I/O anywhere

```rust
use waggle_core::{
    mint, resolve, CanonicalUrl, Channel, MintOptions, MintSpec,
    ResolverContext, Sharer, Timestamp,
};

// Effects are parameters: entropy is any FnMut(&mut [u8]) -> Result<…>,
// time is a value. In production pass getrandom + SystemTime; in tests,
// a counter and a constant — same code, fully deterministic.
let mut entropy = |buf: &mut [u8]| { getrandom::getrandom(buf).map_err(|e| waggle_core::EntropyError(e.to_string())) };
let now = Timestamp::from_unix_ms(1_783_500_000_000);

let manifest = mint(
    MintSpec::new(
        CanonicalUrl::new("ws://swarm/findings/report.md")?,
        Sharer::new("research-lead")?,
        Channel::subagent_general(),
    ),
    &MintOptions::default(),
    &mut entropy,
    now,
)?;

// The sealed matcher + freshness stamping — pure, ~7 ns.
let resolution = resolve(&manifest, &ResolverContext::anonymous_agent(), now);
assert!(resolution.variant.is_some()); // catch-all guarantees totality
```

## Persistence: one contract, three backends

```rust
use waggle_store::{AppendIntent, AppendStore, MintNonce, ReadStore};
use waggle_store_sqlite::SqliteStore;

let store = SqliteStore::open("waggle.db".as_ref())?;
store.append(AppendIntent::Mint {
    manifest: Box::new(manifest),
    nonce: MintNonce(42),            // idempotency: a retry returns the ORIGINAL
}).await?;

let view = store.manifest(token).await?.expect("read-your-mint");
```

The trait split is intentional: a function taking `&impl ReadStore`
**cannot write** — resolve paths are read-only by type, checked by a
`compile_fail` doctest. Backends implement `ReadStore + AppendStore` and
must pass the shipped conformance suite
(`waggle_store::conformance::run_all`) — seq monotonicity, CAS, idempotent
mint, revoked-parent rejection, views ≡ fold. If you write a backend, the
suite is your certification.

## The event log is the truth

```rust
use waggle_core::{reconstruct, LogRecord};

let records: Vec<LogRecord> = store.scan_all().await?;
let world = reconstruct(records);   // shuffle-immune, duplicate-immune
// world.manifests, world.funnels, world.lineage — rebuilt exactly.
```

Any statistic waggle shows you is a fold over this log; `reconstruct` is
the audit path that proves it.

## Where the layers sit

```
waggle-cli / waggled          processes: clock, entropy, disk enter HERE
  └─ waggle-mcp               tool surface: envelope, map, query, wire
       └─ waggle-store[-*]    contract + backends (conformance-certified)
            └─ waggle-core    pure domain — runs identically in wasm
```
