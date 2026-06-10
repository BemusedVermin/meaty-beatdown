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

# The full local gate; mirrors .github/workflows/ci.yml. (`audit` joins at Phase 2.)
ci: fmt-check clippy test
