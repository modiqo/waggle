# 11 — The Standard Track

*New in revision 2. How waggle stays independent of any one protocol while
positioning to become a standard itself — working with Claude-class
harnesses, Codex-class harnesses, and A2A simultaneously, depending on none.*

## 1. The stance: protocol-independent, everywhere-compatible

Waggle's own `ResolverContext` and manifest schema are the lingua franca;
every external schema reaches them through an adapter (06 §1). Nobody's
protocol is upstream of us:

| Rail | Role | Dependency direction |
|---|---|---|
| **MCP** | the distribution rail — where daily usage lives (~97M monthly downloads); `mint`/`resolve`/`record` as tools reach every MCP-speaking harness with zero waggle code | we implement their (open, stable) tool interface |
| **Plain HTTPS** | universal fallback + unfurl surface + `/.well-known/waggle` first contact | none |
| **A2A** | compatibility option — waggle URIs in Artifact URL Parts; Agent Card through an extractor; the `x-waggle/*` extension proposed to their community | one adapter module |
| **Harness metadata** (Claude Code, Codex, …) | first-class context source for the intra-harness wedge | extractors per harness |

If A2A's usage catches its endorsements, we're already the resolution layer
it never defined. If it stalls, the wedge (intra-harness handoffs over MCP)
never noticed. That asymmetry is the strategy.

## 2. Standardize the minimum, and make it boring

Lessons from what worked (MCP, llms.txt) vs. what's stalling (15 registries,
10 IETF drafts, zero interop): **adoption cost decides.** The waggle spec is
only:

1. the **URI shape** (`/x/{token}`, token alphabet/length rules),
2. the **manifest JSON schema** (immutable core · variants · mutable
   sections; `schema: u16` versioned),
3. the **resolution request/response** — `ResolverContext` in, disposition +
   projection + variant index out — and the normative selection algorithm
   (match → specificity → declaration order → mandatory catch-all),
4. the **stage vocabulary** (well-known set + custom-slug rules) and the
   payload-free event shape,
5. **`/.well-known/waggle`** (endpoints + schema version for first contact),
6. the **MCP tool schema** for `mint`/`resolve`/`record` (+ optional
   `funnel`/`share`).

Not standardized: storage, event sourcing internals, the SoA layout, Parquet,
Cloudflare anything — that's our implementation's excellence, not the
standard's burden. A conforming implementation could be a Postgres-backed
Python service; the spec must not care.

## 3. One implementation, every ecosystem — via protocol, not ports

Rev 1 imagined npm/pyo3 bindings; rev 2 deletes them. The MCP server *is*
the language story: Python, TypeScript, Go, anything calls the tools. The
determinism claim gets stronger, not weaker — one implementation performs
variant selection per deployment, so "same context → same projection" is
enforced at a point instead of promised across ports. For the rare in-process
embedder, the facade crate exists (09 §7); for a second *implementation*,
the spec + conformance vectors exist. We maintain no ports.

## 4. Conformance is the credibility mechanism

Two artifacts make the spec real without a committee:

- **Conformance vectors** (public, versioned): manifest parse/serialize
  cases, variant-selection tables (including ties and near-misses — where
  implementations rot), disposition matrices, reconstruct fixtures (records
  in → world-state out, byte-exact). "Passes waggle vectors" is checkable by
  anyone in any language.
- **The backend conformance suite** (07 §5) for storage implementations,
  C-1..C-7 + R-1..R-4.

## 5. Governance humility (sequenced)

1. **Now:** MIT/Apache-2.0 everything; spec draft lives in this repo; call it
   a *spec with a reference implementation* — never "a standard."
2. **At 0.3:** publish the spec draft + vectors; submit the `x-waggle`
   Agent-Card extension and the Artifact-URL resolution mapping to the A2A
   community as a proposal (positioning: the layer their v1.0 explicitly
   left implementation-specific — see 01 §2's verified gap).
3. **At 1.0:** schema freeze; spec versioned independently of crates.
4. **When earned** (two independent implementations + real deployments):
   neutral-home conversation (foundation donation), on the MCP/A2A precedent.
   Declaring standardhood before usage is how projects join the
   ten-drafts-zero-interop graveyard — the research pass documented that
   graveyard; we don't move in.

## 6. Sequencing (interleaved with 10 §1)

**(1)** 0.1 ships the MCP facade + benchmark numbers (usage before spec);
**(2)** spec draft + vectors at 0.3 (shaped by real deployments);
**(3)** A2A proposal lands with working code behind it, not slideware;
**(4)** governance only when someone independent has implemented it.

One honest caveat, standing: becoming a standard is a distribution game more
than a design game. This document removes every technical obstacle; only the
wedge's real usage (10 §2, item 1) can supply the rest. Specs follow
deployments, not the reverse.
