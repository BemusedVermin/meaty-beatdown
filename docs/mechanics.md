# TICK — Mechanics Reference

*A readable, mechanic-by-mechanic tour of everything the engine actually implements.*

This document describes the **behavior** of the prototype as built, not the aspirations of the spec.
Where the implementation deliberately diverges from `frame_rpg_spec.md` (per the 12 locked
decisions in `../ts/CLAUDE.md`), this doc follows the **code**. For the design rationale behind any
mechanic, follow the `spec §x.y` pointers; for the resolved-ambiguity rationale, see `../ts/NOTES.md`.

> **What kind of game this is.** TICK is a **headless, deterministic, turn-based** engine. There is no
> real-time clock and no real-time input: the engine advances a tick counter and *pauses* whenever a
> player must decide. "Execution" in the traditional fighting-game sense (frame-perfect links, just
> frames, plinking, dash-cancel timing) is therefore **translated into reading + sequencing** — the
> skill is *what* you commit and *when in the tick stream*, never *how fast your hands are* (spec §B.2).
> Keep that in mind throughout: many real-time mechanics have no analog here *by design*, and the
> companion doc [`mechanics-gap-analysis.md`](./mechanics-gap-analysis.md) sorts out which absences are
> deliberate vs. genuine gaps.

A "mechanic" below = one rule the engine enforces. Each entry gives **what it does**, the **key
types/functions and file**, and **notable behavior/numbers** (constants live in `core/config.ts`).

---

## 1. The timeline & turn model

### 1.1 The tick
The atomic unit of time. By convention **1 tick = 1 frame at 60 Hz**, so real fighting-game frame data
ports over one-to-one. A single global counter `T` is shared by both fighters; the wall clock is
irrelevant. *(`core/tick.ts`; spec §0.1.)*

### 1.2 `ready_tick` — what turns continuous combat into turns
Every entity carries a `readyTick`: the absolute tick at which it becomes actionable again. **The
engine always asks the entity with the lower `readyTick` to choose next.** Everything else is
bookkeeping on that one idea. *(`Entity.readyTick`, `isActionable`, `moveReadyTick` in
`core/entity.ts`; spec §0.4.)*

### 1.3 Decision regimes: NEUTRAL vs PRESSURE
The regime is derived *entirely* from a `readyTick` comparison — no special-casing (`core/regime.ts`,
spec §2.1):

- **NEUTRAL (simultaneous):** both entities actionable on the same tick → **both commit hidden, then
  reveal.** This is the neutral-game mind-read.
- **PRESSURE (sequential):** one entity is actionable and the other is locked (in a move, hitstun,
  blockstun, recovery, or down) → the free entity chooses **with full information** about what the
  other is locked into and for how long. This is offense / okizeme / punishing.

### 1.4 The resolution loop
`runMatch(initialState, moveTables, agents, options) → { finalState, trace, winner }`
(`core/engine.ts`, spec §2.2). It (1) computes the regime, (2) asks the actor(s) for an `Action`,
(3) advances `T` tick-by-tick applying contacts on active frames, pausing at **cancel checkpoints** and
**decision points**, and (4) repeats until a KO or a tick/decision budget is hit. The whole match is a
pure function of `(initialState, moveTables, decisions)` — same inputs ⇒ byte-identical trace.

### 1.5 Move phases
Each in-flight move is in exactly one phase, derived from elapsed ticks (`phaseAt` in
`core/entity.ts`): **STARTUP → ACTIVE → RECOVERY → DONE**. Active frames are
`elapsed ∈ [startup, startup+active−1]`; the entity is actionable again at `elapsed = total`
(`total = startup + active + recovery`).

### 1.6 The trace
Every turn boundary and event is recorded as a `TraceEvent` (tagged union in `core/engine.ts`):
`STATE` (entity-state snapshot + regime), `COMMIT`, `WAIT`, `CANCEL`, `DENIED` (an unaffordable move),
`CONTACT` (the resolved interaction), and `KO`. The trace is the behavioral contract the golden vectors
freeze.

