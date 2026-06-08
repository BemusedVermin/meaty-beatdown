# TICK (Rust)

A turn-based **RPG with fighting-game-inspired combat**, set in a flooded xianxia world you sail as
an ocean hexcrawl. A from-scratch Rust rebuild: a headless, deterministic engine where frame data
lives on a shared tick timeline — "whose turn it is" falls out of a single `ready_tick` comparison —
driven by a thin Bevy shell.

> **Status: playable prototype.** It boots straight into the overworld; sail into an encounter and a
> real fight runs end-to-end — fighters compiled from rolled stats, animated stick figures, a guard
> option, and an outcome screen. Deferred: the resource / AP-tempo economy, the multi-move combo
> governors, the L3 move taxonomy, loot / foci, a real menu, and golden-vector replay. See
> **[DESIGN.md](./DESIGN.md)**.

## Run it

**Prerequisites:** a recent stable **Rust toolchain — 1.85 or newer** (the workspace is on Rust
**edition 2024**). Install via [rustup](https://rustup.rs). On Windows and macOS that's all you need;
on Linux, Bevy needs the usual system packages (`alsa`, `udev`, …) — see
[Bevy's Linux setup](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md).

Run every `cargo` command from **inside this `rust/` folder**:

```sh
cargo run -p app            # launch the game (opens a window)
cargo test                  # the whole suite (engine + app)
cargo test -p engine        # just the engine: combat, worldgen, content
cargo check                 # fast type-check of both crates
```

The **first** build compiles all of Bevy and takes a while; later builds are fast. For smoother
visuals add `--release` (`cargo run -p app --release`) — slower to compile, faster to run.

### Controls

| where | keys |
|---|---|
| **Overworld** | `W` / `↑` sail forward · `A` / `D` (or `←` / `→`) turn the ship · sail onto a **red marker** to start a fight |
| **Combat** | number keys `1`–`9` use your moves (listed in the HUD, including **Guard**) · `Space` waits |
| **Outcome screen** | `Enter` / `Space` to continue |

### Debug trace

While running, the game writes a plain-text trace to **`tick-debug.log`** in the working directory
(so `rust/tick-debug.log` when launched from here — the resolved path is printed to the console at
startup). It records state transitions, the fight setup with every move's frame data, per-tick HP /
reaction changes, every decision + commit, and overworld travel. It's view-only — it never touches
the simulation. Read it (or paste lines from it) when combat does something unexpected.

## Layout — a two-crate workspace

The dependency arrow runs strictly one way, `app → engine`, so the deterministic core never depends
on the game shell (and never on Bevy's ECS/App/render — only `bevy_math` for geometry). That's what
keeps it portable and golden-vector–testable.

**`engine`** — the Bevy-free deterministic core (deps: `bevy_math`, `hexx`, `fastrand`, `fixed`):

| module | what's here |
|---|---|
| `fighting` | the turn-based combat engine: the tick scheduler + run/pause loop (`sim`, exposing `step` / `advance`), the contact-priority `resolver`, the NEUTRAL/PRESSURE `regime`, 3D AABB hit geometry (`space`), the composable move qualities (`frame`), the `entity` runtime state, and doc-only `config` |
| `exploration` | the flooded-world hexcrawl: a seeded `worldgen` over a `hex` grid producing a `world` of `terrain` tiles + `poi` (ports / sects / masters) + `encounter`s, plus `travel` (sailing) |
| `content` | the **L4 RPG layer** — the compiler `Build → Fighter` (body + moves). Stats/skills/equipment are compilers; the **move is the unit of combat** they produce. WWN-style: `attributes` (3d6), `morphology` (biped/quadruped) + modifiers (fanged/clawed), `equipment`, `moves`, `compile`, and seeded `generate` |

**`app`** — the Bevy game shell (the only crate that depends on full `bevy = 0.18`):

| module | what's here |
|---|---|
| `state` | the game-wide FSM axes (`AppState` / `PauseState` / `GameState` / `CombatState` + `FightState`) + the per-actor `ActorState` component — `States` / `SubStates` mirroring `docs/fsm.md` |
| `combat` | the **combat driver**: compiles an encounter into fighters and pumps `engine::fighting::Sim` one tick at a time inside the `Combat` overlay (so moves animate), reading player input on each decision |
| `exploration` | the **overworld driver**: owns the generated `World` + the ship, sails the hexcrawl, and raises the combat overlay on an encounter |
| `render` | minimal 2D presentation: a hex-mesh overworld + a combat view drawing each fighter as an animated stick figure, plus the text HUD |
| `debuglog` | the file trace described above |

## Where behavior lives

The engine is the authority: a pure `Sim` (tick clock, contact resolution, effect application) that
**pauses** whenever an actor must choose. The `app` shell owns presentation and input and merely
*drives* it — stepping the `Sim` while `FightState::Advancing`, gathering input + `Sim::commit` on
`AwaitInput`, and raising the matching `CombatState` outcome when it ends. Every combat magnitude is
**authored on the move** (no engine combat constants), and moves are **morphology-gated** — a fighter
can only use a move whose required body parts it has. The content layer is the single bridge that
turns a stat block into those moves.
