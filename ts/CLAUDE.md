# TICK — combat + RPG engine (prototype)

A **headless, deterministic** implementation of the engine in `../docs/frame_rpg_spec.md`
(Layers 0–4) plus balance tooling, a CLI fight-runner, and a golden-vector contract.

> **Framing — the TypeScript is disposable scaffolding.** The durable deliverables are (a) the
> design, (b) a pure deterministic core in language-portable constructs, and (c) a language-neutral
> golden-vector test suite that is the behavioral contract for reimplementation in another language.
> Optimize every decision for *"reimplements cleanly and verifiably elsewhere,"* not TS cleverness:
> no inheritance hierarchies, no reflection/metaprogramming, no decorators, no runtime deps in the
> core, **no float math in gameplay logic**, plain data + free functions only.

Out of scope: graphics, game UI, enemy/encounter/progression design (spec Layer 5).

## How to run

| Command | What it does |
|---|---|
| `npm run typecheck` | `tsc --noEmit` (strict) |
| `npm run lint` | eslint: exhaustiveness, async-ban in core, `toNumber`-ban in gameplay |
| `npm run depcruise` | dependency-cruiser: build-failing module-boundary rules |
| `npm test` | vitest (includes `golden:verify` once Phase 8 lands) |
| `npm run check` | typecheck + lint + depcruise + test (the green gate) |
| `npm run fight` | CLI fight-runner (Phase 7) |
| `npm run audit` | balance/consistency audit, pass/fail per check ID (Phase 6) |
| `npm run golden:emit` / `golden:verify` | emit / replay golden vectors (Phase 8) |

**Green gate between every phase:** `npm run check` must pass before advancing. Commit per gate.

## Architecture — layers (spec Appendix A), dependency graph strictly downward

```
core/        L0/L2  fixed, tick, frameprofile (+ invariant I-1), entity, resolver, regime, config, assertNever
spatial/     L1     lane (Fixed pos/offset/height; doesHit with tracking)
moves/       L3     move, resources, economy (AP, R-5 cycle check)
rpg/         L4     sheet, equipment, compiler   ← the ONLY bridge into core/spatial/moves
balance/            budget (MOVE_VALUE linter), audit (C-1..C-11, R-1..R-5)   [tooling]
serialize/          canonical state/trace codecs (integers only)              [shared, pure]
cli/                fight-runner, agents (Scripted/Interactive/Replay), timeline-printer  [edge]
golden/             *.json vectors + emit/verify harness                      [edge]
```

**The single most important rule:** L4 (rpg) never reaches into the engine. Stats + equipment are
*compilers that emit FrameProfiles*; the engine is an *interpreter* that runs them. `rpg/compiler.ts`
is the one bridge.

### Boundaries are enforced, not just documented

dependency-cruiser (`.dependency-cruiser.cjs`) **fails the build** on:
1. Anything in `rpg/` importing `core/`/`spatial/`/`moves/` **except `rpg/compiler.ts`** (the bridge).
2. `core/` importing `rpg/`/`cli/`/`balance/`/`golden/`.
3. `spatial/`/`moves/`/`serialize/` importing `rpg/` or `cli/`.
4. Anything outside `cli/`/`golden/` importing node I/O builtins (sync, pure core — decision 12).
5. Any circular dependency.

eslint (`eslint.config.js`) **fails the build** on:
- A non-exhaustive `switch` over a tagged union (`switch-exhaustiveness-check`).
- `await`/async/`Promise`/`new Promise` anywhere in `core,spatial,moves,rpg,balance,serialize`.
- Importing `fixed.toNumber` into gameplay code (`core,spatial,moves,rpg,serialize`).

Contact math: **all** range/lateral/height math lives in `spatial/lane.ts` behind the single
`doesHit` predicate (audit C-7). No other module computes contact.

> Never relax a module boundary to "simplify." The boundaries ARE the design and the portability
> story. Composition over inheritance; no premature generics, manager god-objects, or plugin layers.

## The 12 locked decisions (these override every ⚠️ in the spec)