---

## 2. Frame data — the FrameProfile

The central object. The engine is an *interpreter* that runs a **resolved** `FrameProfile`; the RPG
layer is a *compiler* that emits one (`core/frameprofile.ts`, spec §0.2).

```
FrameProfile {
  timing:           { startup, active, recovery }   // ticks; total is derived
  hitEffect:        HitEffect                        // §3.1
  properties:       Property[]                       // §4 — frame flags on tick windows
  level:            MoveLevel                         // HIGH|MID|LOW|OVERHEAD|THROW|UNBLOCKABLE
  reach:            ReachProfile                      // §5 — spatial footprint
  cost:             ResourceCost                      // §8 — AP/Stamina/Focus + conditional ap_gain
  cancelWindows:    CancelWindow[]                    // §7 — where/what this can cancel into
  startupCancelable: boolean                          // §7 — default false (decision 6)
  motion?:          { lane, offset }                  // repositioning for movement moves
}
```

### 2.1 Frame advantage is *derived*, never stored (invariant I-1)
There are **no `on_hit` / `on_block` fields** to set inconsistently. They are computed
(`onHit`/`onBlock` in `core/frameprofile.ts`):

```
on_hit   = defender_hitstun   − attacker_recovery
on_block = defender_blockstun  − attacker_recovery
on_whiff = 0                    (you eat full recovery and are exposed)
```

This makes "the advantage the engine reports" *always* consistent with the stun and recovery numbers —
the authoring tool can't lie to the player (spec §0.2, audit C-1).

### 2.2 Profile validation
`checkFrameProfile` rejects malformed data: negative timing, `active < 1`, non-integer damage, and any
property/cancel window that falls outside the move's `[0, total−1]` frame span or that has `to < from`.

---

## 3. Hit effects, counter-hits, juggles, knockdown

### 3.1 HitEffect — what a clean hit does
`{ damage, hitstun, blockstun, chipDamage, knockback, launches, knockdown }`
(`core/frameprofile.ts`). `damage` is integer HP; `chipDamage` goes to **Poise** (guard), not HP;
`knockback` is fixed-point lane pushback; `launches` → AIRBORNE (juggle), `knockdown` → DOWN (oki).

### 3.2 Counter-hit (CH)
If the defender is struck while in a **counter-hit state** — their *own* move's startup or recovery, or
an explicit `COUNTER_HIT_STATE` window — the hit deals **×1.25 damage** (rounded half-up) and **+6
ticks of hitstun** (`counterHitDamage` / `counterHitHitstun` in `core/resolver.ts`; constants
`CH_DAMAGE_MULT`, `CH_HITSTUN_BONUS`). CH is the payoff for winning a timing read (whiff-punishing,
frame traps).

### 3.3 Juggles & gravity scaling
A launcher puts the defender in **AIRBORNE** (carrying a `juggleCount`). Each successive juggle hit's
damage is scaled by **×0.9 per hit** (`juggleScaledDamage`, `JUGGLE_DAMAGE_DECAY = 0.9`), so juggles
terminate. This is the anti-infinite rule on the damage axis (spec §2.8).

### 3.4 Knockdown & wake-up
`knockdown` puts the defender in **DOWN** with a `wakeupTick`; they are non-actionable until they rise,
and the attacker (being actionable first) holds the **PRESSURE** regime for okizeme. A wake-up reversal
is expressible as any move with an `INVULN` startup window — the "get-off-me" option — though no sample
reversal move ships in the content (spec §2.8).

---

## 4. Properties — the frame flags

Each property is live during an **inclusive tick window** `[from, to]` measured in `elapsed` from the
move's start (`core/frameprofile.ts`, spec §0.3). All are tagged-union variants:

