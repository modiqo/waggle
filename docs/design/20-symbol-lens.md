# 20 — The symbol lens: source-code handoffs, structured at mint

*Status: design — pre-implementation. This document extends 18 (content
access) and 19 (interrogation telemetry) to the artifact class they have
not yet taken seriously: source code. It states a testable hypothesis,
commits the technique and its performance discipline, and carries the
reference pseudo-code the implementation will be held to.*

---

## 0 · The hypothesis

> **H.** An agent handed a *code* token orients and converges faster when
> the token can describe its own structure — the symbols it contains,
> with line ranges — than when it can only offer lines and grep. And it
> does so at **zero serve-time cost**, because structure is computed
> once at mint, where the artifact is at hand, and travels with the
> snapshot as plain data.

Three measurable predictions, checked against the receipts machinery
doc 19 shipped (P0/P1):

- **H1 — orientation.** On contract-bearing code tokens, consumers
  reach the required regions in fewer `read`/`search` calls when the
  overview carries a symbol outline (funnel call-counts before/after,
  same tasks, dogfooded on this repository).
- **H2 — contracts become writable.** Authors declare code contracts at
  all. Today `--require` on code means hand-counting line numbers;
  `symbol:` sugar should move contract adoption on code tokens from ~0
  to parity with markdown's `section:`.
- **H3 — no serve regression.** p50 `read` overview on a code token
  with an outline stays within noise of today's overview (it is one
  extra content-addressed blob fetch, no parsing anywhere on the serve
  path — measured in the benches).

If H1/H2 fail in dogfooding, the lens stays (it is cheap); the *Tier 2*
investment (§8) does not happen.

## 1 · Why the current design falls short on code

The lens engine is text-first, not markdown-first — any UTF-8 text
already gets snapshots, line windows, regex search, budgets, and line
contracts. But code exposes four concrete gaps:

1. **Extension-less code is refused as binary.** Content type is
   inferred from the extension alone; `Makefile`, `Dockerfile`,
   `justfile`, hookless scripts → `application/octet-stream` →
   `is_text` false → `read`/`search` refuse a perfectly good text file.
2. **`mint --tree` walks blind.** It skips dotfiles and symlinks but
   not `node_modules/`, `target/`, `build/` — a real source folder
   either blows the 200-file cap or snapshots the build tree.
3. **No orientation.** A markdown token's overview carries an outline;
   a code token's overview carries `total_lines` and good luck. The
   consumer's first moves are blind windows or guessed greps.
4. **Contracts are unwritable on code.** `section:` resolves against
   ATX headings; code has none. Line-number contracts rot in the
   author's head even though they are stable against the snapshot.

Gaps 1–2 are correctness fixes (§6). Gaps 3–4 are the symbol lens.

## 2 · The technique

**Parse at mint. Store structure as data. Serve it everywhere.**

At mint, the native daemon parses the snapshot with **tree-sitter** —
the error-tolerant incremental parser with maintained grammars for
every language agents touch — and runs each grammar's **`tags.scm`**
query: a declarative pattern set that marks AST nodes as definitions
(`name.definition.function`, `name.definition.class`, …). The result is
not an AST and not an index; it is a flat, compact **symbol outline** —
`(name, kind, start_line, end_line, parent)` per definition — serialized
and stored **content-addressed beside the snapshot**.

Everything downstream is the machinery this repository already has:

- The `read` **overview** on a code token includes the outline (budget-
  fitted, like everything else) and advertises a `symbol` lens;
  `read --symbol parse_contract` is sugar for the resolved line window.
- **`--require symbol:NAME`** resolves at mint against the just-
  extracted outline into a plain line-range region — exactly how
  `section:` resolves against the markdown outline — landing in the
  signed contract with the symbol name as its label. Doc 19's bitmask,
  coverage fold, and named misses work unchanged.
- **The edge serves it as data.** Outline blobs replicate with
  snapshots (`edge push` already moves blobs). The Workers runtime
  never parses anything; the wasm question does not arise.
- **Immutability makes caching trivial.** The outline is a pure
  function of `(snapshot bytes, extractor version)`; both are pinned,
  so the outline is minted once and never invalidated. There is no
  staleness problem because there is nothing that can go stale.

