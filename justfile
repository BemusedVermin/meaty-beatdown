# TICK build commands. Cargo runs inside rust/ (CLAUDE.md working agreement #4).

set working-directory := "rust"
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]

default: ci

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

test:
    cargo test --workspace

# The content audit (spec §13.4): I-1 sanity, R-5 cycle scan, juggle termination.
# Runs over the test kit until shipped content lands (Phase 6).
audit:
    cargo test -p engine --test audit

# The full local gate; mirrors .github/workflows/ci.yml (`test` includes the audit
# and the proptest governor suites).
ci: fmt-check clippy test