| Property | Effect during its window | Notes |
|---|---|---|
| **INVULN** `{invulnType}` | Hitboxes of the matching category pass through. | `invulnType ∈ ALL / STRIKE / THROW / PROJECTILE`. Drives reversals & backdash i-frames. |
| **ARMOR** `{armorHits, armorDamageMult}` | Absorb up to `armorHits` **strikes** with no hitstun; still take `armorDamageMult` of the damage; the move continues. | **Throws beat armor** (decision 1) — handled in the resolver, not here. |
| **COUNTER_HIT_STATE** | Being struck here is a counter-hit (§3.2). | Extends CH beyond the default startup/recovery windows. |
| **GUARD_POINT** | Auto-deflects one strike → **parry** outcome. | The parry/sabaki window (§6.2). |
| **BLOCK** `{covers}` | A held stance: a strike whose `level` is in `covers` is blocked; an uncovered level is the mixup landing (clean hit). | Throws ignore it (§6.4). |
| **AIRBORNE** | Entity is in juggle-state. | Used by the launch path. |
| **PROJECTILE_SPAWN** | *(deferred — decision 8)* | Data slot kept; the engine **stub throws** if a spawn is invoked. Projectiles are not simulated. |

> **Tracking is *not* a property.** How a move behaves vs. a sidestep (LINEAR / TRACKING / HOMING) lives
> entirely in `ReachProfile` (§5), so *all* contact math stays behind one predicate (audit C-7).

---

## 5. Spatial model & contact (`spatial/lane.ts`)

### 5.1 Two axes, one job each
- **`pos` (the lane):** the 1D distance scalar — spacing, weapon reach, footsies, knockback. *All*
  frame-data spacing math runs here.
- **`offset` (lateral/depth):** does **one** job — sidestep **evasion**. It is not a second spacing axis.

Fighters **auto-face** and re-center the lane when they become actionable, so `offset` matters *during*
a committed move — exactly when a sidestep dodges it (spec §1.1).

### 5.2 ReachProfile
`{ minRange, maxRange, heightLow, heightHigh, advance, lateralBand, stepIn, trackSide }`
(`core/spatial-types.ts`). `advance` = ground the attacker closes during the move; `lateralBand` =
half-width of the hitbox on the offset axis; `stepIn` = lateral realignment (TRACKING/HOMING > 0);
`trackSide ∈ {−1,0,+1}` = which sidestep direction the move covers.

### 5.3 `doesHit` — the single contact predicate
True iff, **on an active frame**, the attack lines up with the defender (`doesHit` in `spatial/lane.ts`,
spec §1.2). It checks, in order:

1. **type** — defender is not invuln to this attack's category (i-frames win);
2. **range** — defender within `[minRange, maxRange]` along the lane *after `advance`*;
3. **height** — defender's `height` within `[heightLow, heightHigh]`;
4. **lateral** — within the sidestep band — **skipped for throws** (throws ignore offset).

### 5.4 Sidestep / tracking (LINEAR vs TRACKING vs HOMING)
The lateral clause (`inLateralBand`) encodes the Tekken read:

- **LINEAR** (`stepIn = 0`): narrow band → a sidestep beyond `lateralBand` makes the move **whiff**.
- **HOMING** (`stepIn > 0`, `trackSide = 0`): realigns on **both** sides → beats a sidestep either way.
- **TRACKING** (`stepIn > 0`, `trackSide = ±1`): covers **one** side only; stepping the other way dodges.

A sidestep that beats a linear move routes down the same `on_whiff = 0 → eat full recovery → exposed`
path as a baited whiff, so it is a **whiff-punish setup**, structurally identical to footsies.

---

## 6. Contact resolution — the interaction priority table

When `doesHit` is true, `classifyContact(att, def)` (`core/resolver.ts`, spec §2.4) decides the
outcome. Read top-to-bottom it **is** the priority order:

