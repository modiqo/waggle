# 05 — Social Minting: the Human Face of the Same Token

*Revision 2. Repositioned per the standing ruling: social minting is a
**capability of the primitive, not a battlefront**. It exists because (a) the
first production user (rote) needs it day one, (b) cross-harness sharing runs
through humans and their channels (Slack, X, QR on a slide), and (c) "the same
token unfurls in Slack and hands the Codex subagent its own variant" is the
sentence that makes waggle legible. "Dub alternative" is not a positioning
this project uses — see 01 §3.*

## 1. The shape: mint returns a SharePackage, not a string

The insight: **the token is one object; the channel determines the artifact
wrapped around it.** A short URL alone is half a deliverable — the caller
still has to write the post, build the OG tags, render the QR.
`waggle-social` finishes the job:

```rust
pub fn package(
    manifest: &AttributionManifest,
    host: &HostConfig,                // base URL, brand hints
) -> SharePackage;

pub struct SharePackage {
    pub short_url: ShortUrl,                       // https://{host}/x/{token}
    pub artifact: ChannelArtifact,                 // per manifest.channel
}

pub enum ChannelArtifact {
    XPost      { text: String },                   // ≤280 incl. link, outcome-first
    SlackBlock { mrkdwn: String },                 // title · one-liner · stats line
    LinkedIn   { text: String },
    Email      { subject: String, body: String },
    Qr         { svg: String, png: Option<Vec<u8>> },   // `qr` feature
    Terminal   { card: String },                   // copyable curl line + text card
    OgMeta     { html_head: String },              // for hosts rendering unfurl pages
    Custom     { template_id: TemplateId, rendered: String },
}
```

Templates ship with sane defaults, overridable via `HostConfig` (plain string
templates with named slots — no template-engine dependency). All renderers
are pure: same manifest + config → byte-identical artifact — snapshot-testable.
The `package` capability is also exposed as an optional MCP tool (`share`) by
the servers in 09, so an *agent* can produce the human-facing artifact when
its task is "share this with the team."

## 2. QR as a channel, not a feature

`Channel: qr-event` mints like any channel; the artifact is the code itself.
Rendering uses [`qrcodegen`](https://crates.io/crates/qrcodegen) (Project
Nayuki's dependency-free encoder — correct QR is not a place for NIH), error
correction level M, standard quiet zone, SVG primary with PNG behind the
`qr-png` feature. Because the QR encodes the token URL, everything tokens do —
attribution, revocation (yesterday's conference slide can be killed), funnel —
applies to printed matter.

## 3. Unfurl building: deterministic OG from snapshot metadata

`OgMeta` renders exclusively from mint-time `TargetMeta` (invariant I-3):
`og:title/description/image`, `og:url` (the *canonical* URL — unfurl consumers
index identity, not distribution), Twitter equivalents. No scraping, no HTML
parsing dependency, no SSRF surface. Live-stat card images are a serve-layer
concern (08 §4).

## 4. The role social plays in the agent story

The cross-harness path (06 §7, scenario B) runs through human channels: an
agent's artifact leaves its harness as a link in Slack, an unfurl a teammate
can read, a QR on a slide — and re-enters agenthood when the teammate's
harness resolves the same token with *its* context. Social renderers are the
bridge surface of the coordination story, not a separate product. Secondary
integrations (Rust services minting in-product share links, CLI tools,
newsletters) remain welcome and documented in examples — pulled by demand,
never pushed as positioning (01 §5's dissent conditions govern when that
changes).

## 5. What social minting deliberately does not do

- **No click-side JavaScript.** Attribution is server-side at the redirect.
- **No UTM emission**; no importers unless demand appears (**[open]**, v2+).
- **No live-preview scraping** (I-3); no per-recipient identity — a token is a
  channel, not a person.
- **No dashboard product.** The serve layer exposes funnel JSON; rendering a
  marketing UI around it is explicitly out of scope (ruling, 01 §3).
