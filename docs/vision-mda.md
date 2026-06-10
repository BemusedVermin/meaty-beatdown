# TICK — Vision & MDA

*The one-document answer to "what is this game and why will it feel good."*
*Companions: [`frame_rpg_spec.md`](./frame_rpg_spec.md) (how combat works),
[`exploration.md`](./exploration.md) / [`progression.md`](./progression.md) (the RPG around it),
[`the-promise-plot-bible.md`](./the-promise-plot-bible.md) (the story), [`tech-plan.md`](./tech-plan.md) +
[`implementation-plan.md`](./implementation-plan.md) (how we build it).*

---

## 1. Logline

**A turn-based JRPG whose battle system is a real fighting game with the execution cost removed
and the information cost added.** You see your own frame data and an honest prediction of how your
move will land. You cannot see what the opponent committed — only what a trained fighter could
*observe*: their stance, their wind-up, the way their weight shifts. Reads, spacing, frame traps,
combos, and supers — all of Tekken's brain, none of Tekken's hands — wrapped in a party-based RPG
set in a world being quietly eaten by fog.

## 2. The three archetypes, braided

The game sits at the intersection of three things, and each one covers another's weakness:

| Archetype | What it contributes | What it must NOT contribute |
|---|---|---|
| **Tekken** (the imitated fighter) | The combat grammar: a lane between two fighters, sidesteps into new lanes, highs/mids/lows, counter-hits, launchers → juggles → wall splats, throws and breaks, Heat and Rage. Frame data as the language of truth. | Execution barriers. Motion inputs, just-frames, reaction windows — all deliberately translated away. Skill is *what and when*, never *how fast your hands are*. |
| **Dragonball / One-Punch Man** (the aesthetic register) | Escalation. Fights that visibly ramp: meters build, auras flare, transformations (Heat), desperation comebacks (Rage), beams and cinematic supers. Impact frames, speed lines, cut-ins. Power that *looks* like power. | Power that trivializes decisions. Escalation raises the stakes of the read; it never replaces it. (And, per the plot bible, the OPM irony is load-bearing: getting stronger is the trap.) |
| **JRPG / xianxia** (the body of the game) | The party, the world, and the build. Companions who fight beside you on the same timeline. A fog-eaten hexcrawl world with masters, trainers, dungeons, and Diablo-style loot whose affixes are *frame data*. Progression through rank and Forms, not XP bars. | Combat leakage. Fighting-game mechanics live **only** inside combat; exploration and menus stay a warm, readable RPG. No drilling minigames on the overworld. |

## 3. MDA

### Mechanics (what the rules are)

- **One shared deterministic tick timeline.** 1 tick = 1 frame at 60 Hz. Every action is frame
  data (startup / active / recovery). The clock advances only when the simulation says so; the
  engine pauses indefinitely whenever any actor must decide. No dice in combat — hit/miss is
  spacing and timing, resolved once.
- **The fog of war.** You commit blind. The enemy's committed move is never shown — only
  *observable cues* (authored per move: stance, wind-up silhouette, height tell) through a single
  Observation API that the UI **and the AI** both live behind. Matchup knowledge, earned by
  fighting, upgrades what a cue tells you.
- **The honest forecast.** When you line up a move you see a deterministic preview — reach
  envelope on the arena, timing ribbon, damage and frame advantage *if* it connects against the
  world as last observed. The forecast never lies; it can only be invalidated by what you couldn't
  see. That gap is where feints live.
- **Target-lane 3D arenas.** The arena is a true 3D space. Every actor targets exactly one other
  actor, and **the target creates the lane**: all spacing math runs along that line-of-sight.
  Sidesteps displace you off an attacker's lane; with multiple fighters, position *is* tactics —
  sandwiches, back attacks, lining enemies up.
- **Party JRPG battles on the fighting-game clock.** N actors per side (designed for 3, N is a
  knob), all on one timeline, the player committing for every ally at their decision points.
  Companions can be KO'd and revived; loss is a full wipe.
- **Authored everything.** No engine combat constants and no generic moves: counter-hit bonuses,
  parry freezes, juggle behavior, wake-up timings — every magnitude is data authored on a move, a
  reaction, or a fighter, compiled by the RPG layer (Forms + attributes + equipment + affixes →
  resolved frame data). The engine is an interpreter.
- **Meters that ramp** — Breath (exertion), Guard (poise), AP (tempo, the string budget),
  **Focus** (the earned super gauge), plus **Heat** (once-a-fight install) and **Rage**
  (low-HP comeback). Offense and skillful defense build Focus; supers spend it.
- **Combos with hard ceilings.** Tekken's juggle grammar (launch → screw → bound → wall splat)
  expressed as authored reaction states, governed by **seven independent anti-infinite rules**.
  No infinite combos — this is a charter item, audited by tooling, not a hope.
- **A fog-eaten hexcrawl** with visible encounters, master-anchored islands, dungeons, and
  loot whose affixes modify frame data. Progression with **no XP**: rank trials, attribute
  training, Form ranks, gear, and knowledge.

### Dynamics (what play actually looks like)

Three nested loops, one per timescale:

