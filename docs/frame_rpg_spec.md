# FRAME / RPG SYSTEM SPECIFICATION
### A turn-based RPG with fighting-game combat on a shared continuous timeline
*Working title: **TICK** (everything is built on the tick).*

---

## How to read this document

This spec is **modular by design** (per your stated preference). It is organized as a set of layers that each expose a clean interface and hide their internals, so any one layer can be re-implemented without touching the others:

```
┌─────────────────────────────────────────────────────────┐
│  L5  ENCOUNTER / AI            (out of scope — stubbed)   │
├─────────────────────────────────────────────────────────┤
│  L4  RPG LAYER   stats · skills · foci · equipment        │  ← feeds data down
├─────────────────────────────────────────────────────────┤
│  L3  MOVESET     moves · cancels · resources              │
├─────────────────────────────────────────────────────────┤
│  L2  COMBAT ENGINE   the shared tick timeline + resolver  │  ← the heart
├─────────────────────────────────────────────────────────┤
│  L1  SPATIAL     continuous lane · hitboxes · spacing     │
├─────────────────────────────────────────────────────────┤
│  L0  PRIMITIVES   tick · frame data · the entity state    │
└─────────────────────────────────────────────────────────┘
```

The contract between layers is deliberately narrow. **The RPG layer (L4) never reaches into the engine (L2).** It only produces a *resolved FrameProfile* and a *MoveList*, which the engine consumes as opaque data. This is the single most important architectural rule in the document: **stats and equipment are compilers that emit frame data; the engine is an interpreter that runs it.** That separation is what lets you tune the RPG and the fighting game independently.

Symbols used:
- **✅ Decided** — specified fully, internally consistent.
- **🔬 Researched** — grounded in a real-world comparable, noted inline.
- **⚠️ Follow-up** — a genuine fork where I made a defensible default but you should confirm.

A note on prior art, since you said the translation should be near one-to-one: the closest existing thing to what you're describing is **Your Only Move Is HUSTLE** (often "YOMI Hustle") — a turn-based game built directly on fighting-game frame data with simultaneous hidden commitment and no real-time clock. Where useful I reference it as a sanity check that the genre *works*. The formal backing for "map a continuous-time fight onto a discrete causality space so turn-based evaluation applies" is also a known technique in the animation/AI literature (the *temporal expansion approach*), which is reassuring: the translation you intuited is mathematically sound, not a hack.

---

# LAYER 0 — PRIMITIVES

## 0.1 The tick ✅

The atomic unit of time is the **tick**. By convention **1 tick = 1 frame at 60 Hz ≈ 16.67 ms**, so frame data ports over one-to-one from real fighting games and from your own intuition. The clock never advances in real time; it advances only when the resolver says so. "No real-time constraint" means *the wall clock is irrelevant; the tick clock is everything.*

There is a single **global tick counter `T`**, shared by both combatants. This shared clock is what makes whiff-punishing, frame traps, and spacing legible — exactly as in a real fighting game, where both characters live on one 60 Hz timeline.

## 0.2 Frame data — the core data structure ✅

Every action is described by a **FrameProfile**. This is the spec's central object; almost everything else exists to produce, modify, or consume it.

```
FrameProfile {
  startup        : int   # ticks before the first active tick
  active         : int   # ticks the hitbox/effect is live
  recovery       : int   # ticks after active before actionable again
  // derived: total = startup + active + recovery

  // --- on-contact frame advantage (the soul of the system) ---
  on_hit         : int   # frame advantage if it HITS (can be + or -)
  on_block       : int   # frame advantage if BLOCKED (usually -)
  on_whiff       : int   # = 0 by definition; you eat full recovery

  // --- properties (flags + windows), see 0.3 ---
  properties     : Property[]
  cancel_windows : CancelWindow[]   # see L3
  hit_effect     : HitEffect        # damage, stun, knockback, status
  cost           : ResourceCost     # see L3 — includes AP cost/gain (action economy, 3.5)
  reach          : ReachProfile      # see L1 (spatial)
  level          : MoveLevel        # high / mid / low / throw / unblockable
}
```

**Frame advantage** is the load-bearing concept. After two moves resolve, one character becomes actionable before the other. The difference (in ticks) is the *frame advantage*. Positive advantage = "your turn comes first in the next exchange" = pressure. Negative = "you're exposed" = the opponent may punish. This single number is how a turn-based game reproduces the entire neutral/pressure/punish loop of a fighter.

> **Identity that must always hold (invariant I-1):**
> `on_hit = (defender_hitstun) − (attacker_recovery_after_active)`
> `on_block = (defender_blockstun) − (attacker_recovery_after_active)`
> i.e. advantage is *defined* as "how long the defender is locked minus how long I'm locked." If you ever set `on_hit`/`on_block` by hand to a value inconsistent with the stun and recovery numbers, the engine is lying to the player. The authoring tool should compute these, never accept them raw. (Consistency hook — flagged again in the audit.)

## 0.3 Properties (frame flags) ✅

Properties attach to *specific tick ranges* of a move. They are how fighting-game tech (i-frames, armor, etc.) ports over. Each is a window `[from_tick, to_tick]` relative to the move's own start.

| Property | Effect during its window | Ported from |
|---|---|---|
| **Invincible (i-frames)** | Cannot be hit at all. Hitboxes pass through. | Reversals, wakeup DPs |
| **Strike-invuln / Throw-invuln / Projectile-invuln** | Typed invincibility (only that category passes through). | Type-specific reversals |
| **Armor (hyper armor)** | Absorbs N hits without entering hitstun; still takes damage (often reduced). `armor_hits` and `armor_damage_mult` params. | Tekken/SF armor moves |
| **Counter-hit state** | If struck during this window, defender takes a **counter-hit** (bonus damage + extended hitstun, see 2.7). Startup and recovery are counter-hit windows by default. | Universal CH rules |
| **Guard-point** | Auto-blocks one hit from a direction during the window, then continues the move. | Tekken sabaki / parries |
| **Cancelable** | Marks a window where cancels are legal (see L3). | Combo cancels |
| **Airborne / juggle-state** | Entity is launched; subject to juggle rules (2.8). | Launchers/juggles |
| **Projectile-spawn** | Emits an independent timeline entity (see 2.9). | Fireballs/zoning |
| **Tracking** | How the move behaves vs. a side-stepping defender: `LINEAR` (whiffs if defender stepped off-axis), `TRACKING` (realigns vs. moderate offset, weak to one side), `HOMING` (realigns fully, beats sidestep both ways). Default `LINEAR`. | **Tekken linear/homing system** |

⚠️ **Follow-up:** whether **armor** also applies to throws. Default: armor does **not** stop throws (throws beat armor), preserving the rock-paper-scissors below. Confirm.

## 0.4 Entity state ✅

