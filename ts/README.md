# TICK

A headless, deterministic **combat + RPG engine**: a turn-based RPG with fighting-game frame data on a
shared tick timeline. Both fighters live on one 60 Hz clock; "whose turn is it" falls out of a single
`ready_tick` comparison, so neutral, pressure, and punish all emerge with no special-casing. Stats and
equipment *compile* to frame data; the engine *interprets* it.

> **The TypeScript is disposable scaffolding.** The durable deliverables are (a) the design
> (`docs/frame_rpg_spec.md`), (b) a **pure, deterministic core** in language-portable constructs, and
> (c) a **language-neutral golden-vector suite** that is the behavioral contract for a reimplementation
> in another language. Every decision optimizes for *"reimplements cleanly and verifiably elsewhere,"*
> not TS cleverness: no inheritance, no reflection, no float math in gameplay, plain data + free
> functions only. See **[PORTING.md](./PORTING.md)**.

**Status:** complete (Layers 0–4 + tooling). `npm run check` is green — **160 tests**, **0 dependency
violations**, audit **16/16 PASS**, **13 golden vectors** replay byte-identically. Out of scope:
graphics, game UI, and Layer 5 (encounter / AI / progression).

## Quick start

```bash
# all npm / just commands run from this ts/ directory
npm install
npm run check        # the full green gate: typecheck + lint + depcruise + test
```

| Command | What it does |
|---|---|
| `npm test` | Vitest — 160 tests, including the golden-vector replay suite |
| `npm run fight` | Drive the scripted scenarios and print the tick timeline (`npm run fight <id>` for one) |
| `npm run audit` | Balance/consistency audit — a PASS/FAIL row per check id (spec Appendix B) |
| `npm run golden:emit` / `golden:verify` | Emit / replay-verify the cross-language golden vectors |
| `npm run typecheck` · `lint` · `depcruise` | Individual gate steps |

`npm run fight sidestep-ap` prints, for example:

```
  ── Tempo / AP economy — jab → cancel homing → finisher denied (out of AP) ──
  T  │ Borin                         │ Reza   │ Engine
  ───┼───────────────────────────────┼────────┼─────────────────────────────
  0  │ ▶ Tempo Jab (ap 3)            │ · wait │ [NEUTRAL]
  5  │                               │ · wait │ [PRESSURE] Borin HIT → Reza
  8  │ ↳ cancel → Homing Sweep       │        │
  16 │                               │        │ Borin HIT → Reza
  20 │ ✗ Heavy Cleave (can't afford) │        │   ← AP exhausted: the string ends
```

## Architecture

Layers expose narrow interfaces and the dependency graph runs strictly downward. The single most
important rule: **the RPG layer never reaches into the engine** — `rpg/compiler.ts` is the *only* bridge.

```
core/        L0/L2  fixed (16.16), tick, frameprofile (+ invariant I-1), entity, resolver,
                    regime, engine (the resolution loop + Agent interface), cost, config
spatial/     L1     lane — the single doesHit contact predicate (pos/offset/height, all Fixed)
moves/       L3     move/MoveList, resources, economy (AP, the R-5 no-positive-cycle linter)
rpg/         L4     sheet, equipment, compiler   ← the ONLY bridge into core/spatial/moves
balance/            budget (MOVE_VALUE + R-1..R-5), audit (C-1..C-11)        [tooling]
serialize/          canonical integers-only state/trace codecs               [shared, pure]
content/            swappable sample moves/weapons/archetypes                 [data]
cli/                fight-runner, agents (Scripted/Replay/Interactive), timeline-printer  [edge]
golden/             *.json vectors + emit/verify harness                      [edge]
```

These boundaries are **enforced, not just documented**:

- **dependency-cruiser** fails the build on a non-`compiler.ts` L4→core import, on the engine importing
  content ("the engine never hard-codes a move"), and on any dependency cycle.
- **eslint** fails on a non-exhaustive `switch` over a tagged union, on `async`/`Promise` anywhere in
  the core (the engine is strictly synchronous and value-based), and on `fixed.toNumber` (display-only)
  in gameplay code.

## What's inside

- **Deterministic combat** keyed off `ready_tick`: NEUTRAL (both commit hidden, then reveal) vs PRESSURE
  (the plus player acts with full info). Contact resolution in exact priority — invuln > parry > block >
  armor > hit — with throws resolved separately (they beat parry/block/armor; clash on throw-tech).
- **16.16 fixed-point** spatial + damage math (no floats in gameplay), with exact documented rounding
  and a reference-table + property test suite — the portability linchpin.
- **AP tempo economy** (chain actions while plus; conditional `ap_gain` extends offense) and **four
  independent combo governors** (Focus cost, juggle decay, hitstun decay, AP exhaustion).
- **Tekken-style sidestep**: a LINEAR move whiffs an off-axis defender; HOMING/TRACKING realign — all
  through the one `doesHit` predicate.
- **RPG layer**: WWN-style attributes/skills/foci + equipment compile to resolved frame data; tempo is
  derived from DEX+WIS; weapon = your spacing identity.
- **Balance as a checkable property**: the MOVE_VALUE budget identity and the R-1..R-5 / C-1..C-11
  audit, run over swappable sample content.

The design is governed by **12 locked decisions** (throws beat armor, deterministic hit/miss, fixed
damage, the AP=tempo model, tempo = DEX+WIS, no startup cancels by default, parry refunds Focus+AP,
deferred projectiles, sidestep-hop only, 16.16 fixed-point, tagged unions, synchronous value-based
core). They are listed in full — with rationale — in **[CLAUDE.md](./CLAUDE.md)**.

## Documentation

| File | What it is |
|---|---|
| [`../docs/frame_rpg_spec.md`](../docs/frame_rpg_spec.md) | The design — the source of truth |
| [`../docs/mechanics.md`](../docs/mechanics.md) | Mechanic-by-mechanic reference of everything the engine implements |
| [`../docs/mechanics-gap-analysis.md`](../docs/mechanics-gap-analysis.md) | Cross-genre fighting-game mechanics catalog + what TICK has / lacks |
| [`CLAUDE.md`](./CLAUDE.md) | Architecture, enforced boundaries, and the 12 locked decisions |
| [`PORTING.md`](./PORTING.md) | The pure-core contract, fixed-point semantics, golden-vector schema, and the porting checklist |
| [`NOTES.md`](./NOTES.md) | Every `// DECISION:` made resolving an ambiguity, with its tradeoff |

## Stack

TypeScript (strict, ESM), Node 20+. No runtime dependencies in the engine core. Tests with Vitest;
the CLI runs via tsx. Cross-platform npm scripts (developed on Windows 11).