- **The exchange (seconds of fiction, one read).** Neutral is a hidden-commitment mind game:
  whiff-bait, sidestep the linear heavy, duck the high and launch. Pressure is a tempo budget:
  spend AP to keep a string alive, hit-confirm into the real damage, end with knockdown into
  okizeme. Defense is its own read: block high or low, break the throw left or right, parry,
  step, or just hold and bank Guard. The forecast-vs-fog gap generates feints *mechanically*:
  any move whose cue resembles another's is a lie you can tell with your body.
- **The fight (minutes, one escalation arc).** Fights start grounded and end cinematic. Focus
  builds through play, so supers arrive late and feel earned; Heat is a one-time decision about
  *when to transform*; Rage turns a losing position into one last legitimate read. Party play
  layers on top: who faces whom, who gets sandwiched, an ally dashing in to interrupt the combo
  that would have killed you. Fights against *known* styles feel like rewatching a favorite
  matchup; fights against unknown ones feel like the first round against a new character.
- **The voyage (hours, one campaign).** Sail the fog between islands of reality. Pick fights you
  can see coming, loot frame data, learn Forms from masters, train attributes, study rivals —
  then notice the hidden meter ticking up. The world's clock (the Fog advances; masters Cross
  and their islands fade) gives exploration stakes that combat alone can't.

### Aesthetics (what it should feel like, ranked)

1. **Challenge — the master's mind.** The core promise: *the experience of being a top fighting-game
   player — the reads, the frame awareness, the matchup knowledge — granted to anyone who can
   think, with hands taken off the table.* Winning should feel like outwitting, never out-mashing.
2. **Discovery — knowledge is literally power.** Three mirrored layers: discover the world
   (the hexcrawl), discover the enemy (the codex, cues, break hints), discover the truth
   (the Promise, the Tithe). The same player verb — *learn* — drives all three.
3. **Fantasy — anime martial power.** Escalation you can feel: auras, installs, beams, the
   one-punch finisher. The build is the character; your frame data is your identity.
4. **Expression — the build lab.** Forms × attributes × gear × loadout = your personal fighting
   style. Two players' "same" character should play like different Tekken mains.
5. **Narrative — the slow poison.** The Tithe reveal retroactively recontextualizes every fight
   you enjoyed. The combat never gets worse to play; the story makes you ask who you're playing
   it for. (One-Punch Man's emptiness, delivered mechanically.)
6. **Sensation — the anime image.** Impact frames, smears, speed lines, super cut-ins, the
   camera cutting between simultaneous duels like a tournament-arc episode.

## 4. Experience pillars (testable)

1. **Every exchange is a read.** If a situation has a strictly dominant action, it's a bug —
   the balance audit (budget identity + RPS coverage) exists to enforce this.
2. **The forecast never lies; the fog never leaks.** Information honesty both ways. The UI may
   only show what Observation exposes; the prediction must be exactly what the engine would do.
3. **No execution cost, full fighting-game depth.** Anything whose only purpose is dexterity is
   translated away; anything that adds a *decision* is preserved or deepened.
4. **No infinite combos.** Seven governors, audited. A combo is a sentence, not a paragraph.
5. **Builds are characters.** Stats/Forms/gear compile into frame data — the RPG layer reshapes
   the fighting game, never bypasses it.
6. **The world is being unmade, and fighting feeds it.** Exploration stakes (the Fog) and the
   moral engine (the Tithe) are one system seen from two sides.

### Anti-pillars (things this game refuses to be)

- No motion inputs, links, or timing windows — ever, in any mode.
- No random hit/miss in combat; dice live out-of-combat only.
- No XP grind; strength comes from rank, teaching, loot, and knowledge.
- No fighting-game mechanics outside combat (no overworld drills, no encounter frame-previews).
- No oppressive information asymmetry: fog applies to *intent*, never to resolved facts —
  what has already happened is always fully visible and replayable.

## 5. Presentation direction

- **2D side-view characters in a 3D arena.** Sprites (or skeletal 2D) staged on a 3D ground
  plane, camera side-on to the *active lane* — exactly how Tekken frames a 3D fight as a 2D
  image. On decision points the camera cuts to the deciding actor's lane; a tactical overview
  toggle shows the whole arena top-down for party positioning.
- **The timeline ribbon** is the signature UI: a horizontal tick ribbon showing your committed
  actions as solid phase-colored blocks (startup amber / active red / recovery blue), allies'
  the same, and enemies' as **fog-shaded cue blocks** that sharpen with matchup knowledge. The
  forecast renders as a ghost overlay. This one widget teaches frame data by osmosis.
- **Anime VFX language**: impact frames (1–2 tick full-screen flashes on counter-hit), smear
  frames on fast moves, speed lines during dashes, ink-splash hit sparks, cut-in portraits on
  supers and Rage Arts, auras for Heat/Rage states. Hit-stop is unnecessary (the engine pauses
  at decisions), so *visual* punch carries the weight real-time games give to freeze frames.
- **Readability beats fidelity.** Every authored cue must be legible at a glance in silhouette;
  if a cue can't be drawn readably, the move gets a different cue class.

## 6. Scope anchor

The plan in [`implementation-plan.md`](./implementation-plan.md) aims at a **vertical slice**:
one island chain, a party of 3, one dungeon and boss, Heat/Rage/supers working, walls and one
hazard arena, the loot loop closed, and the fog visible on the map. Everything in this document
is judged against that slice first.
