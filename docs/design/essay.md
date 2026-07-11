<h1 align="center">
  <img src="../assets/logo.svg" width="52" alt="the waggle mark: a figure-eight dance with the waggle run as an arrow" align="center"> waggle — the essay
</h1>

<p align="center">
  <em>What inspired the design.</em><br>
  A honeybee twenty million years ago, a termite mound, and forty years of
  distributed systems all arrived at the same answer: share names, not
  payloads.
</p>

<p align="center">
  <a href="#the-dance">The dance</a> ·
  <a href="#stigmergy">Stigmergy</a> ·
  <a href="#what-the-systems-world-already-knew">The systems lineage</a> ·
  <a href="#the-paradigm-stated-plainly">The paradigm</a> ·
  <a href="../../README.md">How to use it →</a> ·
  <a href="../../paper/">The paper →</a>
</p>

<p align="center">
  <img src="../assets/hero.svg" alt="The handoff, before and after: pasting the whole artifact to every subagent, versus handing off a 30-byte token that each consumer resolves into its own projection" width="940">
</p>

> This essay is the *why*. For installation, the harness wiring, and
> step-by-step usage by file type, see the
> **[README](../../README.md)**. For the systems-paper treatment — the
> four-boundary analysis, the algorithms, and the measurements — see
> **[the paper](../../paper/)**.

## The dance

A honeybee returns from a find. On the vertical comb, in the dark, she
performs a figure-eight dance — the **waggle dance** — whose angle to
vertical encodes direction relative to the sun, whose duration encodes
distance, whose vigor encodes quality. She does not carry the field to the
hive. She does not bring back enough nectar for the colony to evaluate. She
carries a **reference**.

<p align="center">
  <img src="../assets/dance.svg" alt="The waggle dance: a figure-eight around a waggle run whose angle to vertical encodes direction to the find, duration encodes distance, vigor encodes quality" width="640">
</p>

And here is the part that matters — the part every distributed-systems
engineer should study. Each follower **resolves the reference herself**:
she flies her own flight, with her own senses, from her own position. The
dance is not the nectar; it is an *attributed, resolvable claim* that the
nectar exists. Recruitment is **measurable** — you can count who followed,
who arrived, and who came back to dance the same field in turn, a
recruitment tree growing from one dance. And the information **expires
honestly**: bees dance only while the source still pays. When the nectar
dries up, the dancing stops, and no bee has to chase down stale copies of
yesterday's directions — there were never any copies to chase.

Twenty million years before context windows, evolution solved the handoff
problem — and it did not solve it by pasting the meadow into the prompt.

## Stigmergy

The dance is one instance of a deeper principle. In 1959, studying how
termites coordinate the construction of a mound with no blueprint and no
foreman, Pierre-Paul Grassé named it **stigmergy**: coordination through
durable marks left in a shared medium, rather than direct messages between
individuals. A termite deposits a pheromone-laced pellet; the *mark itself*
is the message, and the next termite reads it and responds. No termite
holds the plan; the plan is in the marks. Theraulaz and Bonabeau traced the
same mechanism across social insects half a century later.

Stigmergy is the escape from the tyranny of the shared context window. A
message-passing swarm must copy state into every participant — n actors, n
copies, and no copy knows about the others. A stigmergic swarm writes marks
to a common surface that every actor reads *according to its own state*. The
mapping to a handoff substrate is structural, not decorative:

| The colony | The substrate |
|---|---|
| the figure-eight encodes a vector in seconds | a ~30-byte token names an artifact plus its attribution |
| each follower flies her own flight | each consumer resolves *its* projection (the sealed matcher) |
| the follower's senses at the field | `read`/`search`: interrogate the content on arrival |
| countable recruitment | the funnel: resolve → read → run, as receipts |
| dancers who recruit dancers | lineage: children minted under their parents |
| dancing stops when the nectar stops | leases, supersession, revocation |
| the dance floor | the append-only log every mark lands on |

The colony ran a swarm with **zero shared context windows**: share names,
not payloads; let each consumer resolve per its own capability; make
consumption observable; let stale claims die at the source. Waggle is that
choreography, made durable and queryable.

## What the systems world already knew

The biology is not a lucky analogy. It is the same set of decisions the
distributed-systems field spent forty years converging on — under other
names, in other rooms, but the same decisions. There are only three ways to
move information between computational actors:

