# TICK (Rust)

A headless, deterministic **combat + RPG engine** — a from-scratch Rust rebuild. Frame data lives on
a shared tick timeline; "whose turn it is" falls out of a single `ready_tick` comparison.

> **Status: scaffold.** The combat **L2 core** (`fighting`) and a first pass of the **exploration**
> hexcrawl model + generator are built and tested. Deferred: the resource / AP-tempo economy, the
> combo governors, the L3 move taxonomy, the L4 stats→frame-data compiler, and the Bevy systems that
> drive it all. See **[DESIGN.md](./DESIGN.md)**.

## Layout — a two-crate workspace

The dependency arrow runs strictly one way, `app → engine`, so the deterministic core never depends
on the game shell (and never on Bevy's ECS/App/render — only `bevy_math` for geometry). That's what
keeps it portable and golden-vector–testable.

**`engine`** — the Bevy-free deterministic core:

| module | what's here |
|---|---|
| `fighting` | the turn-based combat engine: the tick scheduler + run/pause loop (`sim`), the contact-priority `resolver`, the NEUTRAL/PRESSURE `regime` rule, 3D AABB hit geometry (`space`), the orthogonal move axes (`frame`), the `entity` runtime state, and locked `config` constants |
| `exploration` | the flooded-world hexcrawl: a seeded `worldgen` over a `hex` grid producing a `world` of `terrain` tiles + `poi` (ports / sects / masters) |

**`app`** — the Bevy game shell (the only crate that depends on full Bevy):

| module | what's here |
|---|---|
| `state` | the game-wide FSM axes (`AppState` / `PauseState` / `GameState` / `CombatState`) + the per-actor `ActorState` component — `States`/`SubStates` mirroring `docs/fsm.md` |
| `combat` | the **driver**: the seam that pumps `engine::fighting::Sim` from inside the `Combat` overlay and projects engine state onto ECS for presentation |

## Build

```sh
cargo check                 # type-checks both crates
cargo test -p engine        # the combat + worldgen unit tests
cargo run  -p app           # launch the Bevy shell (opens a window)
```

## Where behavior lives

The engine is the authority: a pure `Sim` (tick clock, contact resolution, effect application) that
**pauses** whenever an actor must choose. The `app` shell owns presentation and input and merely
*drives* it — `Sim::advance` while `FightState::Advancing`, gather + `Sim::commit` on `AwaitInput`,
and project the engine's reaction state onto each actor's `ActorState`. Those driver bodies are
currently `todo!()` stubs in `app/src/combat.rs` (engine/content logic, not shell glue).