Why tags queries and not full ASTs, references, or embeddings: agents
navigate by **grep and windowed reads** — the lens design already
matches that behavior. What they lack on arrival is the *table of
contents*. Definitions-with-ranges is the smallest structure that
provides one, it is language-uniform (one mechanism, ~30 grammars), and
it composes with the contract machinery without a single new invariant.
Relevance ranking is deliberately absent (§9): the consumer pulls what
it needs — and when ranking ever matters, the funnel's paying-regions
fold (19 §3.3) ranks by what *accepted* consumers actually touched,
which no static graph can know.

## 3 · Where the outline lives

A new optional immutable-core field, following the `contract` playbook
exactly (19 §4.2):

- `outline: Option<MediaRef>` — content-addressed, set at mint when the
  target parses; skipped when absent, so **contract-free/outline-free
  manifests keep their exact canonical bytes and every existing
  signature stays valid**. Signature vectors gain a case; none change.
- The blob's content type is `application/waggle-outline+json`. The
  wire shape (§5.2) is versioned by an `x` (extractor) field so a
  future extractor bump mints new outlines without ambiguity.
- The manifest stays small (the 256 KiB cap is untouched); a 2 kLOC
  file's outline is a few KiB of blob, not manifest.

## 4 · Performance discipline

Performance is a design input here, not an afterthought. The budget:

| Path | Target | How |
|---|---|---|
| serve (`read` overview, edge or local) | **zero parsing; one CAS get** — within noise of today | outline precomputed at mint; render is a budget-fit over an already-flat structure |
| mint, single file | extraction ≤ **5 ms / kLOC** on one core; total mint overhead < 15% over snapshot-only | one parse, one query pass, arena output; tree dropped before return |
| mint `--tree` (indexed; thousands of files) | wall-clock ≈ snapshot cost, not snapshot + N·parse | extraction parallel across a bounded blocking pool; appends stay sequential (the committer owns order) |
| memory | peak = one tree + one arena per in-flight file; **no retained trees, no global index** | parse → extract → drop; outline ≈ 40 B/symbol + one shared name buffer |
| startup | zero until first code mint | grammars linked, queries compiled lazily, once per process |

The concrete mechanics the pseudo-code below encodes:

- **Compiled-query caching.** `Query` compilation is the expensive
  setup; one `OnceLock<Query>` per language, compiled on first use,
  shared forever.
- **Parser reuse.** Parsers are cheap-ish but not free; one
  `thread_local` parser per blocking thread, `set_language` per call.
- **Arena, not tree.** The outline is a flat `Vec<Sym>` in document
  order with parent indices — nesting without pointers, binary search
  by line for range queries, one contiguous `String` for all names
  (offsets, not per-symbol allocations).
- **Async seam in one place.** Extraction is pure CPU; it runs under
  `spawn_blocking` behind a semaphore sized to the machine. The async
  daemon never blocks on a parse; the sans-I/O rule never bends
  (extraction takes bytes, returns a value — no clock, no I/O).
- **Sequential-scan-friendly serialization.** The blob is one JSON
  object of parallel arrays (struct-of-arrays), so budget rendering
  slices prefixes without deserializing symbols it will drop.

## 5 · Reference pseudo-code

Lives daemon-side in a new crate, `waggle-lens-code` — never a
`waggle-core` dependency, never compiled to wasm. `waggle-mcp` calls it
at mint behind a cargo feature (`code-lens`, default on for the CLI).

### 5.1 Language table and detection (also fixes gap 1)

```rust
struct LangSpec {
    id: LangId,                          // u8 newtype
    exts: &'static [&'static str],       // "rs" | "py" | "ts" | "tsx" | "go" | ...
    basenames: &'static [&'static str],  // "Makefile", "Dockerfile", "justfile", ...
    grammar: fn() -> Language,           // tree-sitter grammar entry point
    tags_scm: &'static str,              // the grammar's tags query source
}
static LANGS: &[LangSpec] = &[ /* rust, python, ts/js/tsx, go — v1 set */ ];

fn detect(path: &str) -> Option<LangId>   // extension first, then basename table

/// Text sniff (gap 1): the fallback when the extension says nothing.
/// First 8 KiB: no NUL byte and valid-UTF-8 prefix ⇒ treat as text/plain —
/// lines + search lenses apply even when no grammar does.
fn is_probably_text(head: &[u8]) -> bool
```

### 5.2 The outline arena and wire shape

