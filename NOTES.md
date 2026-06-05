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

## Phase 1 — L0 primitives

- **DECISION: `core/` owns the engine-INTERPRETED data shapes; `moves/` owns authoring + economy
  LOGIC.** The spec's FrameProfile aggregates `cancel_windows` and `cost`, and the engine (core L2)
  must interpret them — but Appendix A lists CancelWindow/ResourceCost under `moves/` (L3). Embedding
  L3 types into a core FrameProfile would invert the layering and risk core→moves cycles. Resolution:
  the *data shapes* the engine runs (Property, HitEffect, MoveLevel, and later CancelWindow/ApCost)
  live in core; the `moves/` layer (Phase 4) adds the authoring wrappers (Move, MoveList) and the
  economy/regen/R-5 logic that operate on/produce these core shapes. Net effect: **core imports
  nothing upward at all** (stronger than the dep-cruiser rule, which only forbids core→rpg/cli/
  balance/golden). Deviation from Appendix A's literal file placement is intentional and recorded
  here; the load-bearing boundaries (the dep-cruiser rules) are unchanged.

- **DECISION: the core `Entity` does NOT hold an `RPGSheet`.** Spec §0.4 lists `rpg: RPGSheet` on the
  entity, but core importing rpg/ violates the single-bridge boundary. Per §3.3 the engine runs
  *resolved* frame data and "never sees a stat", so the entity needs no sheet: stats are compiled into
  resolved FrameProfiles/MoveLists by rpg/compiler.ts before the engine runs. The entity references its
  moves by stable ID (portability: ID-based references, not object identity). The RPGSheet stays in rpg/.

- **DECISION: the resource POOL (`Resources`) lives in core; the resource ECONOMY lives in `moves/`.**
  `Entity` owns a plain integer `Resources` record (hp/stamina/poise/focus/ap + caps). Core can't
  import moves, so the pool type is core. `moves/resources.ts` (Phase 4) defines spend/gain/regen and
  `moves/economy.ts` the AP/R-5 logic, all operating on the core `Resources` data — downward deps only.

- **DECISION: frame advantage is DERIVED, never stored (invariant I-1).** FrameProfile has no
  `on_hit`/`on_block` fields; `onHit()/onBlock()` compute `hitstun − recovery` / `blockstun − recovery`
  from the profile. Hand-setting an inconsistent advantage is structurally impossible. Tick-level
  resolution (Phase 3) reproduces this quoted advantage for a last-active-frame connect.

- **DECISION: move PHASE (STARTUP/ACTIVE/RECOVERY) is derived from `elapsed = T − startTick`** via
  `phaseAt()`, the single source of truth; the engine sets the stored `EntityState.kind` from it each
  tick. Property windows `[from,to]` are inclusive and measured in the same `elapsed` frame.

- **DECISION: `Tracking` is encoded ONLY in `ReachProfile` (lateral_band/step_in/track_side, L1), not
  duplicated as a `Property`.** Keeps all contact math in `spatial/lane.ts` (audit C-7) and avoids two
  sources of truth. The spec lists Tracking in both §0.3 and §1.2; we keep the §1.2 spatial encoding.
