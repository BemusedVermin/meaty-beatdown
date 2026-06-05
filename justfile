# justfile — common commands via `just` (https://just.systems). Cross-platform: on Windows recipes
# run in PowerShell, on Linux/macOS in sh. Recipe bodies are single npm/npx invocations (shell-
# agnostic), and multi-step flows use recipe dependencies, so they behave identically on both.

set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# List the available recipes (the default when you run `just` with no arguments).
default:
    @just --list

# Install dependencies.
install:
    npm install

# The full green gate: typecheck + lint + dependency-cruiser + tests.
check:
    npm run check

# Individual gate steps -------------------------------------------------------

typecheck:
    npm run typecheck

lint:
    npm run lint

# Module-boundary check (dependency-cruiser).
depcruise:
    npm run depcruise

# Run the test suite once.
test:
    npm test

# Re-run tests on change.
test-watch:
    npm run test:watch

# Apps & tooling --------------------------------------------------------------

# Drive the scripted fight scenarios; pass an id for one, e.g. `just fight sidestep-ap`.
fight scenario="":
    npm run fight {{scenario}}

# Balance / consistency audit (spec Appendix B) — a PASS/FAIL row per check id.
audit:
    npm run audit

# (Re)emit the golden vectors from the scenarios.
golden-emit:
    npm run golden:emit

# Verify every golden vector replays byte-identically (the cross-language contract).
golden-verify:
    npm run golden:verify

# CI ---------------------------------------------------------------------------

# The automated pipeline: clean install, then the full green gate. Unifies local + CI testing.
ci: install check