```rust
/// Flat, document-ordered, parent-linked. ~40 B/symbol + shared names.
pub struct SymbolOutline {
    syms:  Vec<Sym>,      // sorted by start_line (document order)
    names: String,        // one buffer; symbols borrow by (offset, len)
    x:     u16,           // extractor version — pins the (bytes, x) → outline function
}
struct Sym {
    name:  (u32, u16),    // offset+len into names
    kind:  SymKind,       // u8: Fn | Method | Struct | Enum | Trait | Class | Iface | Mod | Const | Type | Macro
    lines: (u32, u32),    // 1-based inclusive — the SAME shape contract Regions use
    parent: u32,          // arena index; u32::MAX = top level
    depth: u8,
}

impl SymbolOutline {
    fn find(&self, name: &str) -> SmallVec<[u32; 2]>;       // all defs with this name
    fn at_line(&self, line: u32) -> Option<u32>;            // innermost enclosing def — binary search + parent walk
}

// Wire (application/waggle-outline+json), struct-of-arrays so a budget
// render can take prefixes without touching dropped entries:
// { "x": 1, "names": ["Contract","evaluate", ...],
//   "kinds": [3, 0, ...], "start": [121, 209, ...], "end": [230, 222, ...],
//   "parent": [-1, 0, ...] }
```

### 5.3 Extraction (pure CPU — the sans-I/O half)

```rust
static QUERIES: [OnceLock<Query>; N_LANGS];         // compiled once per process
thread_local! { static PARSER: RefCell<Parser>; }   // reused per blocking thread

/// Pure: (bytes, lang, x) → outline. No clock, no I/O, no globals mutated.
fn extract(text: &str, lang: LangId) -> SymbolOutline {
    let query = QUERIES[lang].get_or_init(|| Query::new(grammar(lang), tags_scm(lang)));
    PARSER.with(|p| {
        p.set_language(grammar(lang));
        let tree = p.parse(text)?;                   // error-tolerant: broken code still yields defs
        let mut out = SymbolOutline::with_capacity(guess);
        let mut stack: Vec<(ByteRange, u32)> = vec![]; // enclosing-def stack → parent links
        for m in QueryCursor::matches(&query, tree.root_node(), text) {
            let Some(cap) = m.capture("name.definition.*") else { continue };
            let def = m.definition_node();           // the enclosing definition node
            while stack.last().is_some_and(|(r, _)| !r.contains(def.range())) { stack.pop(); }
            let parent = stack.last().map_or(NONE, |(_, i)| *i);
            let idx = out.push(Sym {
                name:  out.intern(cap.text(text)),
                kind:  kind_of(cap.capture_name()),
                lines: (def.start_line() + 1, def.end_line() + 1),
                parent, depth: depth_of(parent, &out),
            });
            stack.push((def.byte_range(), idx));
        }
        out.sort_by_start_line();                    // document order, stable
        out                                          // `tree` drops HERE — never retained
    })
}
```

### 5.4 The async seam (mint path; parallel `--tree`)

```rust
/// Bounded CPU pool: extraction never blocks the async daemon and never
/// oversubscribes the machine minting a tree.
static EXTRACT_GATE: Semaphore = Semaphore::new(min(num_cpus(), 8));

async fn outline_at_mint(bytes: &[u8], path: &str, blobs: &impl BlobSink)
    -> Option<MediaRef>
{
    let lang = detect(path)?;                        // no grammar → no outline; text still minted
    let text = str::from_utf8(bytes).ok()?;
    let _cpu = EXTRACT_GATE.acquire().await;
    let outline = spawn_blocking(move || extract(text, lang)).await.ok()?;
    if outline.is_empty() { return None; }           // absent field, not an empty blob
    blobs.put(&outline.to_wire(), "application/waggle-outline+json").await.ok()
}

// mint --tree: extract in parallel, append in order — the committer
// stays the single owner of sequence (C-3); CPU work fans out, the log
// does not.
async fn mint_tree(files) {
    let outlines = join_all(files.map(|f| outline_at_mint(f)));   // parallel, gated
    for (file, outline) in files.zip(outlines.await) {
        append_mint(file, outline).await;                          // sequential, ordered
    }
}
```

### 5.5 Budget-fitted rendering (serve path — no parsing anywhere)