```
1. defender invuln to this type            → WHIFF
2. attack is a THROW:
      defender also throwing this tick      → THROW_TECH   (clash, both reset, no damage)
      else                                  → THROWN       (beats block / armor / parry)
3. defender in a GUARD_POINT window         → PARRIED      (attacker frozen, defender hugely plus)
4. defender holding BLOCK:
      level covered                         → BLOCKED      (chip + blockstun + on_block)
      level uncovered (the mixup)           → HIT          (clean — guessed wrong)
5. defender has ARMOR hits remaining        → ARMORED      (damage, no hitstun, move continues)
6. otherwise                               → HIT          (counter = was in a CH state)
```

The seven `ContactResult` variants — `WHIFF / PARRIED / THROWN / THROW_TECH / BLOCKED / ARMORED /
HIT{counter}` — are exactly this branch. The remaining sub-sections explain each defensive option.

### 6.1 Block & guard break
Block is a held **stance** (a move with a `BLOCK` property, e.g. sample `guard` covering `HIGH/MID`).
On a blocked strike the defender takes **chip to Poise** (not HP), enters **blockstun**, and the
attacker gets `on_block`. When **Poise reaches 0**, the defender is **GUARDBROKEN** for **40 ticks**
(`GUARD_BREAK_STUN_TICKS`) — a long, fully-punishable stun — and Poise resets to max
(`core/engine.ts`, BLOCKED branch; spec §2.5). This caps pure turtling.

### 6.2 Parry
A short `GUARD_POINT` window (sample `parry`: active 2–5). A strike caught in it is **PARRIED**: the
attacker is frozen for **30 ticks** (`PARRY_FREEZE_TICKS`) while the parrier recovers in **4**
(`PARRY_RECOVER_TICKS`) — a huge advantage. Success also **refunds Focus (+1)** and **AP (+2)**
(`PARRY_FOCUS_REFUND`, `PARRY_AP_REFUND`; decision 7) — defense that *generates* tempo. High risk
(tight window, loses to throw), high reward.

### 6.3 Armor
Absorbs up to `armorHits` strikes with no hitstun, taking `armorDamageMult` of the damage, and the
armored move keeps going. Per-instance absorption is tracked by `MoveInstance.armorHitsUsed`. Armor
stops **strikes only** — **throws connect through it** (decision 1).

### 6.4 Throws & throw-tech
Throws are short-range, **ignore the lateral check** (beat sidestep), and **beat block, armor, and
parry**. They lose to strikes (a throw has startup; a strike counter-hits the whiff) and to spacing
(backdash). Two throws on the **same tick** → **THROW_TECH**: a clash, both recover in **8 ticks**
(`THROW_TECH_RECOVER_TICKS`), no damage. This closes the defensive RPS so no single option dominates
(spec §2.6).

### 6.5 Sidestep (the lateral defensive option)
Covered in §5.4: dodges **LINEAR** strikes, loses to **TRACKING/HOMING**, and does nothing vs. throws.
The attacker's answer to a step-happy defender is to mix in homing moves — which cost more on the
budget (§9), so coverage is paid for.

> **The defensive RPS:** block ↔ throw, parry ↔ throw/empty, backdash ↔ advancing moves,
> sidestep ↔ homing. Each option is beaten by something; the audit (C-11) checks none strictly
> dominates.

---

## 7. Cancels, confirms & the startup-cancel rule

A **cancel** interrupts your own move during a marked window and chains into another, truncating the
current move's recovery and starting the new move's startup immediately at `T` — which is *why*
canceling creates plus frames and combos (spec §2.3, §3.4).

### 7.1 CancelWindow
`{ from, to, gate, into, cost }` (`core/cost.ts`). The **gate** decides when the cancel is legal:
`ON_HIT / ON_BLOCK / ON_CONTACT / ALWAYS / ON_WHIFF`. `into` lists the legal target move IDs; `cost` is
usually Focus (and possibly AP) — *this is why combos are finite*.

### 7.2 Hit-confirm (the lock-then-confirm rule)
You commit your *initial* move blind (in NEUTRAL), but a cancel gated `ON_HIT`/`ON_BLOCK` is decided by
the **actual contact result** (tracked as `MoveInstance.contact`), so you genuinely **hit-confirm** —
you only spend resources to combo when the hit is real, *not* by reacting to the opponent's input
(spec §2.10).

