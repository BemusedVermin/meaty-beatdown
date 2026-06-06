# Fighting-Game Mechanics Catalog & Gap Analysis

*A cross-genre catalog of fighting-game mechanics, each marked with TICK's coverage — and a
prioritized read on what's worth adding.*

This is the companion to [`mechanics.md`](./mechanics.md) (which describes what TICK **does**). Here we
zoom out to the **whole genre** — traditional 2D, anime/air-dashers, 3D, platform fighters, NRS, and
tag games — and ask: of everything fighting games do, what does TICK have, what is it missing, and
which gaps are *worth* closing.

## How to read this — the turn-based translation principle

TICK is **headless, deterministic, and turn-based**: no real-time clock, no real-time input. The engine
advances ticks and *pauses* for decisions. The spec's own framing (spec §B.2) is that **execution
becomes reading + sequencing** — you express skill by *what* you commit and *when in the tick stream*,
not by how fast or precise your hands are.

That means a large class of fighting-game mechanics is **deliberately translated away**, not missing:
anything whose entire purpose is a real-time execution barrier (just-frames, plinking, kara-cancel
timing, input buffering, negative edge, motion vs. charge inputs, Korean backdash, wavedash) has **no
analog and needs none**. Marking those "missing" would be a category error. We tag them 🚫 and explain.

The genuinely interesting question is the **systems** layer: meter, comeback, defensive resources,
corner play, air game, oki — mechanics that add *decisions*, not *dexterity*. Those port cleanly into a
turn-based model (YOMI Hustle proves the genre works turn-based) and are where the real gaps live.

### Coverage legend

| Mark | Meaning |
|---|---|
| ✅ | **Implemented** — the engine enforces it today. |
| 🟡 | **Partial / expressible** — primitives exist (can be authored as data) but there's no explicit system or sample for it. |
| 🟦 | **Deferred** — a conscious stub with a data slot kept (decision 8). |
| ⬜ | **Absent** — a genuine gap; could be added. |
| 🚫 | **N/A by design** — real-time execution tech or out-of-scope genre; the turn-based model replaces or excludes it. |

---

## Part A — Universal / cross-cutting systems (the checklist)

These are the primitives *every* fighter has in some form — the most useful gap checklist.

### A.1 Frame data & states

| Mechanic | TICK | Note |
|---|---|---|
| Startup / active / recovery | ✅ | `Timing`; the substrate of everything. |
| Frame advantage (+/−), plus frames | ✅ | **Derived** via invariant I-1, never stored. |
| Hitstun / blockstun | ✅ | Drive on_hit / on_block. |
| Hitlag / hitstop (freeze on contact) | 🚫 | No real-time freeze needed; the engine pauses at decision checkpoints and hit-confirms via cancel gates. |
| Pushback on hit | ✅ | `hitEffect.knockback`. |
| Pushback on block | ⬜ | Knockback applies on hit only; blocked strikes don't reposition. |
| Counter-hit | ✅ | ×1.25 dmg, +6 hitstun; CH state = own startup/recovery. |
| Punish counter / lethal-hit (extra-reward tier) | 🟡 | CH already fires on recovery (so whiff-punishes are CH); no *distinct* punish-counter reward tier. |

### A.2 Defense & meter-on-block

