# TICK — Implementation Plan
### Phased build to the vertical slice

> **⛔ STATUS: AWAITING SIGN-OFF.** Per the working agreement (2026-06-09): these docs come
> first, the user reviews them, and **implementation does not begin until the user signs off.**
> After sign-off, Claude implements with the user monitoring; every phase ends at a reviewable,
> testable gate. Do not scaffold `rust/` before that approval.

**Target:** the vertical slice (vision §6) — one island chain, a party of 3, one dungeon + boss,
Heat/Rage/supers working, walls + one hazard arena, the loot loop closed, the Fog on the map.

**Build order rationale:** headless-first (the engine is the product and is fully testable
without pixels); fog/observation before any UI exists to consume it (so nothing is ever built
against leaked state); party machinery before presentation (the camera problem is only solvable
against the real thing); exploration after combat is fun in tests; content and tuning last,
against the audit.

---

## Phase 0 — Workspace & determinism bedrock
**Goal:** an empty engine that can never drift.
- Cargo workspace (`engine`, `app` stub), edition 2024; justfile; CI (fmt, clippy, test).
- `core::fx`: the fixed-point type choice + `FxVec2` as **thin glue over `fixed` + `cordic`**
  (library policy, tech-plan §1.1 — evaluate `cordic` vs `fixed-sqrt` here, adopt whichever
  passes the determinism gate; we hand-roll no numeric algorithms). Glue unit-tested against
  reference values and **frozen** (tech-plan §7).
- Trace event scaffolding + serde; replay-twice determinism test wired into CI from day one.
- **Exit:** CI green; a trivial sim (two entities, WAIT) replays byte-identically.

## Phase 1 — The duel core (headless 1v1)
**Goal:** a complete, honest 1v1 fight in tests.
- `data/` schema (Move, HitEvent, Reaction, PropertyWindow, Ruleset, ArenaDef).
- Scheduler (`ready_tick`, Ready/Cancel decisions, side-blind same-tick commits), move phases,
  the contact priority table, heights/stances (Tekken logic), guard + chip + guard-break,
  throws **with the directional break reaction window**, parry/guard-point, CH with
  `ch_reaction`, target-lane spatial math + `does_hit` (the 1v1 case), movement moves.
- Scripted-decision test agents; sim tests for every §5 interaction.
- **Exit:** the spec §14 worked example's *1v1 beats* (sidestep-whiff, CH whiff-punish, throw
  break both ways) run as green tests off authored test content.

## Phase 2 — The combo system & meters
**Goal:** combos exist and **provably end**.
- The Reaction union in full: Launch/juggle, Crumple, Screw, Bound, knockdowns; walls +
  WALL_SPLAT; okizeme + wake-up options; cancel windows/gates, lock-then-confirm, strings.
- Meters: Breath, Guard, AP (string economy), Focus (gain table); DENIED handling.
- **All seven governors** + `proptest` property suites: fuzzed agents can never exceed K combo hits.
- First cut of `content::audit`: I-1, R-5 cycle scan (`petgraph`), juggle-termination proof.
- **Exit:** governor property tests + audit green over test content; a scripted juggle →
  screw → wall splat → bound → ender trace reads exactly as spec §6.2.

## Phase 3 — The fog (Observation, cues, knowledge, forecast)
**Goal:** information becomes a first-class system — before any UI exists.
- `observe.rs`: the Observation type (physical state, state class, cues + phase tags, HP-only
  meters w/ visibility flags, public event log). Sim state goes module-private.
- CueClass authoring on moves; knowledge tiers T0–T3 gating Observation enrichment;
  break-key reveal at T3.
- **The forecast as projection-replay** (tech-plan §3): forecast == engine run on
  Observation-only state, tested for equality and for non-leakage.
- `agents.rs`: baseline read-profiles (aggressive/turtle/gambler/step-happy) consuming
  Observation only; headless AI-vs-AI fights complete and terminate.
- **Exit:** a "fog honesty" test suite: no agent or forecast can distinguish two true states
  that project to the same Observation.

## Phase 4 — Party combat
**Goal:** N-per-side fights that terminate and make geometric sense.
- Multi-actor scheduling; targeting + retargeting + `switch_focus`; facing-relative guard arcs,
  back hits; pass-through bodies; multi-victim `does_hit` (sweeps/beams clip bystanders);
  friendly-fire flags.