```
Entity {
  pos            : float    # position on the spacing lane (distance axis)   [L1]
  offset         : float    # lateral/depth displacement off the shared axis (sidestep)  [L1, see 1.1]
  facing         : ±1
  ready_tick     : int      # the tick at which this entity becomes actionable
  state          : NEUTRAL | STARTUP | ACTIVE | RECOVERY | HITSTUN
                 | BLOCKSTUN | AIRBORNE | DOWN | GUARDBROKEN
  current_action : MoveInstance | null
  resources      : { poise, focus, stamina, ap, ... }   # see L3 (ap = action economy, 3.5)
  status_effects : StatusEffect[]
  rpg            : RPGSheet  # L4 — read-only to the engine
}
```

`ready_tick` is the mechanism that turns a continuous fight into turns: **the engine always asks the entity with the lower `ready_tick` to choose next.** Everything downstream is bookkeeping on that one idea.

---

# LAYER 1 — SPATIAL MODEL

## 1.1 The grid — lane + Tekken sidestep ✅

**Clarified model:** the spacing dimension is a **continuous 1D lane** (the distance axis between the two fighters — unchanged from before). Layered on top is a **shallow lateral/depth axis** for **Tekken-style sidestepping**: in addition to forward/back along the lane, a character can step *sideways* (into or out of the screen-depth), tracked by a scalar `offset`.

The crucial point you made — *"still on a 1D lane"* — is honored precisely: **`offset` is not a second spacing axis.** Spacing, reach, and all the frame-data math still run on the 1D `pos` lane exactly as before. The `offset` axis does one job and one job only: **evasion.** It decides whether an attack's hitbox lines up with the defender, not how far apart they are.