| Mechanic | TICK | Note |
|---|---|---|
| Blocking, high/low block | ✅ | `BLOCK{covers}` vs. `MoveLevel`; wrong guess = clean hit. |
| Chip / block damage | ✅ | Chip goes to **Poise**, not HP. Chip-to-HP (and chip-kill) ⬜. |
| Guard meter / guard crush / guard break | ✅ | Poise → 0 → **GUARDBROKEN** (40-tick punishable stun). |
| Stun / dizzy (from accumulated hits) | ⬜ | Guard-break exists; there's no *hit-based* dizzy meter. |
| Parry (timed deflect) | ✅ | `GUARD_POINT`; freezes attacker 30t, refunds Focus + AP. |
| Just-frame / instant block (reduced blockstun) | ⬜ | No reduced-blockstun option. |
| Pushblock / advancing guard | ⬜ | No defender-initiated spacing reset. |
| Faultless / barrier defense (no chip, more pushback) | ⬜ | — |
| Just-defend / flawless block (heal/advantage on tight block) | ⬜ | Parry is the only timing-reward defense. |
| Reversal (invuln-startup get-off-me) | 🟡 | Expressible as any move with an `INVULN` startup window; no sample reversal ships. |
| Throw tech / break | ✅ | Same-tick throw clash → `THROW_TECH`. |
| Opposed throw-escape contest (the spec's one dice use, §4.1) | ⬜ | Not implemented; throw-tech is purely same-tick. |

### A.3 Combos & scaling

| Mechanic | TICK | Note |
|---|---|---|
| Combo termination (anti-infinite) | ✅✅ | **Four** independent governors (Focus, juggle decay, hitstun decay, AP). |
| Juggle damage / gravity scaling | ✅ | ×0.9 per juggle hit. |
| Hitstun decay | ✅ | `−2 ticks` per chained hit, floored at 1 → advantage goes minus. |
| Damage scaling / proration (grounded combos) | 🟡 | Only hitstun-decay caps grounded strings; no per-hit *damage* proration off the ground. |
| Juggle systems / air-hit limits / juggle points | 🟡 | `juggleCount` tracked + decay; no explicit hit-count cap or juggle-point budget. |
| Multi-hit moves | ⬜ | `MoveInstance` is single-hit by design (out of scope today). |
| Juggle-extender states (bound / screw / tailspin / ground-bounce / wall-bounce) | ⬜ | Only `launches`/`knockdown`; no re-floor/re-spin states. |
| Special hitstun states (crumple / stagger / wall-stick) | ⬜ | — |
| Reset (drop combo to re-mixup) | 🟡 | Emergent — you can stop and re-pressure; no special reward/mechanic. |

### A.4 Hitboxes, invul & priority

| Mechanic | TICK | Note |
|---|---|---|
| Hitbox / hurtbox / collision | ✅ | Abstracted to lane range + height band (not pixel boxes), behind one `doesHit`. |
| Invincibility / i-frames (typed) | ✅ | `INVULN` = ALL / STRIKE / THROW / PROJECTILE. |
| Armor / hyper armor | ✅ | `ARMOR{hits, damageMult}`; throws beat it. |
| Guard-point / sabaki | ✅ | `GUARD_POINT` → PARRIED. |
| Priority / clash / trades | 🟡 | Simultaneous active hits can both resolve (a trade); no clash-cancel or disjoint-priority system. |
| Projectiles / fireballs / zoning | 🟦 | **Deferred** — `PROJECTILE_SPAWN` slot kept; stub throws. |
| Projectile counterplay (reflect / absorb / race) | 🟦 | Follows from projectiles being deferred. |

### A.5 Cancels

| Mechanic | TICK | Note |
|---|---|---|
| Special cancel (normal → special) | ✅ | `CancelWindow{gate, into, cost}`. |
| Gatling / chain (normal → normal) | ✅ | e.g. `light_jab → light_slash`. |
| Whiff cancel | ✅ | `ON_WHIFF` gate exists. |
| Hit-confirm (cancel only if it connected) | ✅ | Lock-then-confirm via `ON_HIT`/`ON_BLOCK` gate + `MoveContact`. |
| Jump cancel / dash cancel | 🟡 | Expressible (cancel into a movement move) but no jump/dash content authored. |
| Super cancel | ⬜ | No supers to cancel into. |
| Universal meter-cancel (Roman / Focus / Drive / Veil-Off) | ⬜ | No "spend meter to cancel anything" system. |
| Kara cancel (first-frame range extension) | 🚫 | A real-time range-shift execution trick; no analog. |

### A.6 Inputs & buffering — *all 🚫 by design*

Motion vs. charge inputs, input buffer, negative edge, plinking/piano, just-frame inputs, option
selects, simplified/modern controls. **🚫** — TICK takes an `Action` *value* from an agent; there is no
input-interpretation layer, so this entire category is translated away (it's the core of "execution →
reading"). The strategic *content* these enable (frame traps, shimmies, confirms) is preserved through
frame advantage, CH, and the confirm gate; the *dexterity* is not.

### A.7 Movement

| Mechanic | TICK | Note |
|---|---|---|
| Backdash (with i-frames) | ✅ | Sample `backdash`, STRIKE-invuln 0–4. |
| Sidestep | ✅ | `sidestep_l/r`; dodges LINEAR, loses to HOMING. |
| Sidewalk (continuous) | 🟡 | In the spec's movement table; only the hop (`sidestep`) is authored. |
| Step / dash / run / hop (forward) | 🟡 | Spec'd as movement profiles; not authored as content. Movement is just a move with a profile, so this is content, not engine. |
| Air dash / double jump / high jump | ⬜ | No air movement. |
| Cross-up / side-switch (left/right mixup) | ⬜ | Fighters auto-face; no side switching. |
| Wavedash / dash-dance / KBD / backdash-cancel | 🚫 | Execution-movement tech; no analog. |

### A.8 Okizeme & neutral

| Mechanic | TICK | Note |
|---|---|---|
| Neutral / footsies / spacing / whiff-punish | ✅ | The core loop — NEUTRAL regime + spatial whiff. |
| Okizeme / wake-up pressure | ✅ | `DOWN{wakeupTick}` + PRESSURE regime gives the attacker oki. |
| Defender wake-up options (quick-rise / back-rise / roll / wake-up reversal choice) | ⬜ | Oki is currently **one-sided**: the downed player just rises on a fixed tick. |
| Meaty / safe-jump | 🟡 | Meaty timing is expressible (spacing + tick alignment); no safe-jump (no real-time) and no automation. |
| Tech / quick-rise / no-tech (knockdown timing choice) | ⬜ | — |

### A.9 Mixups (the offense RPS)

| Mechanic | TICK | Note |
|---|---|---|
| Strike / throw | ✅ | Block beats strike, loses to throw; throws beat block. |
| High / low (overhead vs. low) | ✅ | `MoveLevel` LOW/OVERHEAD + `BLOCK{covers}`. |
| Frame trap / shimmy / plus-frame pressure | 🟡 | Emerges from frame advantage + CH + AP strings; no shimmy *automation* (and none needed). |
| Left / right (cross-up, ambiguous, side-switch) | ⬜ | No side switching (A.7). |
| Vortex / true blockstring vs. gap | 🟡 | Blockstrings via cancels/advantage; setplay is emergent. |
| Option select | 🚫 | No input layer to fold options into. |

### A.10 Meter, comeback & defensive resources (cross-genre)

| Mechanic | TICK | Note |
|---|---|---|
| "Earned offense" resource | ✅ | **Focus** (gained on parry/CH/whiff-punish, spent on specials/cancels). The closest thing to a meter. |
| Tempo / action-economy resource | ✅ | **AP** — unique to TICK; the per-turn chain budget. |
| Super meter / supers / EX moves | ⬜ | No super tier. |
| Install / power-up (V-Trigger, Heat, Soul Charge, MAX, Sparking) | ⬜ | No temporary buff state. |
| Comeback mechanic (X-Factor, Rage, Fatal Blow, Pandora, Burnout swing) | ⬜ | No HP-threshold or losing-player swing tool. |
| Burst / combo breaker (defensive meter spend) | ⬜ | No defender-initiated combo escape. |
| DI / SDI / air-dodge (launch-survival) | 🚫 | Platform-fighter / real-time; N/A. |

---

## Part B — Sub-genre signature systems (and whether they'd fit)

The flashy, identity-defining systems per sub-genre. None are implemented; the verdict column says how
cleanly each would port into TICK's turn-based, deterministic, integers-only model.

| System (game) | What it is | Fit verdict for TICK |
|---|---|---|
| **Super / EX meter** (SF, KOF) | Gauge spent on enhanced specials / cinematic supers | **Strong** — add a meter resource + super-class moves; Focus is half of this already. |
| **Roman Cancel / FADC / Drive Rush** (GG, SF) | Spend meter to cancel *any* move's recovery, refresh advantage, reposition | **Strong but careful** — a universal meter-cancel is a powerful combo/pressure tool; must respect R-5 and the four governors so it doesn't create infinites. New golden vectors required. |
| **Burst / Psych Burst** (GG, BB) | Invulnerable combo-breaker the *defender* spends to escape | **Strong & wanted** — directly answers the spec's own #1 fun-risk (oppressive pressure, §B.3.2). Pairs with wake-up options. |
| **Heat / Rage / V-Trigger / Sparking** (T8, Tekken, SFV, DBFZ) | Temporary install: swap to buffed frame data for a window or under low HP | **Strong** — it's literally "swap the resolved FrameProfile for a while," which is exactly what the compiler already produces. Great drama; low conceptual friction. |
| **Fatal Blow / X-Factor / Pandora** (MK, MvC, SFxT) | One-shot comeback move/buff gated on low HP | **Good** — an HP-threshold-gated special; clean to express. |
| **Drive system / Burnout** (SF6) | One meter funds armor (Drive Impact), rush, parry, reversal; empty = penalty state | **Good (as a bundle)** — Drive Impact = armor (have it), Drive Parry = parry (have it), Burnout = a GUARDBROKEN-like penalty. A unifying meter could tie them together. |
| **Guard meter / guard crush** (SF Alpha, SC) | Block gauge that breaks under pressure | **✅ already have it** (Poise → GUARDBROKEN). |
| **Guard Impact / DoA hold / Just-Defend** | Timed/directional deflect that beats strikes | **Medium** — overlaps parry; a *leveled/directional* deflect (high/mid/low) would add a read layer. |
| **Wall / corner / wall-splat / ring-out** (Tekken, SF, SC, Smash) | Stage geometry that extends combos and concentrates pressure | **Strong** — bound the lane; knockback into the wall → a `WALL_SPLAT` state + combo extension + corner pressure. Big classic-FG depth; the lane is currently unbounded. |
| **Air game** (jump, air-dash, air-block, jump-ins, cross-up) | The whole vertical axis | **Medium-heavy** — `AIRBORNE` exists for juggles only; a real air game adds a jump action, air actions, air block, and the cross-up mixup. YOMI Hustle shows it's doable turn-based; it's the biggest single addition. |
| **Tag / assist / DHC / snapback / team super** (MvC, DBFZ, KOF) | Multi-character teams | **🚫 out of scope** — TICK is 1v1; this is a different game mode entirely. |
| **Percent damage / DI / ledge / recovery / edgeguard** (Smash) | Platform-fighter KO model | **🚫 different genre** — incompatible with the HP/lane model unless pivoting. |
| **Reversal Edge / Danger Time / Clash minigame** (SC, GG) | Built-in slow-mo RPS exchange | **Medium** — a discrete RPS event fits a turn-based engine naturally, but it's a flavor layer, not a core need. |

---

## Part C — Prioritized recommendations

Filtered through the **durable-deliverable constraints** (pure & deterministic, integers-only on the
wire, no float/RNG in gameplay, tagged-union shapes, must port cleanly). Each item below satisfies them:
nearly all are *new data shapes / states / resources*, not new physics. **Any addition needs new golden
vectors and must not break R-5 or the four governors.**

### Tier 1 — high value, strong fit, addresses the spec's own risk notes

1. **Defender wake-up options.** Give the `DOWN` player a small decision menu (quick-rise / back-rise /
   delayed / wake-up reversal). Today oki is one-sided — the spec flags "information asymmetry feel-bad"
   in PRESSURE (§B.3.2) and this is the standard answer. *Adds: a decision point on rise; a couple of
   `DOWN`-state variants.*
2. **A burst / combo-breaker defensive resource.** A meter the *defender* spends to interrupt a combo
   with an invuln shockwave. This is the genre's canonical answer to "pressure feels oppressive" — the
   spec's #1 fun-risk. Pairs naturally with #1. *Adds: one resource + one defender-initiated checkpoint
   during hitstun.*
3. **Wall / corner.** Bound the lane; knockback into the boundary produces a `WALL_SPLAT` (a
   juggle-extender state) and concentrates pressure. Corner play is a foundational FG axis TICK lacks
   entirely. *Adds: lane bounds + a hit-state + combo-extension rules.*
4. **A super + comeback layer.** A super meter (or reuse/extend Focus) funding super-class moves, plus
   an HP-threshold **install** (Rage/Heat-style temporary FrameProfile swap). Installs are trivial
   conceptually — the compiler *already* emits resolved profiles, so "use the buffed profile for N
   ticks" is a small step. Big payoff and drama. *Adds: a meter + super moves + a time-boxed profile
   swap.*

### Tier 2 — good fit, more systemic

5. **Costed defensive block variants** (pushblock / faultless / instant block). Deepen blocking beyond
   "turtle until guard-break": spend a resource to cut blockstun, negate chip, or reset spacing. Adds a
   real defensive RPS layer on block. *Also add: block pushback (A.1).* 
6. **Richer combo states**: multi-hit moves + juggle-extender hit states (ground-bounce / wall-bounce /
   crumple / stagger). More expressive combos; pairs with the wall (#3). *Adds: `HitEffect` state
   variants + multi-hit support on `MoveInstance`.*
7. **Air game** (jump, air actions, air block, cross-up / side-switch). The biggest missing axis and the
   one new *mixup* (left/right) TICK can't currently express. Heavy, but the single largest depth
   increase. *Adds: a vertical position, air states, side-switching, and the auto-face exception.*
8. **Hit-based stun / dizzy.** A second vulnerability meter (accumulated hits → a brief dizzy), parallel
   to guard-break but on offense. *Adds: one resource + a stun state.*

### Tier 3 — small completeness items (finish what's spec'd)

9. **Wire the unwired RPG levers.** WIS → wider parry window + Focus refund; INT → −Focus cost on
   cancels. Today these are flat `CONFIG` constants (see `mechanics.md` §10.1). Closes the gap between
   the spec's R-2 lever table and the compiler.
10. **Grounded-combo damage proration** + an explicit **juggle hit-count cap** — round out the scaling
    story beyond hitstun decay + juggle decay.
11. **The opposed throw-escape contest** (the spec's *one* remaining dice use, §4.1) and **block
    pushback** — both small, both spec'd, neither implemented.
12. **Author the forward-movement set** (step / dash / hop / sidewalk) as content — the engine already
    supports it; only data is missing.

---

## Part D — Deliberate exclusions (so absences aren't read as oversights)

These are **non-goals**, not gaps:

- **Real-time execution tech** — just-frames, plinking/piano, kara-cancel timing, input buffering,
  negative edge, motion/charge inputs, option selects, Korean backdash, wavedash. The turn-based
  confirm model replaces the *purpose* (frame traps, confirms, shimmies survive); the *dexterity* is
  intentionally gone (spec §B.2).
- **Platform-fighter model** — percent/knockback, DI/SDI, ledges, recovery, edgeguarding. Different
  genre; incompatible with the HP + lane design.
- **Tag / team play** — assists, DHC, snapback, X-Factor, team supers. TICK is 1v1.
- **Projectiles / zoning** — **deferred** (decision 8), not excluded: the `PROJECTILE_SPAWN` data slot
  is kept and the engine stub throws until it's built.
- **Graphics, game UI, and Layer 5** (encounter / AI / progression) — out of scope for the prototype.
- **Hitstop / hitlag freeze** — unnecessary; the engine pauses at decision checkpoints and confirms via
  cancel gates rather than freeze frames.

---

## A note on adding any of this

The durable deliverable is the **design + the pure core + the golden vectors**, not the TypeScript
(`../ts/PORTING.md`, `../ts/CLAUDE.md`). So every mechanic above should be added the same disciplined way the engine
was built: as **plain data + free functions**, **tagged unions** matched exhaustively, **integers/
fixed-point only** in gameplay, behind the existing module boundaries (new states in `core/`, new
contact rules behind `doesHit`, RPG knobs only through `rpg/compiler.ts`), with the **balance rules
(R-1…R-5) upheld** and **new golden vectors** freezing the behavior. A flashy mechanic that breaks
determinism, smuggles in a float, or opens a net-positive AP loop isn't worth it — the whole point is
that "balanced" and "portable" stay *checkable properties*.

---

## Sources

The cross-genre catalog was assembled from genre knowledge plus targeted verification of 2023–2025
systems: SF6 Drive (SuperCombo/GameRant), Tekken 8 Heat (Tekken wiki), Fatal Fury: City of the Wolves
REV/S.P.G. (Dream Cancel / SNK), MK1 Kameo (MK wiki / SuperCombo), Guilty Gear Strive Roman Cancels +
Wild Assault / Deflect Shield (Dustloop), Under Night GRD/Vorpal (Mizuumi), UMvC3 tag systems
(SuperCombo), and Smash/Melee tech (SmashWiki).
