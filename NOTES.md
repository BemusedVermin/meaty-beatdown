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

## Phase 3 — L2 engine

- **DECISION: emergent frame advantage is ANCHORED to the quoted on_hit/on_block, not to the exact
  connect frame.** On a contact at the attacker's move (started S, total = startup+active+recovery),
  the defender's stun begins at `S + startup + active` (the first recovery tick) and lasts `stun` ticks
  ⇒ `defenderReady = S+startup+active+stun`, while `attackerReady = S+total`. So the emergent advantage
  is exactly `stun − recovery` = the I-1 quote, *independent of which active frame connected*. This
  trades real-FG "meaty"/late-active-frame nuance for exact consistency with the frame-data contract
  (audit C-1/C-3) and simpler determinism. Revisit if meaty okizeme becomes a design goal.

- **DECISION: the resolution loop lives in `core/engine.ts`, not `tick.ts`.** Appendix A puts
  `advance_until_next_decision()` in tick.ts, but mixing the L2 scheduler with the L0 time vocabulary
  muddies the file. `tick.ts` stays the Tick/Ticks primitives; `engine.ts` holds MatchState, the Agent
  interface, the loop, and contact application. Boundary rules are unaffected (both are in core/).

- **DECISION: `doesHit` is a pure SPATIAL predicate; the active-frame gate stays in the engine.** The
  spec's `does_hit(attacker, defender, tick)` includes "the move is on an active frame", but that is
  engine timing. `collectContacts` only calls `doesHit` for attackers whose `phaseAt === "ACTIVE"`, so
  `spatial/lane.ts` owns range/height/lateral/type only (audit C-7) and needs no tick/move coupling.

- **DECISION: actionability is `readyTick ≤ T`; the stored `EntityState.kind` is a label.** A move's
  active frames lie strictly before `readyTick`, so once `readyTick ≤ T` the entity is free regardless
  of its label. `normalizeActionable` lazily transitions any now-actionable entity to NEUTRAL,
  re-centers `offset` to 0, and auto-faces the opponent (§1.1). `advanceUntilNextDecision` steps one
  tick at a time and returns at the FIRST tick some `readyTick ≤ T`, so at an ACTION pause `T == min
  readyTick` and `computeRegime`'s ready_tick equality test aligns exactly.

- **DECISION: cancel checkpoints are edge-triggered once per window entry.** `cancelEligible` fires only
  when `elapsed === firstEligibleTick`, where `firstEligibleTick = startupCancelable ? window.from :
  max(window.from, startup)` (no-startup-cancel, decision 6). Because `stepOneTick` always advances the
  clock before the next checkpoint test, a declined cancel cannot re-fire ⇒ no infinite loop, no
  consumed-checkpoint bookkeeping. (Artifact: the cancel commits at the tick AFTER detection — fine for
  a turn-based engine.) Cancel targets for Phase 3 = the whole move table; Phase 4 restricts via
  CancelWindow.into + AP cost. Hit-confirm is honest: CancelView carries the actual contact fact.

- **DECISION: damage scaling uses `toIntRound` (half-up), not `toInt` (floor).** `0.9` in 16.16 is
  `0.899994`, so compounding `mul` + floor makes `100×0.9 → 89`. A pure-integer half-up conversion
  (`floor((raw+32768)/65536)`, no `toNumber`) gives the intuitive `90/81/73…` and CH `10→13`, stays
  deterministic/portable, and still strictly decreases so combos terminate.

- **DECISION: movement is a discrete hop at the first active frame (decision 9).** `Motion {lane,
  offset}` repositions the ENTITY (distinct from `ReachProfile.advance`, which only extends the hitbox).
  Applied once at `elapsed === startup`; `offset` re-centers when the entity next becomes actionable
  (auto-facing). Lateral is set during the move so a sidestep dodges an opponent's same-tick active.

- **DECISION: PROJECTILE_SPAWN is a throwing stub (decision 8).** The Property kind + data slot exist,
  but `guardNoProjectile` throws if a PROJECTILE_SPAWN window ever goes active, so a move that actually
  spawns one fails loudly rather than silently no-op'ing. The projectile entity is deferred (spec §2.9).

## Phase 4 — L3 moves/resources/economy

- **DECISION: AP refills ONLY on entering NEUTRAL, not on every actionable transition.** Refilling to
  AP_max whenever an entity becomes actionable would mask the parry AP refund (decision 7) and make all
  conditional `ap_gain` pointless (always capped to max). Instead a fresh exchange (NEUTRAL regime)
  refills the turn budget; in PRESSURE you act with your CURRENT AP, so conditional gains (parry +2,
  ON_HIT links) genuinely extend offense — which is the whole point of the tempo model (spec §3.5). The
  ⚠️ "ap_refill = AP_max vs carryover" fork is resolved this way for the prototype.

- **DECISION: removed the `CANCELABLE` Property; `cancelWindows` (spec §3.4) is the single source of
  truth for cancels.** The Property and the richer CancelWindow were redundant. The engine drives cancel
  checkpoints off `FrameProfile.cancelWindows` (from/to/gate/into/cost). Hit-confirm windows are authored
  in RECOVERY so the contact is settled when the edge-triggered checkpoint evaluates its gate.

