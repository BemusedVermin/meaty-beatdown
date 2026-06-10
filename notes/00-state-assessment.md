# State assessment — 2026-06-09

## What "the damage" is

Commit `50ac198` ("Restarting workspace from scratch") deleted, in one commit:

- the entire `rust/` workspace (~22k lines): `engine` (fighting L2, exploration hexcrawl,
  content L4 compiler) + `app` (Bevy 0.18 shell: state FSMs, combat/exploration drivers,
  render, debuglog), Cargo.toml/lock, DESIGN.md, README.md
- all 13 `golden/*.json` vectors (the cross-language behavioral contract)

Everything is intact at parent commit `0e2eaae`. Recovery is a `git revert 50ac198` or
`git checkout 0e2eaae -- rust golden` away. **Nothing is lost**, just removed from HEAD.

What survives at HEAD: `docs/` (spec, mechanics, gap analysis, fsm, plot bible),
CLAUDE.md, README.md, .gitignore/.gitattributes.

## What the user wants (from the brief)

1. Refine `docs/` into a vision fitting: YOMI-Hustle-adjacent turn-based fighting JRPG,
   **partial information** (you see YOUR frame data + a hit prediction; you canNOT see
   what the opponent is doing), combos + supers encouraged, full fighting-game trappings
   minus execution cost, more tactical depth, anime visuals (Dragonball / One-Punch Man).
2. The imitated fighter is **Tekken**: lane fighter + sidesteps (docs already have this).
3. Setting/narrative docs (plot bible) are FINE AS-IS — do not rewrite.
4. Mechanics for exploration + progression (ideas present, need another pass).
5. Bevy.
6. Deliverables: refined vision docs, technical plans, MDA spiel, implementation plan.
7. Notes go here (`notes/`, gitignored — done).

## Tensions / contradictions to resolve in the refinement

- **Partial info vs. spec §2.1**: spec says PRESSURE regime = actor sees EVERYTHING the
  locked opponent is doing. User says "they cannot see what the opponent is doing."
  How far does the fog go? (asked)
- **Soulslike flavor vs. anime archetype**: spec is stamina/poise/WWN "scary combat";
  DBZ/OPM wants escalation, supers, flash. (asked)
- **Supers**: gap analysis marks supers/meter/install ⬜ absent; user says supers are core.
  Tier-1 recommendation #4 already points this way.
- **Memory decisions that postdate the docs** (from prior sessions, must fold in):
  - orthogonal move axes (MoveLevel/MoveClass conflation = latent bug; THROW duplicated)
  - authored qualities: NO engine combat constants (mechanics.md still has CH_DAMAGE_MULT,
    PARRY_FREEZE_TICKS etc. as engine constants — contradicts; refactor into authored data)
  - no separate Fighter type (Entity holds offense + defenses + runtime state)
  - RPG-first, fighting-game mechanics quarantined to Combat
  - exploration: flooded-world ocean hexcrawl, loot-driven (Diablo/BL2), masters anchor islands
- **Tekken specifics not yet in spec**: walls/wall splat (gap Tier-1 #3), Heat/Rage,
  Tekken-style high/mid/low logic (mids are the "overheads"; current spec has a separate
  OVERHEAD level — 2D-ism), power crush = armor (have), homing/linear (have), throws +
  throw breaks (have tech, no directional break read), bound/tailspin juggle extenders.

## Open questions asked of the user

See clarifying-questions round 1 + 2 + the text list in chat. Record answers in
`01-decisions.md` when they land.
