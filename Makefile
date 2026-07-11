# Thin wrappers so `make <target>` mirrors the canonical `just` recipes.
# The justfile is the source of truth; see `just --list`.

.PHONY: bench bench-crit preflight

# Regenerate the paper's benchmark artifacts (design doc 22): Tier-1
# cost-model sweep + tables and the reconstruction-determinism gate.
bench:
	cargo run -p waggle-bench -- all

# The criterion hot-path microbenchmarks.
bench-crit:
	just bench

# Everything a commit must pass.
preflight:
	just preflight