### 7.3 No startup cancels by default
A move is cancelable only from **active/recovery** unless it sets `startupCancelable: true`
(`STARTUP_CANCELABLE_BY_DEFAULT = false`, decision 6). This closes the "react to the reveal by
canceling my startup" exploit and makes feints a real, costed choice (audit C-6).

---

## 8. Resources & the AP / tempo economy

Five pools per entity (`Resources` in `core/entity.ts`; spec §3.1). Each answers a *different* question,
so a string can end for several distinct strategic reasons:

| Resource | Regenerates? | Spent on | Role |
|---|---|---|---|
| **HP** | No (in combat) | — | Lose condition. |
| **Stamina** | **+1/tick while not executing a move** (`STAMINA_REGEN_PER_TICK`) | most attacks, dashes | exertion; spacing to recover it is a decision. |
| **Poise** | Resets to max on guard-break | absorbing blocked chip | the guard-break meter (§6.1). |
| **Focus** | Earned (parry/CH refund) | specials, cancels, reversals | "earned offense" — gates access to power. |
| **AP** | **Refills to max only on entering NEUTRAL** | every action's `ap_cost` | the tempo / turn-budget (below). |

### 8.1 The AP model (Tempo)
AP is the **action economy** (decisions 4 & 5, spec §3.5) — *not* a fixed per-turn budget:

- **NEUTRAL:** AP refills to max, then you commit **one** action (no chaining against a free opponent).
- **PRESSURE:** you **chain** actions, paying each `ap_cost`, as long as you can afford the next and a
  cancel/link window allows it. When you can't (or won't) pay, your turn yields and initiative
  re-evaluates by `ready_tick`. **A long combo is literally an AP expenditure.**

AP refills *only* on entering NEUTRAL (`refillBothAp` in `runMatch`), so within a pressure sequence you
spend down toward exhaustion.

### 8.2 Moves that generate AP (`ap_gain`)
A move may carry a **conditional** `apGain { amount, gate }` (`ON_HIT / ON_CH / ON_BLOCK / ON_PARRY /
ALWAYS`). AP-generation is conditional on **success**, never unconditional — you earn extra actions by
playing well. Examples in the sample content: a light jab is **net-neutral on hit** (`ap_cost 1`,
`+1 ON_HIT`) so confirms keep your turn alive; a tempo jab rewards frame traps (`+2 ON_CH`); a parry
banks tempo (`+2 ON_PARRY`); a heavy finisher is AP-negative and ends the string.

### 8.3 AP_max from a derived tempo stat (decision 5)
`AP_max = AP_BASE (3) + tempoTier`, where `tempoMod = roundHalfUp((dexMod + wisMod) / 2)` and
`tempoTier` = how many of the thresholds `[1, 3, 5]` it clears (`tempoTier`/`apMaxFor` in
`rpg/compiler.ts`). **Tempo is derived from DEX + WIS — it replaces the spec's CHA assignment** (the
spec text still says CHA; the code does not). A strong DEX+WIS build reaches `AP_max = 5`.

### 8.4 Unaffordable → DENIED
If an agent picks a move it cannot pay for (AP/Stamina/Focus), the engine degrades it to **WAIT** and
emits a `DENIED` trace event. This is how AP/Stamina/Focus exhaustion *mechanically* ends a string
(`core/engine.ts`).

---

## 9. The combo system — four independent governors

Combos are guaranteed finite by **four** independent throttles (defense in depth — no single tuning
miss makes an infinite). The audit (C-4) verifies all four are present:

1. **Focus cost per cancel** — each special-cancel costs Focus (§7.1); you run out.
2. **Juggle damage decay** — ×0.9 per juggle hit (§3.3); damage trends to nothing.
3. **Hitstun decay** — `effectiveHitstun = base − (comboCount−1)×2`, floored at **1**
   (`effectiveHitstun` in `core/resolver.ts`). Each chained hit gives less advantage, so the string's
   advantage eventually goes negative and the combo **must** end.
