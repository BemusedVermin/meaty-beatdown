# rpg-fighting-game

A headless, deterministic **combat + RPG engine** — a turn-based RPG built on fighting-game frame data,
on a shared tick timeline.

This repo holds the language-neutral **design** plus the **Rust implementation**:

| Path | What it is |
|---|---|
| [`docs/`](./docs/) | The design — source of truth ([`frame_rpg_spec.md`](./docs/frame_rpg_spec.md)) + the mechanics reference & gap analysis. Language-neutral. |
| [`golden/`](./golden/) | Cross-language **golden vectors** — the behavioral conformance contract. Language-neutral. |
| [`rust/`](./rust/) | The implementation — a **from-scratch Rust rebuild** (in progress). See [`rust/README.md`](./rust/README.md) + [`rust/DESIGN.md`](./rust/DESIGN.md). |

The durable deliverables are the design (`docs/`), the pure deterministic core, and the golden
vectors (`golden/`). A TypeScript reference implementation originally generated the golden vectors and
proved out the model; it has since been removed (preserved in git history). The Rust engine must
reproduce the fixed-point reference table and replay every golden vector byte-identically.

Run commands from inside `rust/` (`cd rust`).