```rust
/// Overview rendering: include shallow structure first; bisect the depth/
/// count frontier until the render fits max_bytes. O(log n) renders, and
/// truncation is NAMED (`omitted` count) — never silent.
fn render_outline(o: &SymbolOutlineWire, max_bytes: usize) -> Value {
    let order = indices_sorted_by(depth_asc, then_line_asc);
    let fits = |n| render_prefix(o, &order[..n]).len() <= max_bytes;
    let n = partition_point(0..=order.len(), fits);      // binary search
    json!({ "symbols": render_prefix(o, &order[..n]),    // [{name, kind, lines, depth}]
            "total_symbols": o.len(), "omitted": o.len() - n })
}
```

### 5.6 Symbol contract sugar and the symbol lens

```rust
// mint: --require symbol:NAME  (beside lines:/section: — 19 §4.2 as built)
fn resolve_symbol_requirement(outline: &SymbolOutline, name: &str, i: usize)
    -> Result<Region, Envelope>
{
    match outline.find(name).as_slice() {
        []    => Err(err_with_nearest_names(name, outline)),   // misses teach, as always
        [one] => Region::new(Some(name), lines(one).0, lines(one).1, i),
        many  => Err(err_naming_candidates(many)),             // "qualify: Contract::evaluate"
    }
}

// read --symbol NAME → the resolved line window through the EXISTING
// read_lines path; the read event's region bitmask stamps itself via the
// machinery P1 shipped. lenses_for() gains "symbol" when an outline blob
// exists — discovered from state, never asserted.
```

### 5.7 Ignore-aware tree walk (gap 2)

```rust
// Replace the hand-rolled recursive walk with the gitignore-aware walker
// the ecosystem standardized on (the `ignore` crate): respects
// .gitignore/.ignore, skips hidden, never follows symlinks; our 200-file
// cap and determinism (sorted paths) preserved on top.
fn collect_files(dir: &Path) -> Vec<PathBuf> {
    WalkBuilder::new(dir).hidden(true).follow_links(false).build()
        .filter_map(files_only).take(CAP + 1).collect_then_sort()
}
```

## 6 · Surface and spec impact