1. **Throws beat armor.** Armor absorbs strikes only; throws connect through armor.
2. **Deterministic combat.** Hit/miss is decided by spacing + timing + hitboxes — no d20 to-hit.
   Dice remain only for out-of-combat skill checks and the opposed throw-tech/grapple contest.
3. **Fixed damage** for the prototype. `DAMAGE_VARIANCE` flag in config, default `false`; no RNG
   wired into damage.
4. **AP = the Tempo model** (spec §3.5): while holding initiative you chain actions, each paying
   `ap_cost`, until you can't/won't pay; AP-positive moves extend the turn. Not a fixed per-turn budget.
5. **AP from a derived tempo stat** = blend of DEX and WIS:
   `tempoMod = roundHalfUp((dexMod + wisMod) / 2)`; `AP_max = AP_BASE + tempoTier`. Curve constants in
   `config.ts`. Tempo is **derived, not a 7th attribute**; no core attribute gains a second major lever.
   (Replaces the spec's CHA assignment in §3.5.4/§4.2 — fix in code comments; do **not** edit the spec.)
6. **Startup cancels disallowed by default.** A move is cancelable only from active/recovery windows
   unless it carries `startupCancelable: true`. (Anti-react-to-reveal; makes feints real — spec §2.10.)
7. **Parry refunds both Focus and AP** on success (spec §2.6, §3.5.2).
8. **Defer the projectile simulation.** Keep `PROJECTILE_SPAWN` in the Property enum + data slot; do
   not implement the projectile entity. A single `// DEFERRED (spec §2.9)` stub that throws if invoked.
9. **Sidestep hop only.** Discrete `SIDESTEP_L`/`SIDESTEP_R`; no continuous sidewalk.
10. **Fixed-point spatial math (16.16).** All positional quantities are signed 16.16 fixed-point,
    never floats (`core/fixed.ts`). Floats allowed only for display in `cli/` and the budget linter's
    scoring in `balance/`. Exact rounding semantics: `mul` floors toward −∞ (arithmetic `>>`); `div`/
    `fromRatio` truncate toward zero; `toInt` floors; `roundHalfUp` ties toward +∞. See `fixed.ts` /
    PORTING.md.
11. **All sum types are tagged unions** with a literal `kind`/`tag` discriminant, matched by exhaustive
    `switch` with an `assertNever(x: never)` default. No stringly-typed unions, no boolean-flag soups.
    Maps 1:1 onto Rust enums / C# records / C++ `std::variant`.
12. **The core is strictly synchronous and value-based.** No Promise/async/await/callbacks/timers/IO in
    `core,spatial,moves,rpg,balance`. The engine asks an `Agent` for an `Action` value synchronously.
    All async/stdin/file IO lives only in `cli/` and the golden-vector harness edge.

## Portability contract (what must survive the rewrite)

- **Pure core step function** `step(state, decisions) → state'`: no RNG, no clock, no ambient/global
  state, no IO. Same inputs ⇒ byte-identical outputs, in any language.
- **ID-based references**, not object identity (entities/moves/etc. referenced by stable IDs).
- **Integers-only on the wire**: serialized state/traces contain no floats; fixed-point serializes as
  its integer `raw`. This is what makes golden vectors comparable across languages.
- **Tagged unions → native enums** (decision 11).

## Working agreement

- Phase by phase; green gate (`npm run check` + the phase's listed tests) before advancing; commit per
  gate naming the phase + spec sections.
- Maintain this file (architecture, boundaries, 12 decisions) and `NOTES.md` (every `// DECISION:`).
- When an ambiguity isn't covered by the spec or the decisions, make the smallest, most modular, most
  portable choice and mark it `// DECISION:` with the tradeoff (log it in `NOTES.md`).

## Stack notes

- TypeScript strict (`strict`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`,
  `noImplicitReturns`), Node 20+, ESM. Module resolution is `Bundler` → **extensionless** relative
  imports; the code runs via `tsx`/`vitest`, never plain `node` (we never emit JS).
- No runtime dependencies in the engine core. Windows-friendly npm scripts (no bash-isms).