1. **Copy semantics** (message passing) — send the bytes. Simple, and every
   pathology of the agent handoff follows: n copies, no identity,
   corrections that never propagate. This is today's default.
2. **Place semantics** (shared memory) — both parties touch one location.
   Fixes duplication, but needs shared infrastructure and trust, and a raw
   location says nothing about *who may see what*.
3. **Name semantics** (references) — send a small, immutable, attributed
   *claim*, and let the resolution be computed per consumer, at the data, on
   demand.

Waggle is a commitment to the third — and each of its design decisions has a
lineage the agent literature has largely overlooked:

- **Tuple spaces.** In 1985, David Gelernter's *Linda* made coordination a
  property of a shared, content-addressable medium — *generative
  communication*, where a process posts a tuple to the space and any other
  process reads it by pattern, decoupled in time and identity. Waggle's log
  is a persistent, attributed descendant of the tuple space: the dance floor
  as a data structure.
- **Named-data networking.** In 2009, Van Jacobson and colleagues argued the
  network should route by the *name of the content*, not the address of a
  host — fetch "this article," not "bytes from that server." Waggle is that
  idea applied to the artifact a swarm of agents passes around: the consumer
  asks the reference, and resolution happens wherever the bytes live.
- **Capabilities.** Dennis and Van Horn (1966), and later Mark Miller's
  object-capability model, made *possession of an unforgeable reference* the
  unit of authority. Waggle's private tokens are capability URLs — 16
  characters of entropy where holding the token *is* the credential, refused
  by every public surface.
- **Leases.** Gray and Cheriton (1989) made cache freshness a *bounded
  promise* rather than a hopeful guess: a lease is valid until it expires,
  and then you revalidate. Waggle's `revalidate_after` is a lease; the dance
  that stops when the nectar dries is a lease that lapsed.
- **Content addressing.** Merkle's hash trees, and later IPFS, named data by
  the hash of its bytes, so identity is intrinsic and any replica's answer is
  verifiable. Waggle's snapshots are content-addressed — pinned at mint,
  immutable, and hash-provable wherever they replicate.
- **The log is the truth.** Event sourcing, and Jay Kreps' framing of the log
  as the unifying abstraction of real-time systems, made the append-only
  record primary and every view a fold over it. Waggle's manifest tables,
  funnel counts, and lineage are all folds over one payload-free log, and
  migration is a *replay* — because the destination reconstructs, byte for
  byte, from the same stream.
- **The end-to-end argument.** Saltzer, Reed, and Clark (1984) taught that
  function belongs at the ends, near the data, not in the pipe. Waggle's
  `read` and `search` move the *question* to the bytes and return budgeted
  slices — the follower's senses at the field, not the field carried home.

None of these is waggle's invention. Waggle's contribution is to notice that
the agent handoff is a distributed-systems problem in disguise, and to
assemble these well-worn primitives — with the bee's discipline of
per-consumer resolution and honest expiry — into a substrate an agent can
use in one line.

## The paradigm, stated plainly

Committing to name semantics has four consequences, and they are what make
waggle more than a shortener of file paths:

- **Information exchange becomes projection, not transmission.** A resolve
  answers with the variant matched to *this* consumer — the model-tuned
  digest, the image for the vision agent, the transcript for the one without
  ears, the fail-closed instructions for CI. One name, many truthful
  renderings; the sealed matcher guarantees the same context always gets the
  same projection.
- **Retrieval becomes interrogation.** `read` and `search` move the question
  to the bytes and return slices that *name the payload they spared you*. The
  consumer that needs three facts from a sixty-page report ingests a few
  hundred bytes, ever.
- **Lineage becomes data, not discipline.** `parent` at mint writes the
  delegation tree into the log itself — who handed what to whom is a query,
  not an archaeology project. Revoking a parent tombstones the branch.
- **History becomes reconstructable.** Every mint, resolve, read, and
  correction is an event in an append-only, payload-free log. Shuffle it,
  duplicate it, ship it to another machine: `reconstruct` rebuilds identical
  state, and the log stays dark about content — so the receipts never become
  the leak.

The bee never carries the field home. She dances, and the hive knows — who
danced, who flew, who found nectar, and when the field went dry. Our swarms
should be built the same way.

---

<p align="center">
  <em>Ready to use it? The <a href="../../README.md">README</a> is the
  five-minute path. Want the rigor — the boundary analysis, the algorithms,
  the numbers? Read <a href="../../paper/">the paper</a>.</em>
</p>