| Piece | Change |
|---|---|
| manifest (spec §2) | optional `outline: MediaRef` in the immutable core; absence rule identical to `contract` — absent field, unchanged bytes, existing signatures valid |
| `mint` | outline extraction when a grammar matches (feature `code-lens`); `--require symbol:NAME` sugar; text-sniff fallback for extension-less files |
| `read` | overview gains `symbols` (budget-fitted) and advertises the `symbol` lens; `--symbol NAME` reads the resolved window |
| `coverage` / events | **unchanged** — symbol contracts are line-range regions; the P1 bitmask and folds apply as-is |
| edge | outline blobs replicate with `edge push`; serve path is a CAS get + render — no parser in wasm, no new E-matrix row beyond blob presence |
| vectors | signature vectors gain an outline-bearing case (existing cases MUST pass unmodified); an `outline.json` vector pins the wire shape for one fixed source file per language |
| benches | `extract_rs_1kloc` (target ≤ 5 ms), `render_outline_budget` (µs), overview p50 with/without outline (H3's gate) |
| `gc` *(follow-up)* | outline blobs join the live set (07's retention policy): referenced by a serving manifest ⇒ live; tombstoned token ⇒ sweepable with its snapshot |

## 7 · Invariant compliance

- **Sans-I/O**: `extract` is a pure function of bytes; effects (blob
  writes, semaphores, spawn_blocking) live at the daemon edge where
  effects already live. `waggle-core` gains one optional field and no
  dependency.
- **I-1/I-7**: untouched — the outline is authored *content* (a blob),
  not telemetry; events change not at all.
- **I-2**: untouched — outlines don't participate in variant matching.
- **Determinism**: outline = f(snapshot bytes, extractor version `x`),
  both pinned; re-mints of identical bytes under the same `x` produce
  byte-identical outline blobs (CAS dedupes them for free).
- **R-1..R-4**: no new record kinds; replay semantics unchanged.

## 8 · Tier 2 (explicitly deferred): structural search

A `search` mode that matches grammar patterns rather than regexes
("every call of `$F` inside `impl Contract`") is the natural
escalation, and the same tree-sitter foundation supports it. It is
deferred because it requires live parsing on the serve path — native-
only at first, with the edge answering honestly that structural search
runs at the owner's daemon. It enters only if H1/H2 hold and funnel
data shows regex search failing on code tokens (searches with no
follow-up reads — the churn signature of 19 §3.2). Nothing in Tier 1
forecloses it; the crate boundary is already drawn.

## 9 · Non-goals

- **No relevance ranking.** The consumer pulls; selection is its job
  (the name-semantics thesis). If a 200-file tree ever needs a ranked
  overview, rank by the funnel's paying regions — evidence, not graph
  centrality.
- **No reference graph, no embeddings, no persistent index.** The
  outline is per-snapshot data, not a queryable cross-file database.
  Waggle stays a reference substrate, not a code-intelligence engine.
- **No grammar sprawl.** v1 ships four language families (Rust, Python,
  TypeScript/JavaScript, Go) chosen by agent traffic; each addition
  must pay for its binary-size cost. Files without a grammar keep the
  full text loop — the lens degrades to exactly today's behavior,
  never below it.

## 10 · As built: the code handoff versus `rg`, by example

*(Added after implementation; first live evidence in
[notes/dogfood-01](notes/dogfood-01-symbol-handoff.md).)*

### Language coverage

| Outline + `symbol:` contracts | Extensions |
|---|---|
| Rust | `.rs` |
| Python | `.py` `.pyi` |
| TypeScript | `.ts` `.mts` `.cts` |
| TSX | `.tsx` |
| JavaScript | `.js` `.mjs` `.cjs` `.jsx` |
| Go | `.go` |

Everything else keeps the **full text loop** (line windows, regex
search, `lines:` contracts) — including extension-less carriers
(`Makefile`, `Dockerfile`, `justfile`, bare scripts), which the
basename table and the byte sniff admit as text. No grammar is never
an error; it is the absence of one lens.

### The same task, both ways

The task from the dogfood run: a subagent must read
`Contract::evaluate` in a 300-line Rust file it has never seen.

**The `rg` loop** (what agents do today):

```
rg -n "fn evaluate" contract.rs     # 2 hits: the def and a call site — pick one
sed -n '200,240p' contract.rs       # guessed window: starts mid-doc-comment
sed -n '207,230p' contract.rs       # second guess to capture the full extent
```

Three calls, two guesses, and nothing anywhere records that it
happened.

**Through the token:**

```
waggle read --token 9u6KEr6F --symbol evaluate
# → lines 213-226, the exact definition extent, pinned at mint
```

One call, no guessing — and the read stamped the contract region, so
`coverage` flipped to `met` and the orchestrator could verify the
review without trusting the reviewer's report.

### Where the token beats `rg` (positives)

- **Orientation before search.** The overview carries the symbol table
  of contents (28 symbols in the dogfood file) — `rg` can only answer
  questions you already knew to ask.
- **Exact extents, no window guessing.** `--symbol` serves the
  definition's pinned range; the grep→guess→re-read loop disappears.
- **Reach.** A remote consumer (edge, another machine) has no
  filesystem to grep; `search`/`read` run where the bytes live and the
  matches travel back. In the cross-machine handoff `rg` is not
  worse — it is absent.
- **Stability.** The outline is a pure function of the snapshot;
  `symbol:evaluate` means the same lines tomorrow. `rg` against a live
  tree drifts with every edit — including edits the orchestrator makes
  while the subagent reads.
- **Receipts.** Every read and search-hit stamps contract regions;
  interrogation becomes evidence. Disk `rg` is invisible by nature.

### Where `rg` still wins (negatives, honestly)

- **The unminted workspace.** The lens exists per token; grepping a
  whole checkout nobody minted is `rg`'s home turf, and §9 forbids the
  persistent index that would change this. (The open experiment: does
  a session-start `mint --tree` of the workspace move this boundary?)
- **Lexical parity, not superiority.** `search` through a token is the
  same regex class as `rg` — the wins are the structural layer around
  it, not the matcher.
- **No structural queries yet.** "every `.unwrap()` outside tests" is
  Tier 2 (§8), still gated on H1/H2 evidence.
- **Scope caps.** Trees cap at 200 files (post-deny-list) and four
  grammar families; `rg` has no such ceilings.
- **A mint must happen first.** `rg` is zero-setup; the token's
  advantages all cost one up-front `mint --snapshot` by whoever owns
  the handoff.

The summary the docs should keep repeating: **through the token beats
around it for any code artifact that crosses a delegation boundary;
`rg` keeps the unminted workspace.** The product's job is to make the
first category grow — by being the easier path, not only the audited
one.
