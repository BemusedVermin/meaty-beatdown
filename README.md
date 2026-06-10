# TICK (rpg-fighting-game)

A **party-based, partial-information, turn-based JRPG built on fighting-game frame data** — all
of Tekken's brain with none of Tekken's hands, drawn like an anime, set in a world being eaten
by fog.

You see your own frame data and an honest forecast of how your move will land. You **cannot**
see what the opponent committed — only the cues a trained fighter could read, sharpened by
matchup knowledge you earn across a campaign. Combos, supers, Heat, Rage, walls, throws-and-
breaks: the full fighting-game trappings, with execution cost replaced by tactical depth.

## Status

**In build** (signed off 2026-06-10). The docs below are the source of truth; the deterministic
Rust engine + Bevy app are being built phase-by-phase in `rust/` —
see [`docs/implementation-plan.md`](./docs/implementation-plan.md).
A previous prototype (TypeScript reference + Rust workspace + golden vectors) was retired to git
history (`0e2eaae`) in the 2026-06-09 clean-slate reboot; the v2 behavioral contract will be
regenerated from the new engine.

## The docs

| Doc | What it is |
|---|---|
| [`docs/vision-mda.md`](./docs/vision-mda.md) | The vision and MDA — what this is and why it'll feel good |
| [`docs/frame_rpg_spec.md`](./docs/frame_rpg_spec.md) | **The combat spec (v2)** — the source of truth for the fighting system |
| [`docs/exploration.md`](./docs/exploration.md) | The fog-eaten hexcrawl: sailing, encounters, masters' islands, loot |
| [`docs/progression.md`](./docs/progression.md) | No-XP progression: rank trials, attributes, Forms, gear, knowledge |
| [`docs/fsm.md`](./docs/fsm.md) | The state machines (app, game, combat, actor) |
| [`docs/the-promise-plot-bible.md`](./docs/the-promise-plot-bible.md) | Setting & narrative — THE PROMISE (v0.2) |
| [`docs/tech-plan.md`](./docs/tech-plan.md) | Architecture: deterministic Rust engine + Bevy shell |
| [`docs/implementation-plan.md`](./docs/implementation-plan.md) | The phased build plan to the vertical slice |
| [`docs/archive/`](./docs/archive/) | v1 prototype docs (historical reference) |
