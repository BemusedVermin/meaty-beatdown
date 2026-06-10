# Locked decisions — clarifying rounds, 2026-06-09

## Round 1 — structural

1. **Lost code: keep the clean slate.** HEAD stays docs-only. The old rust/ + golden/
   live in history (`0e2eaae`) as reference only. New docs/plans define a fresh rebuild.
   Old golden vectors are therefore dead as a contract.
2. **Fog of war: cues + learnable knowledge.** You never see the opponent's committed
   move directly — only observable cues (animation phase, stance, height tells).
   Opponent move data becomes known through fighting/studying them; the prediction UI
   sharpens with matchup knowledge. Fog is an RPG progression axis.
3. **Combat economy: hybrid re-flavor.** Keep the four-meter structure
   (stamina/poise/focus/AP) but re-flavor and re-tune toward aggression — anime skin,
   faster regen, cheaper offense. (Open: exact meter consolidation w/ Ki — asked.)
4. **My role: docs first, then I build everything with the user monitoring.**
   I must understand what I write as well as they do.
   **DO NOT start implementing immediately after writing the plan — wait for sign-off.**

## Round 2 — fighting system

5. **Supers: Heat + Rage + Ki gauge.** Once-per-fight install (buffed frame data),
   low-HP comeback super w/ armor, PLUS a buildable Ki gauge for EX versions and
   cinematic supers. Transformations/beams/escalation = maximal anime.
6. **Stage: walls + authored hazards.** Bounded lanes, wall splat / wall carry,
   per-arena authored hazards (floor/balcony breaks, knock-overboard). Hazards are
   arena DATA, not engine rules.
7. **Topology: party JRPG battles.** Multiple actors per side ON the lane as the
   normal case. (Player + companions vs enemy groups.)
8. **Move source: all three layered.** Forms learned from masters/trainers are the
   movelist source (xianxia, plot-bible First Form); stats/skills gate and scale (L4
   compiler); equipment/loot modifies frame data + grants moves with rolled affixes.

## Round 3 — party / meta

9.  **Party control: player commits for everyone** at their decision points
    (interleaved naturally by ready_tick).
10. **Party size: plan for 3, but N must be a knob** the user can play with.
    Engine supports arbitrary N per side; balance target is 3.
11. **PvP: PvE only.** Fog + prediction UI tuned purely for fun; AI design is free.
12. **Presentation: 2D side-view.** Sprite/skeletal 2D, side camera, anime VFX
    (speed lines, impact frames, super flash cutaways).

## Carried from memory (prior sessions, to fold into the rewrite)

- Orthogonal move axes (decompose MoveLevel/MoveClass; THROW duplication bug).
- Authored qualities: NO engine combat constants; no generic moves.
- No separate Fighter type — Entity holds offense + defenses (static ++ dynamic) + runtime.
- RPG-first: fighting-game mechanics quarantined to Combat.
- Exploration: flooded-world ocean hexcrawl, loot-driven, masters anchor islands.
- Plot bible (the-promise-plot-bible.md) is good AS-IS. fsm.md will need updates
  (party combat, no more 2-fighter assumptions).
