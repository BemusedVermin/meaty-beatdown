# PORTING.md — reimplementing the TICK engine in another language

The TypeScript in this repo is **disposable scaffolding**. The durable deliverables are the design
(`docs/frame_rpg_spec.md` + the 12 locked decisions in `CLAUDE.md`), a **pure deterministic core**
expressed in language-portable constructs, and a **language-neutral golden-vector suite** that is the
behavioral contract. A port is correct when it reproduces the fixed-point reference table bit-for-bit
and replays every golden vector to a byte-identical trace.

This document is (A) the pure-core contract, (B) a durable-vs-disposable table, and (C) a porting
checklist.

---

## A. The pure-core contract

### A.1 The step function

The engine is conceptually a pure transition

```
step(state, decisions) → state'
```

with **no RNG, no clock, no ambient/global state, and no I/O**. Same inputs ⇒ byte-identical outputs,
forever, in any language (audit C-3). In this implementation the loop is `runMatch(initialState,
moveTables, agents, options) → { finalState, trace, winner }` (`src/core/engine.ts`); the agents are a
**synchronous, value-based** interface — the engine asks for an `Action` and gets one back
synchronously, never via a Promise/callback/await (decision 12):

```
interface Agent {
  chooseAction(view: PlayerView): Action;          // commit a move/movement/wait
  chooseCancel(view: CancelView): DecisionResult;  // at a cancel checkpoint: a move or DECLINE
}
```

Determinism rules a port MUST preserve:

- **No floats in gameplay.** Every positional/spatial quantity is 16.16 fixed-point (§A.2). Floats are
  permitted only for display (the CLI) and the budget linter's scoring (tooling) — never in the engine.
- **ID-based references, not object identity.** Entities and moves are referenced by stable
  integer/string IDs; no logic depends on pointer/GC identity.
- **Integers-only on the wire.** Serialized state and traces contain no floats; fixed-point serializes
  as its integer `raw` (§A.4).
- **All sum types are tagged unions** with a literal discriminant, matched exhaustively (§A.3).
- **The regime, and the whole neutral/pressure/punish loop, derive from `ready_tick` alone** (spec
  §2.1, audit C-2). The engine always asks the entity with the lower `ready_tick`; ties ⇒ NEUTRAL
  (both commit simultaneously and hidden); otherwise PRESSURE (the actor chooses with full info).

### A.2 Fixed-point (16.16) — the portability linchpin

A `Fixed` is a **signed integer `raw`**; the real value it denotes is `raw / 2^16` (= `raw / 65536`).
Keep world coordinates well within ±2³¹ so 64-bit intermediates stay exact. Reproduce these EXACTLY
(reference: `src/core/fixed.ts`, contract tests: `src/core/fixed.test.ts`):

| op | semantics | port note |
|---|---|---|
| `fromInt(n)` | `n * 65536` | exact |
| `add/sub` | integer add/sub of `raw` | exact |
| `mul(a,b)` | 64-bit product, **arithmetic right shift by 16** → **floors toward −∞** | i64 intermediate + signed `>>` (Rust/C#/C++ all floor on signed `>>`) |
| `div(a,b)` | `(a << 16) / b`, integer division → **truncates toward zero** | i64 intermediate + native integer division (truncates toward zero) |
| `fromRatio(num,den)` | same as `div` of two ints → **truncates toward zero** | |
| `toInt(f)` | `floor(raw / 2^16)` → **floors toward −∞** | `toInt(3.5)=3`, `toInt(-3.5)=-4` |
| `toIntRound(f)` | `floor((raw + 32768) / 2^16)` → **rounds half-up toward +∞** | used for damage scaling (so `100×0.9 → 90`, not 89); stays integer-only |
| `roundHalfUp(x)` | `floor(x + 0.5)` (ties toward +∞), on a plain number | the few stat→int conversions; integer-equivalent: `roundHalfUp(n/2) == floor((n+1)/2)` |

Constants that depend on this rounding (`src/core/config.ts`):
`CH_DAMAGE_MULT = fromRatio(5,4) = 81920` (×1.25, exact); `JUGGLE_DAMAGE_DECAY = fromRatio(9,10) =
58982` (≈0.89999 — note the truncation; `toIntRound` is what makes the compounded result land on the
intuitive integer).

**A port's first acceptance test is a translation of `fixed.test.ts`** (the hand-authored reference
tables for `mul`/`div`/`fromRatio` incl. negatives and rounding edges). If those pass, the spatial and
damage math will match.

### A.3 Tagged unions → native enums

Every sum type is a discriminated union with a literal `kind`/`tag`, matched by an exhaustive `switch`
with an `assertNever(x: never)` default (decision 11). Each maps 1:1 onto a Rust `enum` (matched with
no wildcard arm), a C# record-hierarchy / discriminated switch, or a C++ `std::variant`. The unions to
replicate (with their data):

- `EntityState` — NEUTRAL · STARTUP/ACTIVE/RECOVERY(move) · HITSTUN/BLOCKSTUN(until) · AIRBORNE(until,
  juggleCount) · DOWN(wakeupTick) · GUARDBROKEN(until)  (`src/core/entity.ts`)
- `Property` — INVULN(invulnType,window) · ARMOR(armorHits,armorDamageMult,window) ·
  COUNTER_HIT_STATE · GUARD_POINT · BLOCK(covers) · AIRBORNE · PROJECTILE_SPAWN  (`frameprofile.ts`)
- `ContactResult` — WHIFF · PARRIED · THROWN · THROW_TECH · BLOCKED · ARMORED · HIT(counter)
  (`src/core/resolver.ts`)
