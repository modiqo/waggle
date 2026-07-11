# The waggle specification — v0.3 draft

*Normative. Everything here is enforced by the reference implementation's
CI; the conformance vectors in [`vectors/`](vectors/) are the portable
half — an independent implementation that matches them, and passes the
conformance suite (`waggle-store::conformance`), is a waggle
implementation. Language is RFC-2119 (MUST/SHOULD/MAY).*

## 1 · The token

A token is 4–23 characters from the Bitcoin base58 alphabet
(`123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz` — no
`0OIl`). Defaults: **8 characters** public, **16 characters** private
(capability URLs — possession is the credential). Generation MUST use
rejection sampling (no modulo bias). Tokens are case-sensitive, ordered
lexicographically by string form.

## 2 · The attribution manifest

Three zones, and the zoning is load-bearing:

- **Immutable core** (set at mint, never changed): `schema`, `token`,
  `target`, `sharer`, `channel`, `minted_at`, `meta` (the mint-time
  snapshot — I-3), `parent`, `content` (the snapshot `MediaRef`),
  `variants`, `private`, `contract` (the consumption contract, §2.1).
  Signatures cover exactly this zone (§6), so mutations MUST NOT
  invalidate them. An absent `contract` MUST NOT appear in the
  serialized manifest or the canonical core bytes — contract-free
  manifests keep the exact bytes (and signatures) they had before the
  field existed.
- **Versioned mutable** (CAS by `version` — C-9): `expires_at`,
  `revoked_at`, `superseded_by`. Lifecycle changes MUST require
  `expected_version` and MUST fail with a conflict naming both versions
  on mismatch.
- **Cosmetic mutable** (LWW): `campaign`, `labels`.

Serialized manifests MUST NOT exceed **256 KiB**; bodies over **64 KiB**
SHOULD ride as content-addressed `MediaRef`s instead of inline.

### 2.1 · The consumption contract

An optional `contract` declares what a consumer must reach:
`{ regions: [{label?, start, end}…], min-permille }` — 1 to **8**
regions (the width of the event touch bitmask, §4), each a 1-based
inclusive line range (`start ≥ 1`, `start ≤ end`; labels ≤ 80 chars),
and a threshold in `1..=1000` (permille of regions; default 1000 =
every region). Implementations MUST reject contracts outside these
bounds at mint. The contract is satisfied when
`touched × 1000 / required ≥ min-permille`, where a region counts as
touched if any served read window or search hit overlapped it (§8).
Coverage reports MUST name the untouched regions.

## 3 · Variants and the sealed matcher

A manifest carries ≥1 variants; mint MUST guarantee exactly one
catch-all (synthesizing one if none is declared, rejecting duplicates).
Selection is **sealed** — implementations MUST NOT expose hooks that
alter it:

1. a variant matches iff every constrained dimension accepts the
   context (`model_family`/`harness`: case-insensitive membership, an
   UNDECLARED context value fails a constrained dimension;
   `modalities`: superset; `posture`: membership);
2. specificity = count of constrained dimensions (0–4);
3. highest specificity wins; ties break by declaration order;
4. selection over minted manifests is total.

Same context ⇒ same variant index, always. The
[`selection vectors`](vectors/selection.json) are normative.

## 4 · The event log

The log is the truth; every view is a fold over it, rebuildable.

- Records: `minted` (the full manifest — birth is a record),
  `mutation`, `event`. Wire format: one JSON record per line (JSONL),
  serde-tagged with `record`.
- Events are **payload-free** (I-1): exactly
  `{token, stage, actor, at, seq, variant?, regions?}`. Actor is coarse
  classes only (I-7): kind (bot/human/terminal/agent), model FAMILY,
  harness class — never versions or instance identifiers. `regions` is
  a bitmask indexing the manifest's declared contract regions (§2.1) —
  manifest-referencing exactly like `variant`, so I-1 holds: positions
  into a signed declaration, never content. It MUST be absent on
  contract-free traffic; absent parses as no-touch, so pre-contract
  logs replay unchanged.
- The judged outcome rides as stages, not payload: `accepted` /
  `rejected` are well-known stages recorded by the judge of a
  delegation. The derived outcome is a pure function of counts —
  neither ⇒ `pending`, one ⇒ that verdict, both ⇒ `contested`
  (surfaced, never silently overwritten).
- `seq` is per-token, store-assigned, dense from 0 (`minted`) — C-3.
  Record identity is `(token, seq, kind)`; replay MUST dedup on it
  (C-4).
- **Reconstruction guarantees**: R-1 order-insensitive (any shuffle of
  the stream yields byte-identical state), R-2 snapshot+suffix ≡ full,
  R-3 duplicate-immune, R-4 materialized views ≡ the fold.

## 5 · The storage contract

C-1 acked ⇒ durable (per the backend's documented model) · C-2
append-only · C-3 store-assigned dense per-token seq · C-4 replay dedup
on `(token, seq, kind)` · C-6 read-your-mint · C-7 no children under a
revoked parent (pre-revocation children remain visible) · C-8 mint
idempotency by `(sharer, nonce)` — a replay MUST return the ORIGINAL
manifest · C-9 lifecycle CAS · C-10 a cache miss MUST consult the
system of record before answering "unknown".

The conformance suite is the certification; passing it is what "waggle
backend" means.

## 6 · Trust

Signatures are Ed25519 over the **canonical core bytes**: the JSON
serialization of the immutable-core fields in specification order (§2),
maps as sorted (BTree) maps. The signature block
`{alg: "ed25519", key: hex32, sig: hex64}` lives outside the signed
bytes. Verification is three-valued: `unsigned` is not `invalid`;
consumers choose policy per trust context. The
[`signature vectors`](vectors/signature.json) pin the canonical
encoding.

Private tokens (§1) MUST be refused by public rendering surfaces
(unfurls, social artifacts).

## 7 · Resolution semantics

A resolution is **knowledge, not a lease**: it MUST carry `as_of` and
`revalidate_after` (the variant's declared window, else 15 minutes).
`revoked` serves nothing; `superseded` serves content plus the pointer;
`expired` serving policy belongs to hosts. Caches MUST honor each
resolution's own stamp (never an invented TTL) and MUST be invalidated
by lifecycle mutations arriving on ANY path — interactive or
replication. Federated resolution offers `strict` (always revalidate at
the owner) and `eventual` (cache inside the window) — the trade is the
author's declared freshness, never unbounded staleness.

## 8 · Content access

`read`/`search` operate on the mint-pinned content (`content`
`MediaRef`, hash-verified) or, for local callers only, the live target.
Remote callers MUST NOT trigger live filesystem reads. Every response
MUST fit the request's byte budget (default 4096, floor 64); truncation
MUST be explicit (`total_matches` counted in full). Reads record the
`read` stage — counts only, never patterns or matched text. On a
contract-bearing token, the serve MUST stamp the event's `regions`
bitmask with the contract regions the served window (or the search
hits' lines) overlapped — the served *positions* are the evidence;
patterns and text remain excluded absolutely.

## 9 · Invariants, one table

| | statement |
|---|---|
| I-1 | events carry no payload — by type, not policy |
| I-2 | same context ⇒ same projection (the sealed matcher) |
| I-3 | unfurls render the mint-time snapshot, never a scrape |
| I-4 | resolution cannot write — read paths are read-only by type |
| I-7 | actors are coarse classes; identity never enters the log |
