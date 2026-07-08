# waggle — development recipes. `just --list` for the set; CI runs the same.

default:
    @just --list

# Build and install the waggle CLI from this checkout (like rote's dev-install)
dev-install:
    cargo install --path crates/waggle-cli --locked --force

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