4. **AP exhaustion** — each action costs AP and AP only refills in NEUTRAL (§8.1); you can't afford the
   next link.

Each answers a different question (out of access vs. out of damage vs. out of advantage vs. out of
tempo), so they enrich rather than overlap. The **R-5** rule below additionally forbids any
*net-positive* AP loop.

---

## 10. The RPG layer — stats compile to frame data (`rpg/`)

The load-bearing rule: **stats and equipment are compilers that emit FrameProfiles; the engine is an
interpreter that runs them.** The single bridge is `rpg/compiler.ts` (the only `rpg/` file allowed to
import the engine; audit C-5). The engine never sees a stat.

### 10.1 Attributes → frame-data levers (one major lever each, R-2)
Compiled in `rpg/compiler.ts`; curves in `CONFIG.rpg`. **WWN-style low modifiers** keep frame swings
small so the engine stays the star.

| Attribute | Wired lever | Numbers |
|---|---|---|
| **STR** | +damage on **heavies & throws**; +**armor hits** budget | `+2 dmg/mod`; `+1 armor hit/mod`, capped at 2 |
| **DEX** | **−startup** on moves; +movement `advance` | `−1 startup/mod`, capped at 3; `+1 advance/mod` |
| **CON** | +HP, +Stamina, +Poise | `+10 HP`, `+5 stamina`, `+3 poise` per mod |
| **INT** | +Focus pool | `+2 focus/mod` (base 10) |
| **WIS** | feeds **tempo** (with DEX) → AP_max | via `tempoMod` (§8.3) |
| **CHA** | *(no wired lever)* | tempo moved to DEX+WIS (decision 5) |

> **Honest note on partial wiring.** The spec assigns WIS a *defensive-reads* lever (wider parry
> window, Focus refund on parry/CH) and INT a `−Focus cost on cancels` lever. Those per-point scalings
> are **not** wired — parry refunds are flat `CONFIG.combat` constants, and WIS currently only feeds
> tempo. See the gap-analysis doc.

### 10.2 Skills & Foci
`Sheet = { attributes, skills, foci }` (`rpg/sheet.ts`). Weapon-class **skill rank (0–4)** gates which
moves are usable (rank does **not** add a to-hit — combat is deterministic). **Foci** are the
build-defining unlocks / archetype signatures and are the modular content slots. *(Foci are carried as
data on the sheet; sample sheets list `read_the_wind` and `iron_guard`.)*

### 10.3 Equipment (`rpg/equipment.ts`)
A **second compiler** into FrameProfile, stacking after stats. A `Weapon` carries
`{ minRange, maxRange, startupDelta, recoveryDelta, damageDelta, requirements, grantsMoves }` — so
**the weapon is your spacing identity and range/speed/damage profile**. Requirements are a **floor**
(R-3): unmet → you can't use the weapon at all (empty move list); meeting them more gives only small
capped bonuses, never runaway scaling.

### 10.4 What the compiler does
`compileProfile` resolves a base move against a sheet + weapon: DEX lowers startup (capped, floored at
`MIN_STARTUP`); the weapon shifts startup/recovery/damage and **sets the lane range**; STR adds damage
to heavies/throws and adds armor budget. **Hitstun/blockstun are never touched**, so advantage stays
I-1-consistent. `compileResources` builds the pools (all start full); `apMaxFor` sets the AP cap.

---

## 11. Balance as a checkable property (`balance/`)

### 11.1 The MOVE_VALUE budget identity
Every move must "pay" for its strengths with weaknesses (`balance/budget.ts`, spec §4.5):

```
MOVE_VALUE = w_speed·(BASELINE_STARTUP − startup) + w_safety·on_block + w_reward·damage
           + w_range·reach + w_props·Σ(property value) − w_cost·resource_cost
           − w_commit·whiff_recovery        ≈  ARCHETYPE_BUDGET (within ±ε)
```