- **DECISION: a cancel charges `window.cost + target.cost`.** Each chained action pays its own ap_cost
  (tempo) AND the cancel window's cost (the combo tax, usually Focus). Unaffordable ⇒ the cancel is
  refused and the string ends (governors 1 & 4). `apGain` is the target move's, applied on its contact.

- **DECISION: cost/cancel TYPES + resource OPS live in core; `moves/` holds authoring + the static
  linter.** `core/cost.ts` (ResourceCost, ApGain, CancelWindow) and `core/resource-ops.ts` (spend/gain/
  refill/regen) are engine-interpreted, so core. `moves/move.ts` = Move/MoveList authoring; `moves/
  resources.ts` = the four-meter model + a re-export facade; `moves/economy.ts` = the R-5 cancel-graph
  linter + governor report. Extracted `core/ids.ts` (MoveId/EntityId) as a leaf so cost.ts can name a
  MoveId without an entity↔frameprofile↔cost cycle.

- **DECISION: R-5 = no non-negative simple cycle in the cancel graph (sum of net AP ≥ 0).** `netAp(move)
  = apGain − apCost`; `findPositiveApCycles` enumerates simple cycles (anchored at each cycle's smallest
  member) and flags any whose summed net AP ≥ 0. This is the rigorous "scan for positive-weight cycles"
  (audit C-10), stronger than the spec's per-move shorthand. `CancelWindow.into` is an explicit MoveId
  list; the spec's CATEGORY shorthand is deferred.

- **DECISION: hitstun decay via a per-entity `comboCount` (governor 3).** A hit on an already-stunned
  defender increments comboCount; `effectiveHitstun(base, comboCount)` subtracts HITSTUN_DECAY_PER_HIT
  per extra hit (floored at MIN_HITSTUN) so chained advantage eventually goes minus and the combo must
  end. comboCount resets when the defender becomes actionable (the combo dropped).

## Phase 5 — L4 RPG

- **DECISION: `sheet.ts`/`equipment.ts` are PURE DATA (no core import); ALL core-dependent derivation
  lives in `compiler.ts`, the single bridge.** Anything needing CONFIG, Fixed, or FrameProfile (apMax
  via the tempo-tier curve, the frame-data levers, range→fixed-point) is computed in compiler.ts. The
  one derivation `sheet.ts` keeps — `tempoMod` — uses the documented integer-equivalent
  `floor((dex+wis+1)/2)` (= roundHalfUp((dex+wis)/2)) so it needs no core import. dependency-cruiser
  proves compiler.ts is the only rpg→core bridge (audit C-5).

- **DECISION: CHA has no derived combat lever (decision 5).** With tempo derived from DEX+WIS, CHA loses
  the spec's §3.5.4 AP assignment and is not wired to any frame-data lever in the prototype; its identity
  comes from feint/intimidate FOCI and content (out of the compiler's scope). Keeps R-2 (one lever per
  attribute) clean: STR→damage/armor, DEX→speed/advance, CON→survivability, INT→Focus, WIS→defense reads.

- **DECISION: weapon ranges are plain integer "lane units"; the compiler converts to fixed-point.** Keeps
  equipment.ts free of core/fixed. STR damage bonus applies to `moveClass` HEAVY/THROW (or level THROW).
  R-3 bonuses (DEX startup reduction, STR armor hits) are CONFIG-capped.

- **DECISION: R-4 is Pareto dominance over (reach, speed = −startupDelta, damage).** A weapon that is
  ≥ another on all three axes and strictly > on one violates the tradeoff triangle; `paretoDominations`
  returns every such ordered pair (audit R-4). Lives in equipment.ts (pure weapon data, no core).

## Phase 6 — balance tooling + sample content

- **DECISION: `content/` is swappable sample DATA, consumed only at the top.** A new dependency-cruiser
  rule (`engine-is-content-agnostic`) forbids core/spatial/moves/rpg/serialize from importing content/
  — "the engine never hard-codes a move." content/builders.ts fills FrameProfile defaults;
  content/sample.ts has ~12 moves / 3 weapons / 2 archetypes, authored to pass R-1/R-4/R-5.

- **DECISION: budget weights + MOVE_VALUE live in `balance/budget.ts` (tooling, floats allowed).** The
  budget identity is INFORMATIONAL — `budgetReport` flags moves whose value deviates > ±ε from the
  archetype mean (heavy_cleave shows up for Borin, as expected of a big slow armored move); it is not a
  hard pass/fail. R-2 is encoded as an explicit lever map (no attribute both offensive+defensive); R-3
  checks the CONFIG caps exist.

- **DECISION: `audit.ts` is a top-level script (prints + sets process.exitCode); not imported by any
  test.** Its top-level print runs only under `npm run audit`; the rule functions are unit-tested in
  budget.test.ts. Structural invariants (C-2/C-5/C-6/C-7/C-9) pass with a note citing their enforcement
  mechanism (dependency-cruiser / engine / config); the rest (C-1/C-3/C-4/C-8/C-10/C-11, R-1..R-5) run
  live over the content.

- **DECISION (content tuning): LINEAR strikes need lateralBand < a 1-unit sidestep.** With the default
  band = 1.0, a single sidestep (offset ±1) lands exactly on the inclusive edge and still connects, so a
  sidestep wouldn't dodge. The sample LINEAR strikes use band 0.5 so a sidestep genuinely evades them
  (and HOMING's stepIn realigns), which is what audit C-8/C-11 verify.
