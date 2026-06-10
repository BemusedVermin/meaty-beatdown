# TICK — COMBAT SYSTEM SPECIFICATION v2
### A party-based, partial-information, turn-based JRPG battle system built on fighting-game frame data
*The imitated fighter is **Tekken**; the aesthetic register is **Dragonball / One-Punch Man**; the body is a JRPG.*

---

## How to read this document

This is a full rewrite of the v1 spec (in git history) incorporating the locked decisions of
2026-06-09. The layered architecture survives; the headline changes are:

1. **Partial information everywhere.** v1 hid commitments only in the simultaneous "NEUTRAL
   regime." v2 hides enemy intent *always*: you see authored **cues**, never committed moves, and
   a **matchup-knowledge** system turns those cues into data over a campaign. (§7)
2. **Party battles in a 3D arena.** v1 was two fighters on a 1D lane + sidestep offset. v2 is
   N actors per side on a true ground plane, where **every actor targets exactly one other and
   the target creates the lane**. All v1 lane math survives as the per-pair special case. (§3, §8)
3. **Tekken systems land in full**: real heights and crouching, throw breaks as a directional
   read, the launch → screw → bound → wall-splat juggle grammar, walls and authored stage
   hazards, Heat and Rage. (§5, §6, §9)
4. **Supers**: a buildable **Focus** gauge funding EX moves, cancels, and cinematic supers;
   beams and projectiles are first-class. (§10)
5. **Authored qualities, no engine constants.** Every combat magnitude lives in data — on a
   move, a hit, a fighter's defense profile, or the swappable **Ruleset** object. The engine is
   an interpreter with no numbers of its own. (§2.4)
6. **No infinite combos** is a charter with seven independent governors and an audit. (§6.5)

```
┌──────────────────────────────────────────────────────────────┐
│  L5  ENCOUNTER / AI        agents behind the Observation API   │
├──────────────────────────────────────────────────────────────┤
│  L4  RPG LAYER   Forms · attributes · equipment · affixes     │ → compiles Builds into Fighters
├──────────────────────────────────────────────────────────────┤
│  L3  MOVESET     moves · cancels · meters · Ruleset           │
├──────────────────────────────────────────────────────────────┤
│  L2  COMBAT ENGINE   shared tick · scheduler · resolver       │ ← the heart
├──────────────────────────────────────────────────────────────┤
│  L1  SPATIAL     ground plane · target-lanes · walls          │
├──────────────────────────────────────────────────────────────┤
│  L0  PRIMITIVES   tick · Move schema · Entity · Reactions     │
└──────────────────────────────────────────────────────────────┘
```

The load-bearing architectural rule is unchanged: **the RPG layer is a compiler that emits
fighter data; the engine is an interpreter that runs it.** L4 never reaches into L2. The new
second rule beside it: **the Observation API (§7.1) is the only window into a fight** — the UI
and the AI both live behind it, so the fog of war is an architectural boundary, not a UI habit.

Symbols: ✅ decided · 🔬 grounded in a real-game comparable · ⚠️ tuning value, playtest-owned.

---

# §1 DESIGN CHARTERS (the rules above the rules)

These are invariants. Content that violates them is wrong even if it's fun in isolation.

- **C-EXEC — No execution cost.** Skill is *what* you commit and *when on the tick stream*, never
  dexterity. Any mechanic whose only function is a reaction/precision barrier is translated away.
- **C-DET — Determinism.** A fight is a pure function of (initial state, content, decisions).
  No dice in combat resolution; integer/fixed-point math only; same inputs ⇒ byte-identical trace.
- **C-FOG — The fog never leaks, the forecast never lies.** Enemy *intent* is hidden; resolved
  *facts* are public. Anything shown to the player or the AI flows through Observation (§7.1).
  The prediction UI shows exactly what the engine would do against the last-observed world.
- **C-AUTH — Authored qualities.** No combat constant lives in engine code. Magnitudes live on
  moves, hits, fighters, arenas, or the Ruleset (§2.4). There are **no generic moves**: every
  move belongs to a Form and was authored with intent.
- **C-FIN — No infinite combos.** Seven governors (§6.5), each independently sufficient,
  mechanically audited. This is a charter item at the user's explicit insistence.
- **C-QUAR — Quarantine.** Fighting-game mechanics exist only inside combat. Exploration and
  menus never host frame data, drills, or previews (see `exploration.md`).

---

# §2 LAYER 0 — PRIMITIVES

## 2.1 The tick ✅

1 tick = 1 frame at 60 Hz ≈ 16.7 ms, so real frame data ports one-to-one. A single global counter
`T` is shared by **all** actors. The wall clock is irrelevant: the engine advances `T` only when
no actor needs to decide, and pauses indefinitely at every decision point.

## 2.2 The Move — orthogonal axes ✅

v1 conflated type, height, and blockability into one `MoveLevel` enum (and duplicated THROW
across two enums — a latent bug). v2 decomposes every move into **orthogonal axes**: each axis
answers exactly one question, and any combination is expressible.