How the two axes divide labor:
- **`pos` (lane / distance):** spacing, whiff-by-range, weapon reach, footsies, knockback. *(All of L1.2's math, untouched.)*
- **`offset` (lateral / depth):** sidestep evasion. Stepping off-axis makes **linear** attacks miss; the counterplay is **tracking / homing** attacks (0.3) that realign. Fighters **auto-face** (re-orient toward each other) when actionable, so the lane re-establishes itself after a sidestep — `offset` matters mainly *during* an opponent's committed move, which is exactly when a sidestep dodges it.

This keeps the one-to-one frame translation intact (spacing is still a scalar) while adding the Tekken layer you want. Because everything still flows through the single `does_hit` predicate (1.2), this is a localized extension — `spatial/lane.ts` only (App. A).

🔬 In Tekken, sidestep beats *linear* moves and loses to *homing/tracking* moves, and moves often track better to one side than the other — that asymmetry (sidestep-left vs sidestep-right being different reads) is the depth this adds, and it's reproduced below.

## 1.2 Reach and hitboxes ✅

```
ReachProfile {
  min_range    : float    # closer than this and the move whiffs (too close / over the head)
  max_range    : float    # outer edge of the hitbox along the lane
  height_low   : float    # vertical coverage (for anti-air / low attacks)
  height_high  : float
  advance      : float    # how far the attacker MOVES along the lane during startup+active
  lateral_band : float    # half-width of the hitbox on the OFFSET axis.
                          # |attacker.offset − defender.offset| must be ≤ this to connect.
                          # LINEAR moves have a narrow band → a sidestep dodges them.
  step_in      : float    # lateral realign during the move (TRACKING/HOMING > 0; LINEAR = 0)
  track_side   : -1|0|+1  # which sidestep direction this move covers better (0 = both/none)
}
```

The engine's spatial contract is still **one predicate** (so 1D and lane+sidestep expose the same interface — nothing in L2 changes):

```
does_hit(attacker, defender, tick) -> bool
  # true iff at `tick` the move is in an ACTIVE frame, AND:
  #   (range)   defender ∈ [min_range, max_range] from attacker along the lane (after `advance`)
  #   (height)  defender.height ∈ [height_low, height_high]
  #   (lateral) |attacker.offset − defender.offset|  ≤  lateral_band + step_in
  #             — i.e. a sidestep beyond the band makes a LINEAR move WHIFF;
  #               TRACKING/HOMING add step_in to realign; track_side widens it on one side only
  #   (type)    defender is not invuln to this move's type at `tick`
```

This makes **sidestep evasion a spatial fact, just like backdash whiffing** — it routes through the exact same `on_whiff = 0` → "you eat full recovery, now you're exposed" path. A read-based sidestep on a linear heavy is therefore a *whiff-punish setup*, identical in structure to baiting a whiff with footsies.

## 1.3 Movement ✅

Movement is just a move with a FrameProfile (so it lives on the same timeline and can be whiff-punished — committing to a big dash has recovery!).

| Movement | startup | active(moving) | recovery | notes |
|---|---|---|---|---|
| **Step** (short) | 1 | `n` | 1 | cheap spacing adjustment |
| **Dash** (commit) | 3 | `n` | 6 | covers ground fast, punishable |
| **Backdash** | 3 | `n` | 8 | often has i-frames on early startup 🔬 (universal in fighters) |
| **Jump** | 4 (prejump) | airborne arc | landing recovery 4 | enters AIRBORNE |
| **Hop / microdash** | 2 | `n` | 3 | mobility tool |
| **Sidestep L / R** | 3 | 2 | 7 | quick lateral hop: sets `offset` off-axis to dodge **LINEAR** moves; loses to TRACKING/HOMING; punishable on whiff. L vs R are distinct reads (see `track_side`). |
| **Sidewalk L / R** | 2 | `n` (hold) | 5 | sustained lateral movement; covers more `offset` than a step but is slower to recover and more exposed. |

After any sidestep/sidewalk, **auto-facing** re-centers the lane once you're actionable (1.1), so `offset` evasion is a *timing tool used against a committed move*, not a permanent position. A sidestep that beats a linear attack puts you at advantage exactly like a whiff-punish.

Distance covered = `speed_stat × active_ticks` (L4 supplies `speed_stat`). Because movement consumes ticks on the shared clock, **repositioning has an opportunity cost** — you cannot reposition for free, which is what makes neutral a real decision.

---

# LAYER 2 — THE COMBAT ENGINE (the heart)

This is the near one-to-one translation you asked for. I'll give the **resolution loop** first (your 5-step flow, formalized and made consistent), then the **interaction rules** (block/parry/throw/CH/juggle) that the loop calls into.

## 2.1 The fundamental question: who chooses next? ✅

At any moment the engine is in one of two **decision regimes**, determined entirely by `ready_tick`:

- **NEUTRAL (simultaneous regime):** both entities are actionable at the same tick (`ready_tick` equal, or both ≤ `T`). → Both commit **simultaneously and hidden**, then reveal. This is the neutral-game mind read.
- **PRESSURE (sequential regime):** one entity is actionable and the other is *not* (locked in a move, hitstun, blockstun, or recovery). → The actionable entity chooses **with full information** about what the other is locked into and for how long. This is offense/okizeme/punishing.

> **Why this is the elegant core:** in a real fighter, "neutral" is when both players are free and reading each other; "pressure" is when one is plus and acting on a known-disadvantaged opponent. By keying the regime off `ready_tick`, *the same timeline produces both* with no special-casing. This is the consistency keystone of the whole engine.

## 2.2 The resolution loop ✅ (your 5-step flow, formalized)

```
loop:
  # 1. Determine regime
  actor_set = entities with minimal ready_tick (1 = pressure, 2 = neutral)

  if regime == NEUTRAL:
      # (1) you commit, (2) opponent commits — simultaneously, hidden
      a_choice = entityA.commit()        # choose a move (or movement, or wait)
      b_choice = entityB.commit()        # hidden until both locked
      reveal(a_choice, b_choice)

  else: # PRESSURE
      choice = actor.commit()            # full info: sees opponent's lock & remaining ticks

  # (3) CANCEL WINDOW — offered DURING execution, see 2.3 and L3
  #     handled inside the tick advance below, not as a separate phase

  # (4) EXECUTE: advance the global clock tick-by-tick, resolving contacts
  advance_until_next_decision()

  # (5) COOLDOWN: recovery is already part of each move's FrameProfile;
  #     ready_tick was set when the move was committed.

  if combat_over: break
```

### `advance_until_next_decision()` — the tick engine ✅

This is the literal interpreter. It steps `T` forward one tick at a time and stops the instant a player needs to make a decision (a cancel window opens, or someone becomes actionable). Stepping one tick:

```
for each tick T:
  for each entity with an in-flight move:
     update phase (STARTUP→ACTIVE→RECOVERY) based on elapsed ticks
     apply active-frame contacts:
        for each ACTIVE attacker:
           if does_hit(attacker, defender, T):   # L1 predicate
              resolve_contact(attacker, defender, T)   # → 2.4
     move entity by its per-tick velocity (movement / advance / knockback)
     tick down status_effects, stun timers, projectile lifetimes

  # decision checkpoints:
  if any entity entered a CANCEL window this tick: PAUSE → offer cancel (2.3)
  if any entity's ready_tick == T (became actionable): PAUSE → return to loop
```

**No real-time constraint** falls out for free: the engine pauses at every checkpoint and waits for human input indefinitely. The *tick clock* is the only clock.

## 2.3 Cancels — your step (3) ✅

A **cancel** lets you interrupt your own move during a marked window and chain into another move, paying its cost and resetting the timeline from that point. This is how combos, frame traps, and blockstring pressure exist (full mechanics in L3.4). The engine-side contract:

- Cancels are only offered when `T` is inside a `cancel_window` of your in-flight move **and** the cancel's gating condition is met (e.g., "on hit only," "on block only," "always," "only if it connected").
- Accepting a cancel **truncates the current move's recovery** and starts the new move's startup immediately at `T`. This is exactly why canceling a move's recovery into another move creates plus frames / combos.
- Declining lets the move finish normally.
- ⚠️ The hardest fairness question in the whole system lives here: **in the NEUTRAL regime, do you get to see the opponent's revealed move before deciding your cancel?** See 2.10 — this is the one place the simultaneous model needs a careful rule, and I've specified a default.

## 2.4 `resolve_contact` — what happens when a hitbox meets a hurtbox ✅

When `does_hit` is true at tick `T`, the defender's *current state* decides the branch:

```
resolve_contact(attacker, defender, T):
  if defender is INVULN to this type:        → whiff (no effect)        # i-frames win
  if defender is in PARRY window:            → PARRY (2.6)              # parry beats strike
  if defender is BLOCKING (correct height):  → BLOCK (2.5)              # block beats strike (chip/stun)
  if defender is BLOCKING (wrong height):    → it's a HIT (mixup landed)
  if defender has ARMOR (hits remaining):    → ARMORED (take dmg, no stun, continue)
  else:                                      → HIT (2.7)
  # throws are resolved separately (2.6) because they ignore block
```

This ordered branch *is* the interaction priority table. Read top to bottom, it encodes "invincibility > parry > block > armor > clean hit."

## 2.5 Blocking ✅

- Blocking is a **stance**, itself a move with a FrameProfile: `startup` (a few ticks to raise guard — you cannot block instantly, which is what makes mixups work), `active` (holding guard, can be held across ticks), `recovery`.
- Block has a **height requirement**: `HIGH`/`MID` blocked by standing block, `LOW` blocked by crouch block, `OVERHEAD` only by standing, `THROW` not blockable at all. Guessing wrong = clean hit. This is the strike/throw/high/low **mixup**.
- On a blocked hit: defender takes **chip damage** (small, → a resource, not HP — see L3 poise), enters **blockstun** for `blockstun` ticks, and the attacker gets `on_block` frame advantage.
- **Guard meter / Guard break:** repeated blocking drains a **Poise/Guard** resource. At zero → **GUARDBROKEN** (a long fully-punishable stun). This prevents "just block forever" and rewards offense. (Resource defined in L3.)

## 2.6 Throws and parries — closing the RPS ✅

The classic fighting-game triangle, preserved exactly:

| If attacker does… | …and defender is… | Result |
|---|---|---|
| **Strike** | Blocking | Blocked (attacker safe-ish, chip) |
| **Strike** | Parrying | **Parried** (attacker fully punishable) |
| **Throw** | Blocking | **Thrown** (throws beat block) |
| **Throw** | Throwing (same tick) | **Throw tech** (clash, both reset, no damage) |
| **Strike** | Throwing | **Strike wins** (throw has startup; strike counter-hits the throw whiff) |

- **Parry:** a move with a *short active window* and the `guard-point`/parry property. If a strike connects during the window → attacker is frozen in a long recovery (huge plus for defender). High risk (tight window, bad if you guess throw), high reward. ⚠️ Whether parry is a pure-defense option or also a resource generator (e.g., refunds Focus) is a tuning lever — default: parry refunds a small amount of Focus to reward the read.
- **Throws** ignore block, have short range (`max_range` small), and **cannot be canceled into** (no throw combos from nothing). They are the answer to a turtling/blocking opponent. They lose to strikes (startup) and to backdash (spacing) and to jump (throws are usually grounded). This is what keeps any single defensive option from dominating — the balance argument is in §B.
- **Sidestep** (1.1, 1.3) is the *lateral* defensive option: it dodges **LINEAR** strikes entirely (they whiff via the lateral check in `does_hit`) and sets up a whiff-punish, but loses to **TRACKING/HOMING** strikes and does nothing against throws (throws are short-range and realign on auto-facing). So the attacker's counter to a step-happy defender is to mix in homing moves — which are typically slower or weaker per the budget (4.5), so they pay for that coverage. This is the Tekken layer of the RPS.

> This interaction (**strike / throw / block / parry / backdash / sidestep**) is the rock-paper-scissors that makes the neutral game non-trivial. Each defensive option is beaten by something: block loses to throw, parry loses to throw/empty, backdash loses to advancing/long moves, sidestep loses to homing. The audit (§B) checks that none strictly dominates.

## 2.7 Hit, hitstun, counter-hit ✅

On a clean hit:
- Defender takes `hit_effect.damage` (L4 scales this), enters **hitstun** for `hitstun` ticks, may be pushed back (`knockback`) or launched (`AIRBORNE`).
- Attacker gets `on_hit` frame advantage (per invariant I-1).
- **Counter-hit (CH):** if the defender was in a `counter-hit state` (i.e., in startup/recovery of their *own* move) when hit, apply `ch_damage_mult` (default ×1.25) and `ch_hitstun_bonus` (default +6 ticks). CH is what rewards whiff-punishing and frame traps — it's the payoff for winning the timing read. 🔬 Universal in fighters.

## 2.8 Juggles and knockdown ✅

- A **launcher** puts the defender in `AIRBORNE`. While airborne they can be hit again (juggle).
- **Gravity scaling / juggle decay:** each successive hit in a juggle applies a decreasing damage multiplier (`juggle_damage_decay`, default 0.9^n) and reduces remaining hitstun, so combos terminate. This is the anti-infinite rule. (Consistency check in §B: confirm no move loops into itself with net ≥0 advantage and no decay — that's the infinite-combo failure mode.)
- **Knockdown → Okizeme:** a knockdown puts the defender `DOWN` with a wake-up timer. The attacker is actionable first (pressure regime) and can set up a mixup on wake-up. Defender gets a **wake-up reversal** option (an invincible-startup move, if they have one and can pay for it) — the classic "get-off-me" tool. This keeps offense from being a free win after one knockdown.

## 2.9 Projectiles / zoning ✅

A projectile is an **independent timeline entity** spawned by the `projectile-spawn` property: it has its own `pos`, velocity, lifetime, and a one-active-tick-per-cell hitbox. It lives on the same global clock. This gives you zoning (throw fireball, control space) and the counterplay (jump over, reflect via guard-point, race it with your own). ⚠️ Follow-up only if you want projectile-heavy archetypes; otherwise the default single-projectile-per-caster rule is enough.

## 2.10 The one genuinely hard rule: information in the neutral regime ✅ (with ⚠️ flag)

In the PRESSURE regime there's no ambiguity — the actor sees everything. The subtlety is **NEUTRAL**, where both commit hidden. The question your step (3) raises: *after the reveal, can I cancel in reaction to what I now see?* If yes without limit, the second-mover always wins (no real commitment). If never, you lose the fighting-game "confirm" (hit-confirming a combo only when it actually lands).

**Specified default — the "lock then confirm" rule:**
1. Both players commit hidden (the *initial* move only). Reveal.
2. The timeline runs. **You may only cancel at a cancel-window checkpoint, and a cancel that is gated `on_hit`/`on_block` is decided by the actual contact result** — i.e., you *can* hit-confirm (cancel only if it connected) because by then the contact is a fact, not a read. But you **cannot** retroactively change your *initial* committed move after the reveal.
3. To prevent "react to reveal by canceling startup," **startup cancels are disallowed by default** (you can only cancel from active/recovery, i.e., after your move has committed to contact). Special moves may be flagged `startup-cancelable` as a deliberate, costed exception.

This preserves genuine commitment (you chose your poke blind) while keeping the skill of confirming (you only spend resources to combo when the hit is real). 🔬 This mirrors how confirms work in real fighters: you react to *hit/block*, not to the opponent's input. ⚠️ Confirm you're happy with "no startup cancels by default"; it's the cleanest anti-degenerate rule but it's a real design choice.

---

# LAYER 3 — MOVESET, RESOURCES, CANCELS

## 3.1 Resources ✅

Resources gate moves and create pacing. Four core meters (modular: add/remove without touching the engine, since the engine only reads `resources[name]`).

| Resource | Regenerates? | Spent on | Purpose / what it prevents |
|---|---|---|---|
| **Stamina** | Yes, per tick when NEUTRAL (not while attacking) | Most attacks, dashes, blocking-while-hit | Prevents mashing; spacing to recover stamina becomes a decision. Soulslike flavor. |
| **Poise (Guard)** | Slowly; resets on knockdown | Absorbing blocked hits | Guard-break system (2.5); caps pure turtling. |
| **Focus** | On parry/CH/whiff-punish (skill-rewarding) | Special moves, cancels, reversals | The "earned offense" meter; ties skill expression to resource. |
| **Action Points (AP)** | Refreshes each time you (re)gain initiative, up to `AP_max`; some moves *generate* AP (3.5) | Every action has an AP cost; long pressure/combo strings spend more | The **action-economy / tempo** layer (3.5): caps how much you do per turn; AP-generating moves are the reward for clever sequencing. |
| **Health (HP)** | No (only via items/skills out of combat) | — | Lose condition. |

🔬 Stamina-gated attacking + a poise/guard system is the recognizably *Dark Souls* layer grafted onto fighting-game frame data — which is exactly the fusion you described.

> **Balance rule R-1 (resource economy):** every offensive option costs a resource that is hardest to regenerate while attacking, and every defensive option either costs or drains one. There is **no zero-cost dominant action**. The audit verifies this per-archetype.

## 3.2 Move taxonomy ✅

| Class | Typical frame shape | Resource | Gated by (L4) | Role |
|---|---|---|---|---|
| **Normal (light)** | fast startup (3–6), low recovery, small `on_block` (−2..+1) | Stamina (low) | weapon proficiency | pokes, neutral, frame traps |
| **Normal (heavy)** | slow startup (12–20), high reward, minus on block | Stamina (high) | Strength req | whiff-punish, CH fishing |
| **Command normal** | mid startup, special property (overhead/low) | Stamina | weapon | mixup tools |
| **Special** | varies; unique properties (i-frames, armor, projectile) | Focus | Skill rank + Focus | archetype identity |
| **Reversal** | invincible startup, huge recovery if whiffed | Focus (high) | Skill + Focus | wake-up / get-off-me |
| **Throw** | short startup, short range, unblockable | Stamina | Strength (for damage) | anti-turtle |
| **Movement** | see L1 | Stamina (small) | speed_stat | spacing |

## 3.3 The MoveList (interface to L4) ✅

```
MoveList = Move[]
Move {
  id, name, class
  base_profile : FrameProfile          # the move's "naked" frame data
  requirements : { attr/skill/equipment gates }   # L4 decides if usable
  scaling      : ScalingRule[]          # how L4 stats modify base_profile → resolved profile
}
```

**This is the entire contract between the RPG layer and the engine.** L4 takes `base_profile`, applies `scaling`, checks `requirements`, and emits a *resolved* `FrameProfile` the engine runs. The engine never sees a stat.

## 3.4 Cancels — full rules ✅

```
CancelWindow {
  from_tick, to_tick           # relative to the move
  gate : ON_HIT | ON_BLOCK | ON_CONTACT | ALWAYS | ON_WHIFF
  into : [move_ids] | CATEGORY # what you may cancel into
  cost : ResourceCost          # usually Focus; this is why combos are finite
}
```

- **Chain/link cancels** (light→light) build small combos cheaply (Stamina).
- **Special cancels** (normal→special) are the bread-and-butter combo enabler, cost Focus.
- **Super/Reversal cancels** cost the most.
- **Whiff-cancel** (cancel the recovery of a *whiffed* move) is powerful and should be rare/expensive — flagged for the audit as a degeneracy risk.

**Combo termination is guaranteed by four independent governors** (defense in depth, so no single tuning miss creates an infinite): (1) Focus cost per cancel; (2) juggle damage/hitstun decay (2.8); (3) **hitstun decay** — each chained hit slightly reduces the next move's effective `on_hit`, so eventually advantage goes negative and the combo *must* end; and (4) **AP exhaustion** (3.5). The audit checks all four are present and that no loop nets ≥0 advantage at zero net resource cost.

## 3.5 Action Economy — Action Points (AP) ✅

This is the requested action-economy layer. It is a **distinct strategic axis from the tick clock**, and deliberately does *not* duplicate any existing resource. Keep this division clear:

- **Ticks** = the *physics*. Real time on the shared clock — governs spacing, whiff-punishing, who is actionable. (L0–L2.)
- **AP** = the *tempo / turn-budget*. A TTRPG-style action economy that governs **how many actions you may chain before initiative is re-evaluated**, independent of raw time.
- **Focus** = *access to power*. Whether you can afford a given special at all.
- **Stamina** = *exertion*. Whether your body can keep swinging.

A combo/pressure string can therefore end for four *strategically different* reasons — out of AP (tempo), out of Focus (can't afford the next special), out of Stamina (exhausted), or hitstun decayed (the next hit won't connect). Each answers a different question, so they enrich rather than overlap.

### 3.5.1 The model ✅

```
AP fields on every move (part of ResourceCost):
  ap_cost : int    # AP consumed to perform this action (≥ 0)
  ap_gain : { amount: int, gate: ON_HIT | ON_CH | ON_BLOCK | ON_PARRY | ALWAYS }
                   # AP generated, conditionally — this is "moves that generate extra AP"

Per entity:
  ap        : current pool
  AP_max    : cap (stat-derived, 4.2)
  ap_refill : amount restored each time you (re)gain initiative (default = AP_max; see tuning)
```

**How it plays, by regime** (ties directly to 2.1):

- **NEUTRAL (simultaneous):** you commit **one** action. It spends its `ap_cost`. You do **not** chain in neutral because the opponent is also free — there's no locked target to string against. So AP barely constrains neutral; it mostly *accrues* there.
- **PRESSURE (you have initiative on a locked opponent):** this is where the action economy lives. You may **chain a string of actions**, paying `ap_cost` for each, **as long as you can afford the next one and a cancel/link window allows it**. When you can't (or choose not to) pay, your turn yields and initiative re-evaluates by `ready_tick`. This is exactly how combos and blockstring pressure are bounded: **a long combo is literally an AP expenditure.**

### 3.5.2 Moves that generate AP — the requested mechanic ✅

Certain moves carry `ap_gain`, refunding/generating AP under a condition. This creates the core decision you asked for — *spend a big AP-sink finisher now, or play an AP-positive link to keep the string alive for more*:

| Example move | ap_cost | ap_gain | Effect on play |
|---|---|---|---|
| **Light poke** | 1 | +1 `ON_HIT` | Net-neutral *on hit* — confirms keep your turn alive; whiffs cost you tempo. |
| **Tempo jab / "gap-closer"** | 1 | +2 `ON_CH` | Rewards frame traps: landing a counter-hit *refunds* tempo, extending pressure. |
| **Heavy finisher** | 3 | 0 | Big payoff, ends the string (AP-negative by design). |
| **Special (launcher)** | 2 | +1 `ON_HIT` | Enough refund to permit *one* juggle follow-up, not an endless loop. |
| **Parry** | 0 | +2 `ON_PARRY` | A read-based parry *generates* tempo, turning defense into a long punish turn. |
| **Perfect block / just-guard** | 0 | +1 `ON_BLOCK` | Defensive skill expression: a tightly-timed block banks tempo for your own offense. |

> **Design intent:** AP-generation is **conditional on success** (mostly `ON_HIT`/`ON_CH`/`ON_PARRY`), never unconditional. You earn extra actions by *playing well*, not by mashing. This is the "skill → reward" loop expressed in the action economy, paralleling how Focus is earned.

### 3.5.3 The anti-degeneracy rule ✅ (critical for balance)

> **Balance rule R-5 (no net-positive AP loop):** for any sequence of actions that can chain into itself, the **sum of `ap_gain` must be strictly less than the sum of `ap_cost`.** Equivalently: no move may refund ≥ its own cost on a gate it can satisfy by hitting the *same* follow-up it loops into. Concretely — a move with `ap_gain ≥ ap_cost` may **not** be in its own (transitive) cancel/link target set.

This guarantees every string is finite *on the AP axis alone*, independent of the other three governors. It's the action-economy analogue of juggle decay, and the audit (§B, C-10) checks it by scanning the cancel graph for positive-weight cycles.

### 3.5.4 Stat tie-in (resolved cleanly, no double-dip) ✅

Per **balance rule R-2** (one major lever per attribute), AP is assigned to the attribute that was thinnest in combat before: **Charisma**. This gives CHA a real, distinctive identity — *the tempo / action-economy attribute* — rather than being a near-dump stat. CHA now governs `AP_max` and `ap_refill` (a high-CHA "tempo" build out-*actions* you), which sits coherently beside CHA's existing pressure/feint/intimidate kit. See updated 4.2. ⚠️ The exact curve (`AP_max = 3 + tier(CHA)`, `ap_refill`, whether unused AP partially carries over) is a playtest tuning table — flagged, not asserted.

---

# LAYER 4 — THE RPG LAYER (stats → frame data, and gating)

This layer answers your point 3: **stats feed into the system AND gate moves, D&D-style, with equipment.** Base system is Worlds Without Number-flavored (your stated lean), adapted so its outputs are *frame-data modifiers* rather than d20 attack rolls — because in this game, "did I hit" is decided by **spacing and timing (the engine), not a die.** That's the key adaptation and I want to flag it explicitly.

## 4.1 Why dice mostly leave combat ✅ (important design decision)

🔬 In WWN, skills use **2d6 + attribute mod + skill rank** vs. a target number, and combat uses a d20 to-hit vs. AC. **In this game, the engine already decides hit/miss deterministically via the timeline + hitboxes.** Re-rolling a d20 on top would double-resolve and destroy the fighting-game skill expression (you could whiff-punish perfectly and still "miss" to a die — anti-fun).

**Decision:** combat hit/miss is **deterministic** (engine). Dice are retained for:
- **out-of-combat skill checks** (2d6 + mod + rank, pure WWN) — exploration, social, crafting;
- **damage variance** (optional small roll, default OFF for competitive clarity, ON for PvE flavor — ⚠️ your call, see 4.6);
- **opposed checks that aren't spatial**, e.g., the *grapple/throw escape* contest (WWN has a nice opposed-Strength grapple 🔬 — we reuse it for throw-tech difficulty vs. heavier foes).

This keeps WWN's *character* (low, meaningful modifiers; reliable skills; scary combat) while letting the engine own timing. ⚠️ If you'd rather keep a to-hit roll for fidelity to TTRPG feel, that's a real alternative — but I recommend deterministic, and the rest of the spec assumes it.

## 4.2 Attributes ✅

Six attributes (WWN uses STR/DEX/CON/INT/WIS/CHA). Modifiers are **low** (WWN-style: roughly −2..+3), and *that low range is a feature* — it keeps frame-data swings small enough that the engine stays the star. Each attribute maps to **specific frame-data levers**, so building a character literally re-shapes your frame data:

| Attribute | Combat lever (how it edits FrameProfile / resources) | Out-of-combat |
|---|---|---|
| **STR (Strength)** | +damage on heavies/throws; raises **armor_hits** budget on armored moves; gates heavy weapons; throw-tech contest. | carry, force |
| **DEX (Dexterity)** | **−startup** on lights/normals (faster pokes); +movement `advance`; gates fast weapons; widens cancel windows slightly. | stealth, acrobatics |
| **CON (Constitution)** | +max HP; +max **Stamina**; +**Poise** (guard durability). | endurance |
| **INT (Intelligence)** | +max **Focus**; unlocks/empowers "technique" specials (e.g., charge moves, traps); −Focus cost on cancels. | lore, tech |
| **WIS (Wisdom)** | **Defensive reads**: widens **parry** window; +Focus refund on parry/CH; better wake-up timing. | perception (the WWN *Notice* skill) |
| **CHA (Charisma)** | **Tempo / action economy**: governs `AP_max` and `ap_refill` (3.5) — high-CHA builds out-*action* you; plus feint specials, guard-meter chip bonus, "intimidate" debuffs. The dedicated *tempo* attribute. | social |

> **Balance rule R-2 (one-lever-per-attribute, no double-dipping):** each frame-data lever is driven by **exactly one** attribute, and offensive levers (STR damage, DEX speed) are balanced against defensive levers (CON/WIS survivability) so a pure-offense build is *faster but frailer*, never strictly better. The audit verifies no attribute touches both a major offensive and a major defensive lever (that would create a dominant stat).

⚠️ **Follow-up — derived modifier curve.** I'm using WWN's *low* modifier philosophy. Exact mapping (e.g., "DEX +1 = −1 startup tick on lights, capped at −3") needs a tuning table I can draft once you confirm the modifier range and the tick budgets in 4.5. This is genuinely a playtest-tuned number, not something to assert blind.

## 4.3 Skills and Foci ✅

🔬 WWN skills are **rank 0–4**, bought with skill points, added to 2d6 checks. We keep that, plus WWN's **Foci** (feat-like talents) as the **moveset gate**:

- **Combat skills** (Stab/Punch/Shoot analogues → here: per weapon-class proficiency, rank 0–4). Rank does **not** add a to-hit (no die); instead **rank unlocks moves and improves their frame data** (higher rank → access to that weapon's heavy/special moves, and better `on_block` safety). This is how "skills gate moves" is realized concretely.
- **Foci** = the build-defining unlocks. A Focus is what grants an *archetype's signature mechanic* (e.g., *Iron Guard*: gain armor property on a blocked-to-counter move; *Whirlwind*: a multi-hit special; *Read the Wind*: WIS-scaled parry). Foci are the **modular content slots** — new archetypes = new Foci + new Moves, zero engine changes.

> **Balance rule R-3 (gates are floors, not multipliers):** a requirement either lets you use a move at its designed numbers, or you can't use it. Meeting a requirement *more* (e.g., STR far above a heavy weapon's req) gives a **small, capped** bonus, never runaway scaling. This stops "dump everything into one stat" from snowballing.

## 4.4 Equipment ✅

Equipment is a **second compiler into FrameProfile**, stacking after stats. Modular: an item is just a bundle of frame-data deltas + a MoveList contribution + requirements.

```
Weapon {
  class            # dagger / sword / greatsword / spear / fists / bow ...
  grants_moves     # the MoveList this weapon contributes
  frame_deltas     # global +startup/−recovery/±range/±damage applied to its moves
  reach_profile    # this is where weapon RANGE lives — spacing identity!
  requirements     # STR/DEX gates
  resource_mods    # e.g., greatsword: +damage, +stamina cost, −speed
}
Armor {
  poise_bonus, hp_bonus
  speed_penalty    # heavier armor → +startup / −movement (the tradeoff)
  damage_resist
}
Accessory/Talisman {
  resource_mods, special_property_grants   # e.g., +1 armor_hit, +Focus regen
}
```

**Weapon = your spacing identity.** A spear has long `max_range`/`min_range` (great poke, weak up close); a dagger is short-range, fast startup, low damage; a greatsword is slow, huge, plus-on-block-if-spaced. This is the single most important RPG-into-engine coupling: **choosing a weapon chooses your frame data and your preferred range**, exactly bridging "RPG build" and "fighting-game character."

> **Balance rule R-4 (the universal tradeoff triangle):** for any equipment, **range ↔ speed ↔ damage** — improving one worsens at least one other, weighted by requirements/weight. No weapon may be top-tier in all three. The audit scores every weapon on this triangle and flags any that Pareto-dominate another.

## 4.5 The frame-data budget — how balance is *enforced*, not hoped for ✅ 🔬

This is the mechanism that makes "please make it balanced" a property of the system rather than a wish. Every move must satisfy a **points identity**: its strengths must be paid for by weaknesses.

```
MOVE_VALUE(move) =
      w_speed   * (BASELINE_STARTUP - startup)      # faster = costs points
    + w_safety  * on_block                           # plus-on-block = costs points
    + w_reward  * expected_damage                     # more damage = costs points
    + w_range   * reach_advantage                     # more range = costs points
    + w_props   * Σ property_values                   # i-frames/armor = costs points
    - w_cost    * resource_cost                        # paying resources REFUNDS points
    - w_commit  * total_frames_if_whiffed              # being punishable REFUNDS points

CONSTRAINT:  MOVE_VALUE(move)  ≈  ARCHETYPE_BUDGET   (within ±ε for every move)
```

In words: **a move that is fast, safe, long-range, damaging, and propertied must be either resource-expensive or horribly punishable on whiff — or it violates the budget and gets flagged.** This is the formal version of fighting-game design wisdom ("everything good must have a real downside"). It also gives you a *tool*: to add a new move, solve for the weakness that balances its strengths.

> The weights `w_*` are the master tuning knobs. Set them once per game-feel target; then balance becomes "does every move hit budget?" — checkable by script. ⚠️ Initial weights need playtesting; I can propose a starting set and a spreadsheet model on request (this is exactly the iterative frame-balancing real fighters like Tekken do continuously — it's expected, not a gap).

## 4.6 Damage, HP, and the WWN "scary combat" feel ✅

- **HP** is low-ish and slow to restore (Soulslike). Damage numbers come from `hit_effect.damage × STR-scaling × combo-decay`.
- 🔬 WWN's **Shock damage** (unarmored = scary) ports as: low-Poise/low-armor targets take **bonus chip and bonus CH** — being under-armored is genuinely dangerous, matching both WWN and Dark Souls.
- ⚠️ **Damage variance default OFF** for PvP clarity (deterministic combos), **ON (small 2d6-flavored roll)** as a PvE option. Your call; flagged in 4.1.

---

# WORKED EXAMPLE — proving the engine actually plays

Two characters. **Reza** (DEX dagger, fast/frail) vs. **Borin** (STR greatsword, slow/armored). One full exchange, tick by tick, to show neutral → pressure → punish all emerge from §2.

**Setup:** Both NEUTRAL, `ready_tick = 0`, `T = 0`, spaced just inside greatsword range.
- Reza commits **Light Slash** (startup 4, active 2, recovery 6; `on_block −1`, `on_hit +3`; short range).
- Borin commits **Heavy Cleave** (startup 14, active 3, recovery 18; `on_block −6`, `on_hit +5`, armor[4..14]; long range). Both hidden → reveal.

| T | Reza | Borin | Engine |
|---|---|---|---|
| 0 | commit Light (st4) | commit Heavy (st14) | NEUTRAL: both hidden, revealed |
| 4 | **active** | startup | `does_hit(Reza→Borin)`? In range → **HIT**… but Borin's **armor** window is [4..14] → **ARMORED**: Borin takes dmg, *no hitstun*, Heavy continues |
| 6 | recovery (6t) | startup | Reza now locked in recovery until T=12 |
| 12 | actionable (ready=12) | startup (active at 14) | **PRESSURE regime flips to Reza**: he's free, sees Borin locked till active@14. He reads the incoming Heavy. |
| 12 | commits **Backdash** (st3, iframes 13–16) | — | Reza chooses to escape rather than trade |
| 14 | backdash active (moved out of range) | **active** (Cleave) | `does_hit(Borin→Reza)`? Reza out of `max_range` → **WHIFF**. `on_whiff=0` → Borin eats full 18 recovery |
| 17 | actionable @ ~17 | recovery till 35 | **PRESSURE flips to Reza**: Borin locked 18t. Reza **whiff-punishes**. |
| 17 | commit **Light → cancel → Special** (confirm) | DOWN soon | Light hits a recovering Borin = **counter-hit** (×1.25, +6 stun); hit-confirm cancel into Special (pays Focus); combo, juggle decay caps it; knockdown |
| ~40 | okizeme pressure | wake-up; may reversal (Focus) | Knockdown → Reza sets mixup; Borin's get-off-me option keeps it fair |

**What this demonstrates (maps to the audit):**
- Neutral mind-read, armor trading, **spacing-based whiff**, regime flips, **whiff-punish + counter-hit payoff**, hit-confirm cancel, juggle decay, okizeme + reversal — *all emerge from the single `ready_tick` rule and the contact resolver.* No bespoke logic per situation. That's the consistency proof.
- The DEX/frail vs STR/armored identities came entirely from L4 compiling different frame data — the engine treated both identically.

### Addendum — sidestep evasion + the AP action economy

A second short exchange to show the two new systems. Borin (now playing a high-CHA *tempo* variant, `AP_max = 5`) has initiative on a blocking Reza (PRESSURE regime).

| T | Action | AP | Lateral / tracking | Engine |
|---|---|---|---|---|
| — | Borin starts string, AP = 5 | 5 | — | has initiative on locked Reza |
| 0 | **Tempo jab** (LINEAR, ap_cost 1, +1 ON_HIT) | 5→4, hits → +1 → **5** | on-axis, connects | net-neutral confirm — turn stays alive |
| 6 | cancel → **Light** (LINEAR, cost 1) | 5→4 | Reza **sidesteps** (offset > band) | `does_hit`? lateral fails → **WHIFF**. Reza dodged the linear follow-up |
| — | Reza now off-axis, Borin mid-whiff | — | — | Reza can whiff-punish the linear miss — sidestep = whiff-punish setup |
| (alt) 6 | cancel → **Homing sweep** (HOMING, cost 2, +0) | 4→2 | `step_in` realigns to Reza's offset | **HITS** despite the step — homing is the counter to step-happy defense, but cost 2 + slower bleeds the budget (4.5) |
| 12 | **Heavy finisher** (cost 3, gain 0) | 2 < 3 → **cannot afford** | — | **AP exhaustion (governor 4)** ends the string. Initiative re-evaluates by `ready_tick`. |

This shows: (1) **sidestep dodges linear, loses to homing**, routed entirely through `does_hit`'s lateral clause — no new engine path; (2) **AP-positive links** (the +1 ON_HIT jab) keep a turn alive while **AP-negative finishers** end it; (3) the string terminated on the **AP axis** even though Focus/hitstun hadn't run out — the four governors are genuinely independent.

---

# APPENDIX A — MODULE / FILE MAP (for implementation, honoring your modularity preference)

```
/core
  tick.ts            # T, scheduler, advance_until_next_decision()   [L0/L2]
  frameprofile.ts    # FrameProfile, Property, invariant I-1 checks  [L0]
  entity.ts          # Entity, state machine                         [L0]
  resolver.ts        # resolve_contact, interaction priority (2.4)   [L2]
  regime.ts          # NEUTRAL/PRESSURE decision (2.1)               [L2]
/spatial
  lane.ts            # pos (lane) + offset (sidestep) + height; does_hit w/ tracking (1.1–1.2)  [L1]
/moves
  move.ts            # Move, MoveList, CancelWindow                  [L3]
  resources.ts       # Stamina/Poise/Focus/AP/HP                     [L3]
  economy.ts         # AP costs/gains, R-5 no-positive-cycle check (3.5)  [L3]
/rpg
  sheet.ts           # attributes, skills, foci                      [L4]
  compiler.ts        # stats+equipment → resolved FrameProfile       [L4]  ← the only L4→L2 bridge
  equipment.ts       # weapons/armor/accessories                     [L4]
/balance
  budget.ts          # MOVE_VALUE identity + linter (4.5)            [tooling]
  audit.ts           # runs §B checks as automated tests
```

The **single bridge** is `rpg/compiler.ts → FrameProfile`. Nothing else in `/rpg` may import from `/core`. If that rule holds, you can rebuild the RPG or the engine independently — the modularity you wanted, enforced by import boundaries.

---

# APPENDIX B — CONSISTENCY & FUN AUDIT
*(You asked me to go back over the whole spec for consistency AND fun. This is that pass, done honestly — including the places it's not yet airtight.)*

## B.1 Consistency checks

| # | Claim | Verdict |
|---|---|---|
| C-1 | Frame advantage is never set independently of stun+recovery | ✅ Enforced by invariant **I-1** (0.2); authoring tool computes it. |
| C-2 | Neutral vs pressure never need special-case code | ✅ Both derive from `ready_tick` comparison (2.1). Worked example confirms. |
| C-3 | Hit/miss decided once, not twice | ✅ Engine is deterministic; dice removed from combat hit-resolution (4.1). No double-resolution. |
| C-4 | No infinite combos | ✅ **Four independent governors** (Focus cost, juggle decay, hitstun decay — 3.4/2.8 — plus **AP exhaustion**, 3.5). Quadruple redundancy is deliberate. |
| C-5 | Every layer's interface is one-directional | ✅ Only `compiler.ts` bridges L4→engine; import-boundary rule (App. A). |
| C-6 | The "react to reveal" exploit is closed | ✅ Lock-then-confirm + no startup cancels by default (2.10). |
| C-7 | Spatial model swap doesn't ripple | ✅ Lane + sidestep both go through the single `does_hit` predicate (1.2); the lateral axis is `spatial/lane.ts` only — L2 untouched. |
| C-8 | "2D / continuous grid" honored per your clarification | ✅ **Resolved** — continuous 1D spacing lane + Tekken lateral/depth `offset` for sidestep evasion (1.1). Spacing stays scalar (one-to-one frame translation intact); `offset` only gates linear-vs-tracking. |
| C-9 | WWN identity preserved | ✅ for skills/foci/saves/low-mods/shock; ⚠️ **diverges** by removing the d20 to-hit (4.1) — a deliberate, justified break you should sign off on. |
| C-10 | No infinite AP / net-positive tempo loop | ✅ **Balance rule R-5** (3.5.3): no move may sit in its own transitive cancel set with `ap_gain ≥ ap_cost`; audit scans the cancel graph for positive-weight cycles. |
| C-11 | Sidestep evasion has real counterplay (not a dominant defense) | ✅ Sidestep beats LINEAR, loses to TRACKING/HOMING and does nothing vs. throws (2.6); homing moves cost more on the budget (4.5). Folds into the defensive RPS. |

## B.2 Fun / depth checks (does it produce the *feeling* of a fighting game?)

| Pillar of fighting-game fun | Reproduced? | Where |
|---|---|---|
| **Neutral mind-game** (read/whiff-punish) | ✅ | NEUTRAL regime + spatial whiff (2.1, 1.2) |
| **Spacing matters** | ✅ | weapon reach = identity (4.4), movement has cost (1.3) |
| **Commitment & risk** | ✅ | every action has whiff recovery; budget rule (4.5) forbids no-downside moves |
| **Combos & execution → here, *planning*** | ✅ | cancels/confirms (3.4, 2.10); execution skill becomes *reading + sequencing*, which is the whole point of removing the timing constraint |
| **Defensive RPS** (block/parry/throw/step) | ✅ | 6-way interaction (2.6): block↔throw, parry↔throw, backdash↔advance, sidestep↔homing |
| **Lateral evasion / Tekken reads** | ✅ | sidestep dodges linear, homing punishes the step (1.1, 2.6); SSL vs SSR are distinct reads (`track_side`) |
| **Tempo / action economy** | ✅ | AP strings (3.5); spend-a-finisher-vs-extend-the-turn is a real decision; AP-positive links reward confirms |
| **Comeback / get-off-me** | ✅ | wake-up reversals (2.8), Focus-earned offense |
| **Build identity (RPG)** | ✅ | attributes edit frame data (4.2); weapon = spacing (4.4); CHA = tempo build (3.5.4) |
| **Soulslike weight** | ✅ | stamina-gated offense, poise/guard-break, scary-when-unarmored (3.1, 4.6) |

## B.3 Honest risks the audit surfaced (where it could become *un*-fun, and the mitigation)

1. **Analysis paralysis.** No time limit + full frame data visible could make matches glacial (the rpgcodex critique of simultaneous-turn games: "headless chickens looking for an opening"). **Mitigations baked in:** stamina/Focus regen only in neutral (rewards committing), guard-break (punishes infinite turtling), and the pressure regime giving the plus player a clear initiative. ⚠️ Still the #1 thing to playtest. A soft "intent timer" or an optional clock is a fallback lever if needed.
2. **Information asymmetry feel-bad.** In PRESSURE the actor sees *everything* the locked player will do. That's realistic (they're committed) but can feel oppressive on the receiving end. **Mitigation:** reversals + the fact that getting plus had to be *earned*. Tune knockdown advantage conservatively.
3. **The budget weights are unproven.** §4.5 makes balance *checkable* but the weights `w_*` and the DEX→startup curve (4.2) are real playtest numbers, not derivable a priori. I've been explicit rather than faking precision. This is normal for the genre (even Tekken re-tunes frame data every patch) — but it's the largest open work item.
4. **AP pacing interaction.** The action economy (3.5) is a *second* throttle on offense alongside the tick clock. If `AP_max`/`ap_refill` are set too low, pressure turns fizzle and combos feel anemic; too high and strings overstay. This is a new tuning surface introduced by the action economy — earmarked for playtest alongside the budget weights.
5. **Remaining crisp forks on you:** throw/armor (0.3), damage variance (4.6), the AP-model interpretation (see open items), startup-cancel rule (2.10), parry-as-Focus (2.6). The 1D-vs-2D fork is now **resolved** (Tekken lane+sidestep, 1.1).

## B.4 Verdict

Internally **consistent**: yes — the `ready_tick` regime rule + invariant I-1 + the single L4→engine bridge make the system coherent, and the worked example (incl. the sidestep/AP addendum) runs cleanly with no special cases. The four combo-governors, the no-positive-AP-cycle rule (R-5), and the budget identity make "balanced" a *checkable property*, not a hope. The Tekken sidestep and the AP economy both slot in without touching L2 — the lateral axis lives entirely in `does_hit`, and AP is an orthogonal tempo resource that doesn't duplicate ticks, Focus, or Stamina.

**Fun**: the structure reproduces every pillar of fighting-game depth (neutral, spacing, RPS, commitment, payoff), adds the Tekken lateral-evasion read and a genuine tempo/action-economy decision layer, and grafts a Soulslike-RPG build layer on top via the stats-compile-to-frame-data design. The main threat to fun is pacing (analysis paralysis), now with a second pacing knob (AP) that helps *and* must itself be tuned — the key playtest target.

---

# OPEN ITEMS FOR YOU (the explicit ⚠️ follow-ups, collected)

1. **Spatial model — RESOLVED** to Tekken lane + sidestep (1.1). No longer a fork; flagging only in case you want sidewalk to be *continuous-hold* (Tekken sidewalk) vs. a fixed-distance hop — I defaulted to both existing (1.3).
2. **AP model interpretation** — I read "action points" as a **tempo / turn-budget** resource (how many actions you chain before yielding initiative), distinct from ticks/Focus/Stamina (3.5). The main alternative reading is a **fixed per-turn AP budget like a tactics game** (e.g., "3 AP every turn, moves cost AP, no chaining concept"). I chose tempo because it integrates with the pressure regime and rewards your "moves generate AP" idea directly — but if you meant the tactics-budget version, say so and I'll re-cut 3.5. **(New fork from this round.)**
3. **AP stat home** — I assigned `AP_max`/refill to **CHA** to give it a combat identity without double-dipping (3.5.4). Confirm, or pick a different attribute / a new derived "Tempo" stat.
4. **Deterministic combat (recommended) vs. keep a to-hit roll** for TTRPG fidelity? (4.1, C-9)
5. **Damage variance** OFF (PvP) / ON (PvE)? (4.6)
6. **Armor vs. throws** — confirm throws beat armor. (0.3)
7. **"No startup cancels by default"** — confirm the anti-degeneracy rule. (2.10)
8. **Parry as resource generator?** — confirm Focus refund + the new AP refund on parry (2.6, 3.5.2).
9. **Tuning tables** — modifier→tick curves, budget weights `w_*`, and now the **AP economy numbers** (`AP_max`, `ap_refill`, per-move `ap_cost`/`ap_gain`). Want me to draft a starting spreadsheet model covering all three? This is the iterative-balance work, best done against your target game-feel.