- `Action` (MOVE/WAIT), `DecisionResult` (Action | DECLINE), `Regime` (NEUTRAL | PRESSURE(actor)),
  `TraceEvent` (STATE/COMMIT/WAIT/CANCEL/DENIED/CONTACT/KO)  (`src/core/engine.ts`)
- Fieldless enums (string-literal unions here): `MoveLevel`, `InvulnType`, `AttackType`, `MovePhase`,
  `ApGate`, `CancelGate`, `Facing`.

### A.4 The golden-vector schema (the behavioral contract)

A golden vector (`golden/*.json`) is **self-contained** — a port replays it without the TS content.
Canonical JSON: **integers only** (no floats; fixed-point as `raw`), **recursively key-sorted**, LF
newlines, 2-space indent (`src/serialize/canonical.ts`).

```
GoldenVector {
  schemaVersion : int
  config        : the locked constants the trace depends on (documentation for the port)
  moveTables    : [ { moveId → FrameProfile }, { moveId → FrameProfile } ]   // both fighters, resolved
  initialState  : MatchState            // { t, entities:[Entity,Entity] }
  options       : { maxTicks, maxDecisions }
  decisions     : [ Decision[], Decision[] ]   // each fighter's recorded stream, in ask-order
  trace         : TraceEvent[]          // the expected tick-by-tick event + entity-state stream
}
Decision = { kind:"action", action:Action } | { kind:"cancel", result:DecisionResult }
```

**To verify a port:** for each vector, rebuild the move tables + initial state, replay `decisions`
through a ReplayAgent (returns the next recorded decision in order), run the match under `options`, and
assert the produced `trace` canonicalizes to the stored `trace` **byte-for-byte**. The frame-advantage
convention a port must reproduce: on a contact, the defender's stun begins at `startTick + startup +
active` and lasts `stun` ticks, so emergent advantage equals the I-1 quote (`stun − recovery`)
regardless of which active frame connected.

The shipped vectors: both worked examples (`reza-borin`, `sidestep-ap`); each defensive interaction
(`block`, `parry`, `throw`, `throw-tech`, `sidestep-linear`, `sidestep-homing`); each combo governor
terminating (`governor-ap`, `governor-focus`, `governor-juggle`, `governor-hitstun`); and a
`counter-hit` punish.

---

## B. Durable vs disposable

| Durable (the contract — reproduce exactly) | Disposable (rewrite freely) |
|---|---|
| The design: `docs/frame_rpg_spec.md` + the 12 locked decisions (`CLAUDE.md`) | All TypeScript source under `src/` |
| The fixed-point semantics (§A.2) incl. shift/rounding directions | The build tooling (tsconfig, eslint, dependency-cruiser, vitest, tsx) |
| The tagged-union shapes (§A.3) | The CLI fight-runner + timeline printer (`src/cli/`) |
| The module boundary map (spec App. A; `CLAUDE.md`) | The npm scripts and package layout |
| The golden vectors (`golden/*.json`) + the canonical encoding | The sample content (`src/content/`) — swap freely; the engine never hard-codes a move |
| The balance rules R-1..R-5 and audit checks C-1..C-11 (as properties to uphold) | The specific budget weights `w_*` (playtest-tuned) |

Within the TS code, the boundary that is **load-bearing** (not just style) is: stats + equipment are
*compilers that emit FrameProfiles*; the engine is an *interpreter that runs them*. Only one file
(`rpg/compiler.ts`) bridges the RPG layer into the engine. A port must keep that one-way bridge.

---

## C. Porting checklist

1. **Fixed-point first.** Implement `Fixed` to the §A.2 semantics (i64 intermediates; `mul` floors via
   arithmetic `>>`; `div`/`fromRatio` truncate toward zero; `toInt` floors; `toIntRound` half-up;
   `roundHalfUp` for stat→int). Translate `fixed.test.ts` and pass every row, **including the negatives
   and rounding edges** — this is the single thing most likely to silently diverge.
2. **Replicate each tagged union (§A.3) as a native enum**, matched exhaustively (no wildcard arm), so
   adding a variant is a compile error until every site handles it (the role of `assertNever`).
3. **Reconstruct the module boundaries** (spec App. A) as the target's visibility system — Rust crates
   + `pub`, C# assemblies + `internal`, C++ targets. Keep: only `rpg/compiler` bridges L4→engine; the
   engine imports no content; all contact math lives behind one `doesHit` predicate; the core is pure
   and synchronous.
4. **Port the engine** as a pure `step`/`runMatch` over plain data: regime off `ready_tick`; the
   resolution loop (advance tick-by-tick, apply contacts on active frames via `doesHit`, pause at
   cancel checkpoints and decision points); `classifyContact` priority (invuln > parry > block > armor
   > hit; throws resolve separately and beat parry/block/armor); the AP economy (charge on commit/
   cancel, refill on entering NEUTRAL, conditional `ap_gain` on contact) and the four combo governors;
   §2.10 (hidden neutral commit; no startup cancels unless flagged).
5. **Consume the golden vectors.** Implement the canonical integers-only JSON reader/writer (§A.4) and
   the replay harness, and require **identical output** on every vector. Re-`emit` is only legitimate
   when you intend a behavior change; otherwise a mismatch is a porting bug.
6. **Uphold the balance properties.** Port the R-1..R-5 linter and the C-1..C-11 audit (or re-run the
   spirit of them) over your content so "balanced" stays a checkable property, not a hope.

When steps 1 and 5 both pass, the port is behaviorally equivalent to this reference implementation.
