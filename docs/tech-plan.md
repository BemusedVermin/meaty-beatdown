# TICK — Technical Plan
### Architecture for the Rust + Bevy rebuild

*What we're building and the boundaries that keep it buildable. The phasing lives in
[`implementation-plan.md`](./implementation-plan.md). The deleted v1 workspace (git `0e2eaae`) is
reference material only — this is a fresh build to the v2 docs, not a restoration.*

---

## 1. Shape of the workspace

Two crates, one dependency arrow, same discipline as v1 (it worked):

```
rust/                      # cargo workspace, edition 2024 (Rust ≥ 1.85)
  engine/                  # deterministic core — NO Bevy, no floats, no wall clock
  app/                     # the Bevy game shell — drives the engine, owns all presentation
```

- **`app → engine`, never the reverse.** The engine compiles headless and runs full fights and
  full voyages in tests with zero graphics.

### 1.1 Library policy ✅ (user-directed, binding)

**We use external libraries wherever a well-maintained crate fits; bespoke code is reserved for
the domain itself.** The combat rules, the content, and the sim semantics are the product — we
write those. Math, algorithms, parsing, testing infrastructure — we do not rewrite what the
ecosystem already provides. Thin glue (newtypes, trait impls, adapters) over a crate is fine
and expected; reimplementing a crate's job is a review-blocking smell. Each adoption is gated
on one question at the phase where it lands: *does it pass our determinism tests?*

