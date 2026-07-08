# waggle — development recipes. `just --list` for the set; CI runs the same.

default:
    @just --list

# Build and install the waggle CLI from this checkout (like rote's dev-install)
dev-install:
    cargo install --path crates/waggle-cli --locked --force

# Run the full-lifecycle demo (docs/guide/06) against a throwaway store
demo:
    bash scripts/demo.sh

# Boot wrangler dev and run the full Miniflare matrix (CP-10e) — needs node
edge-test:
    #!/usr/bin/env bash
    set -euo pipefail
    PORT=43811
    rm -rf edge-worker/.wrangler/state   # fresh hive per run — DO storage persists
    (cd edge-worker && npx --yes wrangler dev --port $PORT >/tmp/wrangler-edge.log 2>&1 &)
    for i in $(seq 1 420); do curl -sf http://127.0.0.1:$PORT/health >/dev/null && break; sleep 1; done
    WAGGLE_EDGE_TESTS=1 WAGGLE_EDGE_EXTERNAL_PORT=$PORT cargo test -p waggle-edge-worker --test miniflare -- --nocapture
    WAGGLE_EDGE_URL=http://127.0.0.1:$PORT WAGGLE_EDGE_BEARER=dev-tenant-token-0123456789abcdef cargo test -p waggle-cli --test federation e3_three_tier -- --nocapture
    pkill -f "wrangler dev" || true

# Model-check the cache layer with loom (15 §5.2)
loom:
    RUSTFLAGS="--cfg loom" cargo test -p waggle-store-sqlite --test loom_cache --release

# Run the criterion hot-path benchmarks (see benches/PERF.md)
bench:
    cargo bench -p waggle-core --bench hot_paths
    cargo bench -p waggle-store-sqlite --bench store_paths
    cargo bench -p waggle-mcp --bench query_paths

# Format everything
fmt:
    cargo fmt --all

# The read-only quality wall: fmt-check + clippy(-D warnings) + file-size lint
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo run -p xtask -- lint-file-size

# Workspace unit + integration tests
test:
    cargo test --workspace

# Verify the sans-I/O crates compile for the Workers target
wasm-check:
    cargo check -p waggle-core -p waggle-ops -p waggle-social --target wasm32-unknown-unknown

# Regenerate COMMANDS.md from the operations catalog
gen-docs:
    cargo run -p xtask -- gen-docs

# Everything a commit must pass, in order
preflight: check test wasm-check
    @echo "preflight green"

# What CI runs
ci: preflight gen-docs
    git diff --exit-code COMMANDS.md
