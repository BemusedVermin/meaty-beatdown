# DESIGN — the decomposed model

This crate is built on a few decisions we converged on. Each is encoded directly in the types.

## Three systems

1. **Frame resolution engine** — pure: frame data + spatial state → a contact outcome + advantage.
   Knows nothing about resources or fighters. (`spatial::does_hit`, `resolver::classify_contact`,
   `engine::compute_regime`.)
2. **Fight engine** — produces frame data into the resolver and *consumes its results to mutate
   fighter status*. This is where a hit's effects are transformed by the defender and applied.
   (`fight`, `engine::run_match`.)
3. **Fighter production** — the build (stats + equipment) compiles into an `Entity`. The single
   bridge; all stat→frame-data knowledge lives here. (`rpg`.)

These three systems are **layers within the single Bevy-free `engine` crate** (the data model, the
resolution + fight engine, and — once built — fighter production are modules, not separate crates).
The Bevy game shell is a second crate, **`app`**, that depends on `engine` and *drives* it. The
dependency arrow runs one way, `app → engine`, so the deterministic core never reaches up into the
loop and never depends on Bevy's ECS/App — only `bevy_math` for geometry.

`MatchState.entities` is a `Vec`, not a 2-tuple, and `compute_regime` already handles N entities — so
"many competitors" is left open rather than hardcoded to two.

## Moves are built from orthogonal axes

"What kind of attack" is **not** one flat enum. It's independent axes you compose:

- **TYPE** — `AttackProfile::Strike` vs `Throw`. A throw has no height and no blockability — so
  "blockable throw" / "high throw" are *unrepresentable*.
- **HEIGHT** — `GuardHeight::{High,Mid,Low}` (strikes only). Overhead = High, low = Low; which stance
  blocks it emerges from `Property::Block { covers }`.
- **BLOCKABILITY** — `StrikeDefense::{Blockable { blockstun }, Unblockable}`. Blockstun lives inside
  `Blockable`, so only a blockable thing can have it.

A move that doesn't attack (movement, block, parry) simply has `attack: None`.

## A hit's consequences = one reaction + a gated effect list

- **Reaction** (`moves::Reaction`) — the single state a connecting hit produces:
  `Hitstun | Launch | Knockdown | Crumple | Stagger`. This replaces the old `launches`/`knockdown`
  boolean pair (no flag soup; the contradiction is a type error). `on_counter` optionally overrides
  it — that's the counter-hit-only launcher.
- **Effects** (`moves::GatedEffect`) — a composable, gated list: `Damage{resource} | Knockback |
  ResourceGain{who,resource} | Status`. Chip = `Damage(Poise)` gated `OnBlock`; AP gain =
  `ResourceGain(Ap)` gated `OnHit`. Damage to *any* pool is uniform.

## Advantage is derived, never stored (invariant I-1)

There are no `on_hit`/`on_block` fields. `moves::on_hit` = `reaction_duration(onHit) − recovery`;
`moves::on_block` = `blockstun − recovery`. The compiler must never set advantage directly.

## The Entity is the fighter (no separate `Fighter` type)

An `Entity` carries its **offensive profile** (`move_ids`), its **defensive profile** (`defenses`),
and its **runtime state**. `defenses` are incoming-effect transforms (`DamageResist`, `StatusImmune`,
…); a fighter's *effective* defense is the static `defenses` plus any derived from active
`status_effects`. The fight engine runs `apply_defenses` between *produce* and *apply*.

## Fixed-point (16.16) — reproduce exactly

`mul` floors via arithmetic `>>16`; `div`/`from_ratio` truncate toward zero; `to_int` floors;
`to_int_round` is half-up. The reference-table test in `fixed.rs` is the first acceptance test.

## Open forks (not yet decided)

- Offensive profile: `Entity.move_ids` + a shared `MoveTable` (the symmetric choice taken here) vs. a
  per-entity move table. Confirm before going further.
- `Status` effects + the dynamic-defense derivation from `status_effects`: stubbed; wire up together.
- `Defense::ReactionArmor` shape (ignore/downgrade launch by weight) — placeholder.