- KO/revive (UTILITY moves), side elimination, full-wipe loss.
- Burst (reaction window + latch) and RESCUE-gated moves; tests proving emergent ally
  interruption (CH on the comboer) needs no special case.
- **Exit:** scripted 2v2 reproducing spec §14 end-to-end as one green test; AI 3v3 fuzz runs
  terminate within tick bounds.

## Phase 5 — Escalation: Heat, Rage, supers, projectiles
**Goal:** the anime layer, lawfully.
- Heat (burst action + engager hits, compiled Heat variants, duration latch), Rage + Rage Art,
  EX moves, supers, missiles (spawn/clash/lifetime) and beam envelopes; arena hazards firing
  authored events.
- Budget weights extended (w_arc, w_track, w_meter, w_lie); audit covers the new axes.
- **Exit:** an AI fight trace shows the escalation arc (early pokes → Heat mid-fight → super
  ender); all governors still hold under supers (property tests re-run green).

## Phase 6 — The compiler & real content
**Goal:** Builds become Fighters; the slice's cast exists.
- `content::compile`: attributes (six levers) × Form ranks × equipment × affix riders →
  MoveList + DefenseProfile + Heat variants; requirement floors (R-3).
- Slice content: **4 Forms** (player Forms incl. First Form + enemy styles), 3 party characters,
  ~6 enemy types + 1 boss kit, weapons across the R-4 triangle, a starter affix pool, Rulesets.
- Full audit goes green and **golden vectors v2** are frozen under `golden/`.
- **Exit:** audit + vectors in CI; headless campaign-fight playlist runs deterministically.

## Phase 7 — The Bevy combat experience
**Goal:** the fight you can see and feel.
- `app` states per fsm.md; the combat driver pump; commit menus + reaction prompts;
  **the timeline ribbon** (committed phases, fog-shaded cue blocks, knowledge-gated overlays);
  forecast painting (envelope decals + ghost ribbon); lane-cut camera + tactical toggle.
- Sprite staging on the 3D plane; first VFX pass (impact frames, speed lines, hit sparks,
  Heat/Rage auras, super cut-in placeholder).
- **Exit:** a human can play a full 3v3 (incl. boss kit) start to finish; a fog-honesty UI
  review confirms nothing on screen exceeds Observation.

## Phase 8 — The voyage
**Goal:** the world around the fights.
- Exploration sim: seeded hex worldgen (islands/shallow/deep fog/anomalies), stability + Fog
  border (authored-moment moves), travel + Anchor, token drift + engage, soft-loss flow.
- Map render + sailing feel; hex → ArenaDef authoring; ports/POI scene framework
  (shop/trainer/master interactions as menus first).
- **Exit:** sail → spot token → fight → loot → port loop playable; Fog border visibly advances
  on a scripted beat.

## Phase 9 — Progression & the slice content pass
**Goal:** the RPG closes its loops.
- Loot generation (base × rarity × affix) + loadout deck UI + codex UI (knowledge tiers
  surfaced); trainers/masters (attribute training, Form teaching, a rank trial boss);
  companions' builds; the hidden Tithe accumulator (story-flag gated reveal).
- Slice assembly: the island chain, one dungeon (authored layout, chests, boss, guaranteed
  drop), intro narrative beats.
- **Exit:** **the vertical slice** — a new player reaches the boss through real loops
  (fight → loot → train → re-spec → delve) in one sitting.

## Phase 10 — Tuning, polish, hardening
**Goal:** the slice is *good*, and provably still lawful.
- Tuning tables pass (spec §15) against playtests; AI read-profile balancing; VFX/sound-hook
  polish; performance pass (should be trivial — integer sim); save/load; docs refreshed to
  match reality; golden vectors re-frozen if semantics moved (deliberately, with a changelog).
- **Exit:** sign-off review against the vision doc's six pillars, each demonstrated live.

---

## Standing rules for every phase

1. **The audit and the property tests are merge gates** from the phase that introduces them.
2. **No combat fact is computed in `app`** — driver review rule (tech-plan §4).
3. **Any new mechanic names its governor** before it merges (spec §6.5).
4. **Schema changes after Phase 6 re-freeze the golden vectors** with an explicit changelog
   entry — silent semantic drift is the one unforgivable bug in a deterministic engine.
5. Phase order within 7–9 may interleave for sanity (e.g. minimal map render before the full
   combat UI polish) — gates, not dates, are the contract.