| Need | Crate | Note |
|---|---|---|
| Fixed-point arithmetic | **`fixed`** | the Q-format types (`I32F32` etc.); we do not write our own number type |
| Fixed-point sqrt / trig (arcs, projections, facing) | **`cordic`** (fallback: `fixed-sqrt`) | works over `fixed` types; replaces the previously-planned hand-rolled sqrt — evaluate at Phase 0, freeze whichever passes the determinism gate |
| Hex coordinates | **`hexx`** | proven in the v1 worldgen |
| Seeded RNG | **`fastrand`** | small, deterministic, proven in v1 |
| Serialization (traces, saves, golden vectors, content files) | **`serde` + `serde_json` / `ron`** | RON for authored content when it goes file-based (§5) |
| Graph algorithms (the audit's cancel-graph cycle scans, R-5/R-6) | **`petgraph`** | cycle detection / SCCs are solved problems; don't hand-roll them |
| Property testing (the anti-infinite fuzz suites) | **`proptest`** | shrinking fuzz cases beats a bespoke loop |
| Trace snapshot tests (pre-golden-vector) | **`insta`** (candidate) | adopt if it earns its keep at Phase 1 |

- **`engine` deps therefore:** `fixed`, `cordic`, `hexx`, `fastrand`, `serde` (+`ron` later),
  `petgraph` (audit), with `proptest`/`insta` as dev-deps. No `bevy_math` in the engine — not
  to avoid a library, but because mixing `f32` math into a fixed-point determinism contract is
  the one place a library would *cost* correctness (v1 traded determinism for `bevy_math`'s
  f32 AABBs and we flagged it then; v2 doesn't repeat that trade).
- **`app` deps:** `bevy` (latest stable when the Bevy phase starts — was 0.18 in the deleted
  shell; re-evaluate then), plus the engine. Same policy applies on the app side: prefer
  Bevy's built-ins and the ecosystem (e.g. an input-manager crate, `bevy_egui` for debug
  panels, a tweening crate for VFX timing) over bespoke equivalents — candidates evaluated at
  Phase 7, each through the same "does it earn its keep" gate.

## 2. The determinism charter (C-DET, enforced by construction)

1. **No floats in the engine.** Positions, arcs, and damage math use `Fx` (a chosen
   `fixed` type, e.g. `I32F32`) and `FxVec2` — where `core::fx` is **thin glue only**: a vec2
   newtype whose arithmetic delegates to `fixed` and whose sqrt/trig delegate to `cordic`
   (library policy, §1.1). We implement no numeric algorithms ourselves. The fixed-point
   format is part of the behavioral contract (as v1's PORTING.md established) — document it
   once, never change it casually.
2. **No wall clock, no ambient RNG.** Time is the tick; randomness only via a seeded generator
   stored in sim state (combat needs almost none; worldgen/loot consume seeds).
3. **A fight is a pure function** of (initial state, content + Ruleset, decision log). Same for
   a voyage given (seed, decision log). Replays, tests, and golden vectors all fall out of this.
4. **Traces are the contract.** Every commit, contact, reaction, meter change, and state
   transition is a serde-serializable tagged event. **Golden vectors v2** = frozen traces of
   canonical scenarios, regenerated from this engine once combat stabilizes (the v1 vectors are
   dead — different semantics).

## 3. Engine architecture

```
engine/src/
  core/         ids, tick, fx (Fx, FxVec2 — glue over `fixed` + `cordic`), seeded rng
  data/         the content SCHEMA: Move, HitEvent, Reaction, PropertyWindow, CancelWindow,
                CueClass, Ruleset, ArenaDef, DefenseProfile, meter defs
                — pure types, no behavior; the language both engine and content speak
  combat/
    entity.rs   Entity, ComboTracker, meters, latches
    schedule.rs decision queue: Ready/Cancel/Reaction/Wake-up; side-blind commit collection
    spatial.rs  target-lane math, does_hit, motion integration, walls & hazards
    resolve.rs  the contact priority table, reactions, CH, the seven governors
    observe.rs  ★ the Observation API — the ONLY read path out of a live fight
    agents.rs   AI: read-profiles consuming Observation (so headless fights run end-to-end)
    sim.rs      CombatSim: pump_decisions / commit / step / trace
  content/      authored Forms, moves, enemies, arenas, Rulesets (Rust modules first, §5)
                + compile.rs (Build → Fighter) + audit.rs (budget, R-rules, cue collisions,
                cancel-graph cycles, juggle-termination proof)
  exploration/  hex, worldgen (seeded), fog/stability, travel, encounter tokens, loot gen
```

**The two load-bearing boundaries:**

- **The compiler bridge** (`content::compile`) is the only path from RPG data to runtime
  fighters; the engine never sees a stat (spec §12, audit C-5).
- **The fog boundary** (`combat::observe`). `CombatSim`'s true state is module-private;
  the public surface is `observe(side) -> Observation` + the decision/commit API + the trace.
  The UI **and** `agents.rs` consume `Observation` only — leaking intent becomes a compile
  error, not a code-review hope (spec C-FOG).

**The forecast without leaks:** the prediction UI (spec §7.4) is implemented by running the
*real* sim on a **projection state built only from `Observation`** (unknowns held at
last-observed values). One simulation codebase, zero reimplementation drift, and the forecast
physically cannot use hidden facts because the projection type doesn't contain them.

## 4. App architecture

```
app/src/
  state/        AppState, PauseState, GameState, ExplorationState, DungeonState, CombatState
                — exactly the fsm.md machines, as Bevy States/SubStates/components
  combat/       the driver: decision pump ↔ CombatSim; commit menus; reaction prompts;
                the TIMELINE RIBBON (signature UI); forecast painting (envelope decals,
                ghost ribbon); camera rig
  exploration/  hex-map render, sailing, token drift, POI scenes, loot/reward beats
  menus/        loadout (deck building), codex, inventory, character sheet
  render/       sprite staging, anime VFX (impact frames, smears, speed lines, cut-ins), auras
  debuglog/     trace sink to file (the v1 pattern, kept)
```

- **Camera & staging:** a 3D scene with billboarded 2D sprites on the ground plane. Default
  framing is **side-on to the active lane** (the deciding actor → its target) — Tekken's camera
  grammar; the camera *cuts* between lanes at decision points (the anime tournament-cut), with a
  top-down tactical toggle for party positioning. Engine `FxVec2` converts to `f32` only here,
  at the render boundary.
- **The driver is a pump, not a brain:** Bevy systems move data between `CombatSim` and
  UI/animation. All rules live in the engine; the app may not compute a single combat fact
  (review rule).

## 5. Content & data strategy

- **Phase one: content as Rust modules** (typed constants in `engine/content/`) — refactor-safe,
  audit-checked in CI, zero parsing. **Later: serde/RON files** with the same schema once the
  schema stops moving (the `data/` types are already serde-ready).
- IDs are interned newtypes; every magnitude lives in data per C-AUTH (the Ruleset object
  carries the cross-cutting curves).
- **Saves:** serde snapshots of (world seed + exploration state + builds + codex + story flags).
  Mid-combat suspend is deferred — fights are short; save on the map ⚠️.

## 6. Testing strategy (the engine is the product; test it like one)

| Layer | What | Gate |
|---|---|---|
| Sim tests | one test per mechanic (each spec § gets its scenario: sidestep-whiffs-linear, throw-break, wall-splat-latch, burst, rescue CH, Heat variants…) | every PR |
| Property tests | `proptest`-driven fuzz agents picking random affordable actions; assert invariants: **no combo exceeds K hits**, meters never negative, no deadlock (a decision or KO is always reachable), I-1 advantage consistency | every PR |
| The audit | `content::audit` over all shipped content: budget residuals, R-1…R-7, cancel-graph cycle scan (`petgraph`), cue-collision report | every PR |
| Determinism | replay a recorded decision log twice → byte-identical traces; run on two OSes in CI | every PR |
| Golden vectors v2 | canonical scenario traces frozen under `golden/` once combat stabilizes (phase 6) | regression wall from then on |

## 7. Risks & watch-items

| Risk | Mitigation |
|---|---|
| **Party-fight readability** (N lanes, one screen) | decision-point camera cuts + timeline ribbon as the source of truth + tactical toggle; playtest at N=2 before 3 |
| **Forecast leaking hidden state** | the projection-from-Observation type (§3) makes leaks unrepresentable; tests assert forecast equals replay-on-projection |
| **AP/tempo tuning across party scale** | all curves in the Ruleset (data) — retune without rebuilds; sim-fuzz win-rate dashboards early |
| **Content volume** (Forms × moves × cues × Heat variants) | the budget identity makes moves *derivable from intent*; audit keeps additions lawful; vertical slice caps at ~4 Forms |
| **Camera/staging complexity** | the lane-cut grammar is simple; resist free-camera scope creep until the slice ships |
| **Bevy version churn** | engine is Bevy-free; the app's surface area with Bevy is plugins + sprites + UI, deliberately conventional |
| **Fixed-point edge cases** (sqrt, normalization) | delegate the math to `fixed` + `cordic` (§1.1) and unit-test the thin `fx` glue against reference values; freeze early; the golden vectors then lock it |