```
Move {
  id, name, form_id                  # every move belongs to a Form (C-AUTH: no generic moves)
  tags        : [LIGHT|HEAVY|SPECIAL|SUPER|MOVEMENT|STANCE|RESCUE|BURST|REVERSAL|...]

  # ── the orthogonal type axes ─────────────────────────────────────────
  category    : STRIKE | THROW | PROJECTILE | MOTION | STANCE | UTILITY
  height      : HIGH | MID | LOW | NONE      # strikes/projectiles; NONE for motion/stance
  blockable   : bool                          # orthogonal to height (unblockables exist at any height)
  tracking    : LINEAR | TRACK_L | TRACK_R | HOMING     # vs lateral movement (§3.5)

  # ── timing & space ───────────────────────────────────────────────────
  timing      : { startup, active, recovery }            # ticks
  hits        : [HitEvent]                               # multi-hit moves are first-class
  region      : ReachEnvelope                            # §3.4
  motion      : SelfMotion                               # per-phase self-displacement (§3.6)

  # ── windows & costs ──────────────────────────────────────────────────
  properties  : [PropertyWindow]                         # §2.5
  cost        : { breath, ap, focus }                    # §9
  gains       : [{ resource, amount, gate }]             # gate ∈ ON_HIT|ON_CH|ON_BLOCK|ON_PARRY|ON_WHIFF_PUNISH|ALWAYS
  cancels     : [CancelWindow]                           # §11
  startup_cancelable : bool                              # default false (§11.3)

  # ── information & interaction ────────────────────────────────────────
  cue         : CueClass                                 # what enemies SEE (§7.2)
  reqs        : { stance?, state?, condition? }          # e.g. state=DOWN (wake-up), ALLY_IN_HITSTUN (rescue)
  break_key   : L | R | NONE                             # throws only (§5.4)
}

HitEvent {
  at          : tick offset(s) within the active window
  damage      : int
  chip_guard  : int                                      # drains the blocker's Guard meter
  reaction    : Reaction                                 # §6.1 — what a clean hit DOES
  ch_reaction : Reaction?                                # counter-hit override (e.g. "CH launcher")
  flags       : { friendly_fire (default false), back_bonus?, splat_eligible (default true) }
  gains_override : ...                                   # per-hit Focus/AP gain overrides
}
```

**Frame advantage is derived, never stored** (invariant I-1, inherited verbatim from v1):
`on_hit = defender_stun − attacker_remaining`; `on_block = blockstun − attacker_remaining`;
`on_whiff = 0` (you eat your full recovery and are exposed). The authoring pipeline computes
advantage and refuses inconsistent data — the engine can never lie to the player.

## 2.3 The Entity ✅

There is **no separate Fighter type** (locked decision): one Entity carries the compiled
offensive kit, the compiled defensive profile, and all runtime state.

```
Entity {
  id, side                            # sides drive win/loss (a side loses when all its actors are KO'd)
  pos        : FxVec2                 # position on the ground plane (fixed-point)
  height_off : Fx                     # vertical offset (airborne arcs, juggles)
  facing     : FxVec2                 # unit vector; auto-faces target when actionable
  target     : EntityId               # every actor targets exactly ONE other actor (§3.2)
  stance     : STANDING | CROUCHING | AIRBORNE | DOWN
  state      : FREE | STARTUP | ACTIVE | RECOVERY | HITSTUN | BLOCKSTUN
             | CRUMPLE | JUGGLE | GRABBED | DOWN | GUARDBROKEN | KO
  ready_tick : int                    # when this actor next gets a free decision
  current    : MoveInstance?          # in-flight move + contact bookkeeping + armor uses
  combo      : ComboTracker           # hits taken this combo, decay level, extender latches (§6)
  meters     : { hp, breath, guard, focus, ap }          # §9
  latches    : { heat: Unused|Active(until)|Spent, rage: Armed|Triggered|Spent, burst_used }
  statuses   : [StatusEffect]
  moves      : MoveList               # compiled by L4; the engine treats it as opaque data
  defense    : DefenseProfile         # compiled static ++ dynamic-from-statuses (see below)
}

DefenseProfile {
  weight        : juggle gravity/decay modifier (heavier = shorter juggles) 🔬 Tekken body weight
  block_arc     : frontal arc within which guarding works (§5.3)
  guard_max, breath_max, ap_max, focus_max, hp_max
  ch_vulnerability : multiplier hooks (statuses can raise it)
  visibility    : per-meter visibility flags (what enemies may observe — default: HP only) ✅tunable
}
```

`ready_tick` remains the single idea that turns continuous combat into turns: **the engine
always advances to the earliest pending decision and asks that actor's owner to choose.**

## 2.4 The Ruleset — where "universal" numbers live ✅ (C-AUTH)

Cross-cutting curves can't live on any one move, but they may not live in engine code either.
They live in a **Ruleset**, a content object loaded with the fight — swappable, versioned, and
auditable like any other data:

```
Ruleset {
  hitstun_decay   : per-combo-hit stun reduction schedule        # governor 1
  juggle_decay    : per-juggle-hit damage schedule (× defender weight)  # governor 2
  extender_latches: { screw: 1, bound: 1, wall_splat: 1 }        # per-combo allowances, governor 3
  ch_default     : { damage_mult, stun_bonus }                   # used when a hit has no ch_reaction
  guard_break_stun, throw_tech_recovery, block_reevaluate_every  # ⚠️ all tuning values
  forced_landing : the juggle "gravity floor" rule (§6.5, governor 7)
  ...
}
```

Rule of placement: *an attack's effect* → on the HitEvent. *A defender's susceptibility* → in
DefenseProfile. *A property of the fight itself* → in the Ruleset. The engine holds **zero**.

## 2.5 PropertyWindows (frame flags) ✅

Unchanged in spirit from v1; each property is live during an inclusive tick window relative to
the move's start:

| Property | Effect during window | Ported from |
|---|---|---|
| `INVULN{type}` | Matching attacks pass through (ALL / STRIKE / THROW / PROJECTILE). | reversals, backdash i-frames |
| `ARMOR{hits, dmg_mult, covers}` | Absorb N hits of covered heights without stun; still take scaled damage. Throws and (by default) LOWs go through. | 🔬 Tekken Power Crush |
| `GUARD_POINT{covers}` | Auto-deflect one covered strike → parry outcome (§5.5). | 🔬 Tekken sabaki |
| `CH_STATE` | Being struck here is a counter-hit (extends the startup/recovery default). | universal CH rules |
| `CANCELABLE` | See §11. | combo cancels |
| `HEAT_ENGAGER` | On hit, the attacker enters Heat (§9.5). | 🔬 Tekken 8 |
| `PROJECTILE_SPAWN{spec}` | Emits an independent projectile entity (§10.3). | zoning |

---

