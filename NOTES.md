# NOTES — decision log

Every `// DECISION:` made while resolving an ambiguity not covered by the spec or the 12 locked
decisions is recorded here, with its tradeoff. (The 12 locked decisions themselves live in CLAUDE.md.)

## Phase 0 — scaffold + fixed-point

- **DECISION: module resolution = `Bundler`, extensionless relative imports.** The code is run only
  via `tsx` (CLI) and `vitest` (tests), never emitted to JS or run under plain `node`, so we don't
  need NodeNext `.js`-extension specifiers. Tradeoff: not directly `node`-runnable without a loader;
  acceptable because execution always goes through tsx/vitest. Keeps imports clean and keeps
  dependency-cruiser resolution simple. Disposable-TS concern only — does not affect portability.

- **DECISION: `verbatimModuleSyntax: true`.** Forces explicit `import type` vs value imports, which
  maps cleanly onto a port's type/value separation and keeps `isolatedModules` honest. Minor friction
  (must annotate type-only imports) accepted for portability discipline.

- **DECISION: combat multipliers (CH ×1.25, juggle ×0.9) stored as `Fixed`, not floats.** Keeps all
  damage scaling integer/deterministic (decision 10). Applied via `fixed.mul` then `toInt`. `0.9`
  becomes `fromRatio(9,10) = 58982` (truncates to ≈0.89999) — deterministic and documented; a port
  reproduces the same truncation.

- **DECISION: `toNumber`-ban implemented as a `no-restricted-syntax` selector**
  (`ImportSpecifier[imported.name='toNumber']`) rather than `no-restricted-imports` path patterns.
  Path-pattern matching of relative specifiers is brittle; banning the imported *name* is
  path-independent and robust. balance/ is excluded from the ban (its budget linter may use floats for
  scoring); cli/ is excluded (display). Caveat: a `import * as Fixed` namespace + `Fixed.toNumber(...)`
  would bypass the specifier check — convention is to always use named imports in gameplay code.

- **DECISION: async-ban also applied to `serialize/`.** Decision 12 lists core/spatial/moves/rpg/
  balance; serialize isn't named but must be pure and synchronous (it's the integers-only codec at the
  determinism boundary), so it gets the same async ban. It also gets the `toNumber` ban (integers only).

- **DECISION: `tempoTier` thresholds `[1,3,5]` placed in config as TUNING.** The decision-5 curve
  (`AP_max = AP_BASE + tempoTier`) needs a concrete tier mapping; `tempoMod ≥ 3 → tier 2 → AP_max 5`
  reproduces the spec's worked-example "tempo" variant. Marked TUNING; revisit in Phase 5/6.

- **DECISION: dropped the `no-orphans` dependency-cruiser rule (for now).** During scaffolding every
  stub legitimately has no importers, so orphan warnings would be pure noise and obscure a clean gate.
  Re-add later if dead-code detection becomes valuable once the graph is wired.
