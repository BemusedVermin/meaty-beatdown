# Phase 5 implementation decisions — 2026-06-11

Status: implementation record, not canon. `docs/` remains the source of truth.

Phase 5 was implemented against the signed-off plan with these execution decisions:

1. **Heat variants are explicit moves.** Heat-only moves are authored as normal `Move`
   entries with a `heat_only` flag. The future UI/compiler can hide them outside Heat;
   the engine simply enforces legality.
2. **Heat entry is authored on move flags.** `heat_burst` starts Heat on commit;
   `heat_engager` starts Heat on clean hit. Test content uses a 300-tick Heat duration.
3. **Rage is per actor.** `DefenseProfile` owns the HP threshold and passive damage
   scalar. Rage latches from damage bookkeeping, and Rage Art is an authored once-only
   move flag.
4. **Rage Art consumes on commit.** The Phase 5 engine marks the Rage Art latch spent
   when the move is committed, matching the once-only comeback-super contract.
5. **Missiles are independent runtime entities.** A move can author a `ProjectileSpec`;
   the spawned missile has its own position, facing, speed, lifetime, rectangular hitbox,
   and hit event. Opposing missile overlap annihilates both missiles.
6. **Beams remain frame data.** Long, narrow, multi-tick envelopes are ordinary moves,
   so existing multi-victim and governor logic applies.
7. **Hazards are rectangular authored volumes.** Phase 5 supports `Once`,
   `Cooldown`, and `Always` trigger archetypes over axis-aligned rectangles.
8. **Budget residuals are not enforced yet.** The audit records structural coverage of
   the new Phase 5 axes (`w_arc`, `w_track`, `w_meter`, `w_lie`, Heat, projectile, super)
   and rejects malformed authoring, but does not assert numeric budget fit.

Verification at implementation time: `cargo test` from `rust/` was green before this
note was added; final gate commands should be rerun after note/format cleanup.