# §3 LAYER 1 — THE SPATIAL MODEL (target-lanes in a 3D arena)

## 3.1 The arena ✅

The arena is a bounded region of a 2D **ground plane** (rendered in 3D; simulated in
fixed-point), plus a vertical axis used only for airborne arcs and height bands. An `ArenaDef`
authors: the floor boundary, **wall segments** (with per-segment properties: solid / splat-able /
breakable), and **hazard volumes** (authored contact events — e.g. *overboard*: damage + a
status + repositioning). Hazards are arena **data**, never engine rules. ✅

## 3.2 The target creates the lane ✅ (the core spatial idea)

Every actor has exactly **one target**. The segment from actor to target is that actor's
**lane**: all spacing mathematics — range, advance, knockback, "the gap" — runs along it, and
the lateral axis (sidestep evasion) is perpendicular to it. This is precisely v1's `pos`/`offset`
1D-lane model applied **per pair**: the two-fighter game is the special case where both lanes
coincide. The translation of Tekken footsies therefore carries over untouched; what's new is
that there are several lanes in the arena at once.

- **Facing**: an actionable actor auto-faces its target. Facing only changes while you are
  actionable (or via a move's authored motion) — a committed or reeling actor keeps its facing,
  which is what makes flanking and back attacks (§8.3) possible.
- **Retargeting**: choosing a target is part of committing any action ("do X *at* Y"). There is
  also a near-free `switch_focus` utility move (a few ticks ⚠️) for purely defensive re-facing.
- The chosen option is deliberately simple — *"everyone targets one person; the target creates
  the lane"* — and is flagged for revisit after the first party-combat playtest. ✅user-locked

## 3.3 `does_hit` — still one predicate ✅

The whole spatial model stays behind a single contact predicate. For each active HitEvent of
each attacker, against **every** legal victim (not just the attacker's target — bystanders can
absolutely be clipped by a wide cleave or a beam):

```
does_hit(attacker, move, hit, victim, T) -> bool
  (phase)    T is inside the hit's active ticks
  (type)     victim not INVULN to this category at T
  (range)    victim's position, projected onto the attacker's facing axis,
             lies within [min_range, max_range] (after the move's advance so far)
  (arc)      victim lies within the move's horizontal arc / lateral band about that axis
             — LINEAR moves have a narrow band; TRACK_*/HOMING widen or realign it (§3.5)
  (height)   victim's current height band (stance + height_off) intersects the hit's band
             — a HIGH whiffs entirely over a CROUCHING victim (§5.2)
```

A sidestep that takes you off an attacker's narrow LINEAR band routes through the exact same
`on_whiff = 0 → full recovery → exposed` path as a baited whiff: **lateral evasion is a
whiff-punish setup, structurally identical to footsies.** (Unchanged from v1 — audit C-7/C-8.)

## 3.4 ReachEnvelope ✅

```
ReachEnvelope {
  min_range, max_range      # along the facing axis (min_range > 0 = "whiffs point-blank")
  arc_halfwidth             # lateral half-width at max_range (narrow = LINEAR-feeling)
  height_band               # LOW / MID / HIGH coverage + anti-air extents
  advance                   # ground covered during startup+active (committal forward motion)
  step_in, track_side       # tracking realignment (§3.5)
}
```

Beams (§10.3) are just very long, very narrow envelopes. Sweeps are short, very wide arcs that
naturally hit multiple actors. Party-relevant region size is **priced by the budget** (§13).

## 3.5 Tracking vs lateral movement ✅ 🔬

The Tekken triangle, generalized to the plane:

- **LINEAR** — narrow band; a sidestep (perpendicular hop relative to *the attacker's* lane)
  evades it. Cheap on the budget.
- **TRACK_L / TRACK_R** — realigns against steps to one side only; stepping the *other* way
  evades. The asymmetric read (SSL vs SSR are different guesses) is reproduced exactly.
- **HOMING** — realigns both ways; beats stepping outright, pays for it on the budget and is
  usually slower or weaker.

## 3.6 Movement ✅

Movement is just a move with a FrameProfile (so it lives on the timeline and is whiff-punishable):
step, dash, backdash (early i-frames 🔬), sidestep L/R, sidewalk, crouch (a held stance, §5.2),
jump (enters AIRBORNE; the air game beyond juggles is deliberately out of slice scope ⚠️).
Distances are authored per move and scaled by the compiled speed lever. Because movement spends
ticks on the shared clock, repositioning is never free — neutral stays a real decision.

## 3.7 Walls, splats, hazards ✅ 🔬

Knockback and juggle arcs that would carry a victim through a wall instead produce a
**WALL_SPLAT** (a stuck, juggleable state — §6.2) if the combo's wall-splat latch is unspent,
else clamp to pushback. Breakable segments author a one-time event (arena extension, debris,
bonus state). Hazard volumes fire their authored event on contact. Wall pressure and carry are
a core Tekken depth axis and the main reason arenas are bounded. ✅

---

# §4 LAYER 2 — THE ENGINE (scheduler & resolution)

## 4.1 Decision points ✅

The engine advances `T` tick-by-tick, resolving contacts and motion, and **pauses** whenever any
actor has a decision. Decision kinds:

| Kind | Who | When | Choices |
|---|---|---|---|
| **Ready** | a free actor | `ready_tick == T` | any affordable, requirement-met move (incl. WAIT, Guard, movement) |
| **Cancel** | an actor in a cancel window | window open & gate satisfied | take a listed cancel (pay) or decline |
| **Reaction** | a defender | event-opened (below) | a state-gated option or pass |
| **Wake-up** | a DOWN actor | wake timer | rise / back-rise / delayed rise / any `state=DOWN` move (incl. reversals) |

**Reaction windows** unify the defender-side prompts: a **throw connecting** opens a break
prompt (§5.4); **each combo hit landing** opens a burst prompt for the victim *if* burst is
affordable and unused (§8.5) — otherwise it auto-passes silently (no UI spam, no information
generated). Blocked hits may open costed block-option prompts in future iterations (deferred ⚠️).

## 4.2 Side-blind simultaneous commits ✅ (the fairness rule)

All decisions pending at the same tick `T` are collected and grouped **by side**. Each side
commits all of its actors' choices *without seeing the other side's same-tick commitments*; your
own side's pending choices are mutually visible while you compose them (you're one mind giving
orders). Then the tick executes everything at once.

This generalizes v1's NEUTRAL-regime hidden commit to N actors — and because intent is *always*
fogged in v2 (§7), the old NEUTRAL/PRESSURE information split disappears entirely. What survives
of "pressure" is physical: a reeling, blocking, or committed opponent factually cannot act, and
you can see *that* (it's observable state), so offense on a locked opponent plays exactly like a
fighting game's plus-frames — you just read their wake-up option instead of being told it.

Deterministic same-tick effect ordering: stable entity-id order, with simultaneity rules
(throw-vs-throw → tech; trade hits both resolve) handled in the resolver — order never decides
a winner where the rules define a clash. ✅

## 4.3 The advance loop ✅

```
loop:
  pump_decisions(T)                 # gather same-tick decisions, side-blind commit, apply
  step_tick(T):
    for each in-flight move: update phase (STARTUP→ACTIVE→RECOVERY)
    for each active HitEvent × victim: if does_hit → resolve_contact (§5.1)
    integrate motion (advance, knockback, juggle arcs, projectiles), clamp to arena, walls (§3.7)
    tick statuses, stun timers, latch durations (Heat), meter regen (Breath)
    check KO / side elimination
  T += 1
```

The match result, trace, and every intermediate state are a pure function of
(initial state, content + Ruleset, the decision log) — C-DET. The **trace** (every commit,
contact, reaction, and state change as tagged events) remains the behavioral contract that the
future golden vectors v2 will freeze.

---

# §5 CONTACT RESOLUTION

## 5.1 The priority table ✅

When `does_hit` is true, the defender's state decides the branch — read top-to-bottom, this *is*
the interaction priority:

```
resolve_contact(attacker, hit, defender, T):
  1. defender INVULN to category            → WHIFF
  2. category == THROW:
       defender also throwing this tick     → THROW_TECH (clash; both reset)
       defender GRABBED-able (standing/crouch-throws per reqs)
                                            → GRAB CONNECT → opens the break reaction window (§5.4)
  3. defender in GUARD_POINT covering it    → PARRIED (§5.5)
  4. defender guarding & facing it (§5.3):
       height covered by current guard      → BLOCKED (chip to Guard, blockstun, on_block)
       height not covered (the mixup)       → HIT
  5. defender ARMOR covering it, hits left  → ARMORED (scaled damage, no stun, move continues)
  6. otherwise                              → HIT (counter-hit if defender in CH state)
```

## 5.2 Heights & stances — Tekken logic ✅ 🔬

v1's SF-style OVERHEAD level is **dropped**. The Tekken triangle:

- **HIGH** — fast, safe, often CH tools. **Whiffs entirely over a CROUCHING victim** (not
  blocked — *missed*), which is what makes ducking a read with a launch-punish payoff.
- **MID** — hits crouchers; blocked only by standing guard. The vertical "overhead" threat.
- **LOW** — hits standers; blocked only by crouching guard; typically seeable (slower cues) but
  rewarding. 🔬 Tekken lows are reads, not reactions — which suits a no-execution game perfectly.

**CROUCHING** is a held stance (a STANCE move, like Guard): it ducks highs *passively* and
changes which guard you can hold. Moves may require or grant stances (`reqs.stance`), giving
Forms crouch-mixup identities without engine special cases.

## 5.3 Blocking — a facing-relative stance ✅

Guard is a held STANCE move: brief startup (you cannot block instantly — mixups must work),
open-ended hold, brief release. While guarding:

- Strikes arriving **within your `block_arc`** and matching your guard height are BLOCKED:
  chip drains the **Guard** meter (not HP), you take blockstun, attacker gets `on_block`.
- Attacks from outside the arc — **back and deep flank hits — cannot be blocked or parried.**
  Facing is therefore a defensive resource, and choosing a target doubles as choosing what you
  can defend (§8.3).
- **Guard meter at zero → GUARDBROKEN**: a long, fully punishable stun (Ruleset-authored
  duration), the anti-turtle terminus. Guard regenerates slowly while not blocking.
- While holding guard you re-decide at an authored interval and whenever an event touches you
  (blockstun resolving, etc. — §4.1), so turtling never stalls the turn flow. ⚠️ interval tuning

## 5.4 Throws & the directional break ✅ 🔬

Throws ignore guard and armor, have short envelopes, realign on auto-facing (sidestep does not
escape a committed-to-range grab; spacing and strikes do), and **cannot be canceled into**.

When a grab connects, the defender gets a **reaction window**: guess the throw's authored
`break_key` — **L or R** — or decline. Correct guess → THROW_TECH (clash, both recover, small
separation); wrong or declined → THROWN (the throw's HitEvents run: damage, knockdown, oki).
🔬 This is Tekken's 1/2 break read with the dexterity removed and the *guess* kept. At knowledge
tier 3 (§7.3) the UI shows a grab's break key during its cue — studied opponents get their
throws broken, exactly like high-level Tekken. Command grabs may author `break_key: NONE`
(unbreakable) and pay heavily for it on the budget (§13).

Same-tick mutual throws → THROW_TECH automatically (unchanged from v1).

## 5.5 Parry / guard point ✅

A `GUARD_POINT` window deflects one covered strike: the attacker is frozen in an authored freeze
(on the *parry move's* data — C-AUTH), the parrier recovers fast and typically banks Focus/AP
(authored gains). High risk (tight window, loses to throws and to the uncovered height), high
reward. Parries are Form identity pieces, not universal mechanics.

## 5.6 Counter-hit ✅ 🔬

A defender struck during its own move's startup/recovery (or an explicit `CH_STATE` window) is
**counter-hit**. The hit's `ch_reaction` runs instead of `reaction` — this is how Tekken-style
**CH launchers** exist ("this jab is +1; this CH stuns into a full combo") — and if no override
is authored, the Ruleset default (damage mult + bonus stun) applies. CH is the payoff for
whiff-punishing and frame traps; it is also the **strike answer to throws** (a striking defender
beats an incoming grab's startup).

---

# §6 HIT REACTIONS & THE COMBO SYSTEM

## 6.1 The Reaction union ✅

What a hit *does* to its victim is an authored value, exhaustively matched by the engine:

```
Reaction =
  | Hitstun(n)                # standard reel; n decays per combo hit (governor 1)
  | Crumple(n)                # slow stagger→collapse; juggleable standing-state pickup window
  | Launch(arc)               # airborne; enters JUGGLE (the combo starter)
  | Screw                     # juggle extender: flattens the arc, extends carry — ONCE per combo 🔬 T7 tailspin
  | Bound                     # juggle extender: slams to a bounce, re-juggleable — ONCE per combo 🔬 T6 bound
  | Knockdown(soft)           # techable knockdown (quick-rise options)
  | Knockdown(hard)           # untechable; full oki
  | Push(dist)                # pure separation (also: blocked-hit pushback)
```

Wall interplay: a Launch/Push arc that reaches a splat-able wall becomes **WALL_SPLAT** (stuck,
juggleable, gravity-suspended for an authored window) if that combo's splat latch is unspent —
else it clamps to pushback. 🔬 Tekken wall carry: the *reason* to take juggles toward walls.

## 6.2 The combo grammar ✅ 🔬

The grammar Tekken players will recognize, with each link authored and each extender latched:

```
opener (Launch | CH-launcher | Crumple | GuardBreak punish)
  → juggle hits (decay applies per hit)
  → [Screw]                       (once) — carry toward the wall
  → [WALL_SPLAT]                  (once) — wall pickup
  → [Bound]                       (once) — ground bounce pickup
  → ender (Knockdown(hard) → okizeme, or Push → spacing reset)
```

Every arrow is a **cancel or a link the player chooses and pays for** (AP per action, Focus per
special cancel). Combos are *planned sentences*, not dexterity tests — and they end (§6.5).

## 6.3 Okizeme & wake-up options ✅

Hard knockdown puts the victim DOWN with a wake timer; the attacker factually moves first. At
wake-up the defender chooses among rise-in-place / back-rise / delayed rise / any authored
`state=DOWN` move — including **reversals** (invuln-startup, ruinous on whiff) if the Form
grants one and the meters allow. This closes v1's one-sided oki (the gap analysis's #1 fix) and
keeps okizeme a mixup rather than a sentence.

## 6.4 Grounded strings ✅

Tekken-style strings (jab-jab-sweep, delayed mids) are authored **cancel chains** between
normals (§11): cheap chains with hit/block gates, branch points, and delay windows. The string
*mixup* (will the third hit be the mid or the low?) is carried by the fog (§7) — the defender
sees the string's shared cue, not its branch — exactly reproducing "you have to know the string"
matchup knowledge from real Tekken, as an explicit, learnable system.

## 6.5 THE ANTI-INFINITE CHARTER ✅ (C-FIN — seven governors)

No combo may be infinite. Seven **independent** governors, each alone sufficient to terminate
loops, all audited (§13.4). A new mechanic must state which governor bounds it before it ships.

1. **Hitstun decay** (Ruleset schedule): each consecutive hit in a combo reduces effective stun;
   advantage trends negative, so every chain eventually drops.
2. **Juggle damage decay** (Ruleset × defender weight): juggle damage trends to zero.
3. **Extender latches**: Screw, Bound, WALL_SPLAT each usable **once per combo** (Ruleset).
4. **AP exhaustion**: every action costs AP; AP refills only when free of a string (§9.4).
5. **Focus pricing**: special/super cancels spend the super gauge — extension is bought.
6. **No positive cycles** (rule R-5): the cancel graph may contain no cycle whose net
   AP+Focus ≥ 0; verified mechanically over all content.
7. **The gravity floor** (Ruleset `forced_landing`): once decayed stun in a juggle falls below
   the minimum pickup startup among the attacker's affordable follow-ups, the victim lands —
   the audit proves every juggle path in shipped content terminates within K hits.

**Relief valves** (defender agency, not governors): ally interruption and the solo Burst (§8.5).

---

# §7 INFORMATION — THE FOG OF WAR

## 7.1 The Observation API ✅ (C-FOG)

Everything anyone learns about a fight flows through one interface, **consumed identically by
the player UI and by every AI agent**. If a fact is not in Observation, neither the player nor
the AI can act on it — the fog is enforced by architecture, not discipline.

Observable, per enemy actor, at any decision point:
- **Physical state**: position, facing, **current target** (who they're squared up against is
  visible body language), stance, motion through space.
- **State class**: free / committed / reeling / blocking / down / grabbed — you can *see* that
  someone is locked, just not what they're locked into.
- **The cue** (§7.2) of any in-flight move, with a coarse phase tag (wind-up / swinging /
  recovering). **Exact remaining ticks are not shown** (until knowledge supplies them).
- **HP** (default-visible; per-meter `visibility` flags in DefenseProfile make all of this
  tunable — locked decision: only HP at first). ✅
- **Public events**: everything that has *resolved* — hits, blocks, parries, movement, KOs —
  is permanently public, replayable, and exact (C-FOG: facts are never fogged, only intent).

Own side: full information, always (you are one mind commanding it).

## 7.2 Cues — intent as silhouette ✅

Every move authors a **CueClass**: the observable wind-up. Cues are the fog's currency:

- Cues are **coarse-grained by design**: a Form's moves share a small cue vocabulary
  ("low coiling stance", "high overhand wind-up", "lunging grab shape").
- **Feints are cue collisions**: a move that shares a cue with a scarier sibling *is* a feint —
  the throw that starts like the launcher; the bait that starts like the sweep and recovers
  fast with a CH window. Feint coverage is priced by the budget (§13: cue ambiguity is a paid
  strength), so lying isn't free.
- Cue legibility is an art-direction contract: a cue that can't be drawn readably in silhouette
  gets redesigned (see `vision-mda.md` §5).

## 7.3 Matchup knowledge — the fog as progression ✅

Per-move knowledge tiers, aggregated per Form/enemy style for display (full rules in
`progression.md`):

| Tier | Earned by ⚠️ | What the UI now does |
|---|---|---|
| **T0 Unknown** | — | cue shows as a generic silhouette class |
| **T1 Glimpsed** | seeing the move resolve | move named; codex entry; height/category class shown |
| **T2 Studied** | repeated exposure / bought intel | full frame data in codex; when its cue appears, the ribbon overlays the **candidate set** (all moves sharing that cue) with their hit windows |
| **T3 Mastered** | extensive exposure / master training | exact phase-tick readout on their in-flight moves; **throw break keys shown on grab cues**; candidate set ordered by this enemy's observed habits |

Knowledge never removes the read — a T3 candidate set with two entries is still a guess — it
*sharpens* it, which is exactly what matchup knowledge does for a real fighting-game player.

## 7.4 The forecast — the honest prediction ✅

When composing a commitment, the player sees a deterministic preview rendered from the engine's
own math (never a reimplementation): the move's reach envelope painted on the arena, its phase
ribbon on the timeline, and — for every actor currently inside the envelope — the projected
result *if the world stays as last observed* (damage, reaction, resulting advantage). The
forecast is exact about your move and silent about their hidden intent; the gap between those
two is the game. ✅ user-confirmed: "this allows for feints and other tricks."

## 7.5 The AI contract ✅

AI agents receive the same Observation a player would and **never** read hidden commitments.
Their differing strength comes from authored **read profiles** (aggression, gambler, turtle,
step-happy), per-enemy cue-response tables, and bosses' authored "smart reads" (better priors,
not x-ray vision). This keeps every AI fight honest and every AI beatable by the same skills
the game teaches. (Authoring details live with L5/encounters — out of combat-spec scope.)

---

# §8 PARTY COMBAT

## 8.1 Sides, control, scale ✅

N actors per side on one timeline; the player commits for **every** allied actor at its decision
points (interleaved naturally by `ready_tick`). Designed and balanced for **3 per side**; N is a
content/config knob, not an engine assumption. Sides and elimination drive outcome: a side is
out when all its actors are KO; last side standing wins.

## 8.2 Bodies on the plane ✅

Actors do not body-block (pass-through); only hitboxes interact. Wide hits and beams clip
bystanders per `does_hit` — lining two enemies onto one lane is a legitimate, rewarded play
(and the anime fantasy). **Friendly fire defaults OFF** per hit; specific reckless moves and all
arena hazards may author it ON. ✅

## 8.3 Sandwiches, back attacks, geometry defense ✅

Because guarding is facing-relative (§5.3), being targeted by two enemies on opposing lanes
means someone is at your back: back hits bypass guard and parry. Counterplay is **geometric**:
re-target (free with any action; or the quick `switch_focus`), sidestep to put both enemies on
one side (the kung-fu-movie rotation), spend movement to break the sandwich, or trust an ally to
peel one off. ⚠️ Watch-item: outnumbered situations must feel *dangerous but navigable* — boss
solos get authored armor/homing kits instead of engine pity rules (C-AUTH).

## 8.4 Ally interruption — mostly emergent ✅

A comboing attacker is **locked in their own moves and counter-hittable** like anyone else, so
the primary combo-rescue is simply: *another ally hits them.* That this falls out of the
existing rules with no special case — the rescuer's launcher counter-hits the comboer mid-string
— is the party system's best feature. On top of the emergent path, Forms may author **RESCUE
moves**: gap-closers gated `reqs.condition: ALLY_IN_HITSTUN`, typically armored and
Focus-costed, for when positioning failed. They obey every governor; nothing about rescue is a
new engine path.

## 8.5 The solo Burst ✅

When no conscious ally can reach you, a **BURST move** (gated: usable from HITSTUN/JUGGLE via
the per-hit reaction window, §4.1) spends a large Focus cost + the once-per-fight burst latch:
brief full invuln, a small radial push, both actors reset to free. The canonical
anti-oppression valve 🔬 (GG Psych Burst), deliberately expensive and once.

## 8.6 KO, revival, loss ✅

Companions at 0 HP are KO'd (removed from the timeline, body remains as fiction); allies can
revive them mid-fight via authored UTILITY moves (slow, interruptible, the classic JRPG gamble)
or after victory. **Loss = full party wipe**, and is a soft loss at the campaign layer
(see `exploration.md`). The protagonist falling is not special-cased. ✅

---

# §9 METERS & ESCALATION

Five pools + two latches. Every pool answers a *different* strategic question, so a string can
end (or a fight turn) for distinctly legible reasons. All maxima/regeneration compiled by L4;
all costs authored per move (C-AUTH).

| Meter | Builds / regenerates | Spent on | The question it asks |
|---|---|---|---|
| **HP** | out-of-combat / items / revival | — | are you still in the fight |
| **Breath** | per-tick while not executing | most actions (small), big motions (more) | can your body keep going — the anti-mash pacing floor, deliberately light ⚠️ |
| **Guard** | slow regen while not blocking | drained by blocked chip | can you keep turtling (no → GUARDBROKEN) |
| **AP** | refills to max when you exit a string / regain freedom | every action's `ap_cost`; `ap_gain` on success gates | how long your *turn* runs — the tempo budget |
| **Focus** | **earned**: landing hits, having hits blocked, *taking* damage (small 🔬 comeback factor), parries / CH / whiff-punishes (large — skill pays) | special cancels, EX moves, supers, Burst, many RESCUE moves | what power you've earned — the escalation gauge |

**Escalation is structural**: Focus only accumulates during a fight, Heat and Rage are one-way
latches — so every fight ramps from grounded pokes toward beams and cut-ins by construction,
not by script. (The DBZ arc, guaranteed by bookkeeping.)

## 9.4 AP notes ✅

Per-actor. In open play you commit one action and AP barely binds; against a locked opponent you
chain cancels, paying per link — **a long combo is literally an AP expenditure**. `ap_gain` is
conditional on success (ON_HIT/ON_CH/ON_PARRY...), never unconditional; rule R-5 forbids any
self-reaching cycle with non-negative net gain. (Inherited from v1 §3.5 unchanged in substance.)

## 9.5 Heat ✅ 🔬 (Tekken 8)

One per fight. Enter via a **Heat Burst** action or by landing a `HEAT_ENGAGER` hit. For an
authored duration: the actor's moves use their compiled **Heat variants** (the L4 compiler
emits both profiles up front — Heat is "swap the resolved frame data," conceptually free for the
engine), chip-on-block, unique Heat-only cancels/dashes. Ends on timer or KO. *When to
transform* is a real mid-fight decision, and visually it is the aura moment.

## 9.6 Rage ✅ 🔬

At an authored HP threshold the actor latches **Rage**: a passive authored damage scalar and
access to the **Rage Art** — a once-only, armor-startup cinematic super that consumes the state.
The legitimate comeback read: everyone knows it's loaded (HP is visible), nobody knows when.

---

# §10 SUPERS, EX, PROJECTILES

## 10.1 EX moves ✅

Focus-priced enhanced variants of Form moves (more damage, better reactions, armor, extra hits)
— authored as their own Move entries sharing the base cue (an EX *looks* like its base until it
lands: an honest, priced ambiguity).

## 10.2 Supers ✅

Large Focus spends; cinematic presentation (cut-ins, camera takeover); typically armored or
invulnerable startup, enormous reward, ruinous whiff recovery; gated by Form rank (L4). Supers
respect every governor (they end strings emphatically rather than extending them — most are
combo *enders* by authored reaction).

## 10.3 Projectiles & beams ✅

- **Missiles** (`PROJECTILE_SPAWN`): independent timeline entities — position, velocity along
  facing at spawn, lifetime, one HitEvent. Two overlapping missiles annihilate by priority tier.
  Sidestep beats LINEAR missiles; blocking chips Guard; i-frame-through is Form tech.
- **Beams**: not entities — a move with a very long, narrow envelope and a multi-tick active
  window (Kamehameha as frame data). Inherently LINEAR (sidestep is the answer 🔬 DBZ dodges);
  homing beams are budget-priced exceptions.

---

# §11 CANCELS & CONFIRMS

Inherited from v1 nearly verbatim — the rules that make combos planned rather than reactive:

- `CancelWindow { from, to, gate: ON_HIT|ON_BLOCK|ON_CONTACT|ALWAYS|ON_WHIFF, into, cost }`.
- **Lock-then-confirm**: your initial commitment is blind (side-blind, §4.2), but ON_HIT /
  ON_BLOCK cancels are decided by the **actual contact result** — you genuinely hit-confirm,
  reacting to facts, never to the opponent's hidden input. ✅
- **No startup cancels by default** (`startup_cancelable: false`): you cannot un-commit because
  the reveal scared you. Explicit exceptions are authored, costed feint tech. ✅
- Whiff cancels exist behind `ON_WHIFF` gates: rare, expensive, budget-priced.

---

# §12 LAYER 4 — THE BUILD → FIGHTER COMPILER

The contract, restated for v2 (mechanics of acquisition live in `progression.md`):

```
Build  = { attributes, Form ranks, foci, equipment + affixes, loadout selection }
compile(Build) -> Fighter {
    MoveList      # resolved Moves: base (authored in the Form) × attribute levers
                  #   × equipment deltas × affix riders; Heat variants emitted alongside
    DefenseProfile
    meter maxima, visibility flags
}
```

- The engine never sees a stat; the compiler never reaches past the data contract. One bridge,
  one direction (audit C-5).
- **Weapon = spacing identity** (range/speed/damage triangle, rule R-4); **Form = moveset
  identity** (cue vocabulary, signature mechanics); **attributes = levers** (exactly one major
  lever each, rule R-2); **affixes = riders** on any of the above, budget-audited (§13).
- Requirements are floors with small capped over-meet bonuses, never multipliers (rule R-3).
- Hitstun/blockstun on the defender's side are never compiler-touched, keeping I-1 advantage
  honest.

---

# §13 BALANCE AS A CHECKABLE PROPERTY

## 13.1 The budget identity v2 ✅

Every move must pay for its strengths; v2 extends v1's `MOVE_VALUE` with the new axes:

```
MOVE_VALUE(move) =
    w_speed   · (BASELINE_STARTUP − startup)
  + w_safety  · on_block
  + w_reward  · expected_damage_and_reaction_value      # Launch > Crumple > Hitstun, etc.
  + w_range   · reach_advantage
  + w_arc     · region_width_and_multi_target_value     # party-relevant: wide sweeps & beams pay
  + w_track   · tracking_coverage                        # HOMING pays, LINEAR refunds
  + w_props   · Σ property_values                        # armor/invuln/heat-engager priced
  + w_meter   · Σ gain_values                            # AP/Focus generation priced
  + w_lie     · cue_ambiguity_value                      # sharing a cue with a high-threat sibling pays
  − w_cost    · resource_costs
  − w_commit  · whiff_exposure
  ≈ FORM_TIER_BUDGET ± ε        for every move in shipped content
```

⚠️ The weights are the master tuning knobs (playtest-owned); the *identity* is the law.

## 13.2 The balance rules ✅

R-1 no zero-cost dominant action · R-2 one major lever per attribute · R-3 gates are floors ·
R-4 the weapon range↔speed↔damage triangle (no Pareto-dominant weapon) · R-5 no net-positive
AP/Focus cycle in the cancel graph · **R-6 (new)** every juggle path terminates within K hits
under the Ruleset (governor-7 proof over content) · **R-7 (new)** cue honesty: every move has a
cue; every cue class in a Form has ≥ 2 members *or* its singleton move pays `w_lie = 0`
(no free information hiding, no unreadable one-off tells).

## 13.3 The defensive RPS, v2 edition ✅

Every defensive option loses to something; the audit checks no option dominates:
block ↔ throws & guard-decay · crouch ↔ mids · parry ↔ throws/feints · sidestep ↔ homing &
realigning grabs · backdash ↔ advancing reach · re-target/rotation ↔ committed homing pressure ·
Burst ↔ its price & once-ness · ally rescue ↔ positioning & its own counter-hit risk.

## 13.4 The audit ✅

A content-level test binary (run in CI, like v1's `npm run audit`): I-1 consistency · budget
residuals · R-1…R-7 · cancel-graph cycle scan · juggle-termination proof · RPS coverage matrix ·
cue-collision report (every feint pair listed, priced, and intentional).

---

# §14 WORKED EXAMPLE — a 2v2 under fog

Player side: **Reza** (dagger, *Drifting Leaf* — step/CH Form) and **Borin** (greatsword,
*Iron Mountain* — armor/guard Form). Enemy side: a **Corsair Duelist** (rapier strings) and a
**Fog-Touched Brute** (command grabs). Knowledge: Reza's player has the Duelist at **T2**
(studied), the Brute at **T0** (never met). Arena: a dock — wall on the west, water hazard east.

| T | What happens | Why it demonstrates the spec |
|---|---|---|
| 0 | Both sides have all actors free at T=0 → **side-blind commit** (§4.2). Player: Reza steps in targeting the Duelist; Borin guards facing the Brute. Enemy (hidden): Duelist begins a string at Reza; Brute advances on Borin. | N-actor scheduling; commitments hidden across sides, shared within a side. |
| 4 | Reza's decision point. Observation: Duelist shows the **"low coil" cue, wind-up phase**. At **T2 knowledge** the ribbon overlays the candidate set: `{sweep (LOW, launches on CH), thrust feint (MID, fast recover)}`. A guess — sharpened, not solved (§7.3). | Cues, candidate sets, the read surviving knowledge. |
| 4 | Reza commits **sidestep-N** (perpendicular to the *Duelist's* lane). Forecast shows her vacating the narrow LINEAR band of both candidates (§7.4). | Target-lane geometry; the honest forecast. |
| 9 | The sweep was real — it **whiffs through Reza's vacated band** (`does_hit` arc clause fails). Duelist eats `on_whiff = 0` → full recovery, visibly *recovering* (state class is public). | Sidestep = whiff-punish setup, same path as footsies (§3.3). |
| 10 | Reza, free first, **whiff-punishes**: CH launcher (the Duelist's recovery = CH state) → `ch_reaction: Launch`. Juggle begins: chain (AP 1), special cancel (Focus 3, AP 2) → **Screw** (latch spent) carrying west toward the wall. | CH rules (§5.6); grammar + governors paying as she goes (§6). |
| ~22 | Carry reaches the wall → **WALL_SPLAT** (latch spent), pickup, two decayed hits — hitstun decay now beats her cheapest pickup's startup → **gravity floor forces the landing** (governor 7). She authors the ender: hard knockdown → oki. Combo over: 6 hits, 3 latches spent, AP nearly dry. | Wall carry; three governors visibly terminating the combo (§6.5). |
| 12→24 | Meanwhile the Brute's unknown cue (T0: generic "lunging grab shape") connects on guarding Borin — **throws beat block** — opening the **break reaction window**: L or R, no knowledge hint at T0. Borin guesses L; the key was R → **THROWN**, hard knockdown by the water's edge. | Throw fog; the directional read; reaction windows (§5.4, §4.1). |
| 30 | Brute begins oki pressure on downed Borin — but the Brute is mid-cue on a slow stomp and **Reza retargets** (free with her next commit) and dashes the open lane. Her strike **counter-hits the Brute mid-move: the combo on Borin never starts.** No rescue mechanic fired — just the rules (§8.4). | Emergent ally interruption; retargeting; sandwich geometry reversed. |
| 38 | Borin takes the wake-up decision: delayed rise (reads a meaty), then **Heat Burst** on rising — greatsword Heat variants come online, chip-on-block (§9.5). The fight has visibly escalated: Reza's Focus from the punish-combo + Borin's from taking hits ≈ a super is now affordable. | Wake-up options (§6.3); escalation by construction (§9). |
| 44+ | Borin cancels a blocked Heat cleave into the **beam super** (Focus dump): a long narrow envelope down the lane — it clips **both** enemies, who'd been allowed to line up. Cut-in, KO the Duelist; the Brute blocks at the cost of guard-crush. | Supers & beams (§10); multi-target regions priced but devastating (§8.2). |

Every beat above runs on the general rules — no special cases were invoked anywhere in this
table. That is the consistency proof carried over from v1, now under fog, in a party, on a plane.

---

# §15 OPEN TUNING TABLES ⚠️ (playtest-owned, deliberately not asserted)

1. Ruleset curves: hitstun decay schedule, juggle decay, guard-break stun, block re-evaluate
   interval, wake timers.
2. Budget weights `w_*` and Form tier budgets.
3. Meter economy: Breath costs/regen, AP_max/refill & per-tag ap_costs, Focus gain table
   (the skill-pays multipliers), Burst price, Heat duration, Rage threshold & scalar.
4. Knowledge thresholds (T1/T2/T3 exposure counts) and intel pricing.
5. Cue vocabulary size per Form (readability vs. richness).
6. Attribute lever curves (per `progression.md` §3).