A move that is fast, safe, long, damaging, and propertied must be either resource-expensive or horribly
punishable — or it's flagged. The weights `w_*` are the master tuning knobs (playtest-tuned).

### 11.2 The balance rules (R-1 … R-5)
Linted over the content by `balance/budget.ts`:

- **R-1** — no zero-cost action: every offensive/defensive option pays or drains a resource.
- **R-2** — one major lever per attribute (no attribute drives both a major offensive and defensive lever).
- **R-3** — gates are floors with **capped** bonuses, never runaway multipliers.
- **R-4** — the **range ↔ speed ↔ damage** weapon tradeoff: no Pareto-dominant weapon.
- **R-5** — no **net-positive AP cycle** in the cancel graph (a move with `ap_gain ≥ ap_cost` may not
  sit in its own transitive cancel set). The action-economy analogue of juggle decay.

### 11.3 The consistency audit (C-1 … C-11)
`balance/audit.ts` (`npm run audit`) prints a PASS/FAIL row per check: I-1/validity (C-1), regime from
`ready_tick` (C-2), determinism / byte-identical replay (C-3), four governors present (C-4), single
L4→engine bridge (C-5), react-to-reveal closed (C-6), single `doesHit` predicate (C-7), sidestep
whiffs a linear move (C-8), deterministic combat / no d20 (C-9), no net-positive AP cycle (C-10),
sidestep counterplay — homing connects through the step (C-11).

---

## 12. Sample content (the concrete reference)

Swappable data the engine never hard-codes (`content/sample.ts`). Two archetypes that prove the
DEX-frail vs. STR-armored identities come *entirely* from compiled frame data:

**Reza** — DEX dagger, fast & frail (`{str:0, dex:3, con:1, int:2, wis:2, cha:0}`, focus `read_the_wind`):
- `light_jab` (3/2/5, 8 dmg, LINEAR, `ap 1 / +1 ON_HIT`, cancels → `light_slash`)
- `light_slash` (4/2/6, 10 dmg, LINEAR, `+1 ON_HIT`, cancels → `special_riposte`)
- `special_riposte` (6/3/10, 20 dmg, **launches**, Focus 5 / `ap 2`)
- `throw_grab` (5/2/8, 18 dmg, **THROW**, knockdown, range 1)
- `backdash` (STRIKE-invuln 0–4, lane −2), `sidestep_l` / `sidestep_r` (offset ∓1)

**Borin** — STR greatsword, slow & armored (`{str:3, dex:0, con:3, int:0, wis:1, cha:0}`, focus
`iron_guard`):
- `heavy_cleave` (14/3/18, 40 dmg, **ARMOR 4 hits @4–14**, knockdown, range 4)
- `tempo_jab` (5/2/6, 8 dmg, LINEAR, `+2 ON_CH`, cancels → `homing_sweep`)
- `homing_sweep` (8/3/12, 18 dmg, **HOMING** `stepIn 2`, Focus 4 / `ap 2`, cancels → `heavy_cleave`)
- `guard` (BLOCK `HIGH/MID`), `parry` (GUARD_POINT 2–5, Focus 3, `+2 ON_PARRY`)

**Weapons** (R-4, no Pareto-dominant): `dagger` (range 0–2, fast, −dmg), `greatsword` (range 0–4, slow,
+dmg), `spear` (range 1–5, neutral deltas).

---

## 13. Cross-references

| For… | See |
|---|---|
| Design rationale for any mechanic | `frame_rpg_spec.md` (the `spec §x.y` pointers above) |
| The 12 locked decisions overriding the spec | `../ts/CLAUDE.md` |
| Every resolved-ambiguity `// DECISION:` | `../ts/NOTES.md` |
| Porting contract (fixed-point, golden vectors) | `../ts/PORTING.md` |
| **What fighting-game mechanics are missing** | [`docs/mechanics-gap-analysis.md`](./mechanics-gap-analysis.md) |
