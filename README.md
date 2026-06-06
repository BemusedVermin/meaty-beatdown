# rpg-fighting-game

A headless, deterministic **combat + RPG engine** — a turn-based RPG built on fighting-game frame data,
on a shared tick timeline.

This repo holds the language-neutral **design** plus **two implementations**:

| Path | What it is |
|---|---|
| [`docs/`](./docs/) | The design — source of truth ([`frame_rpg_spec.md`](./docs/frame_rpg_spec.md)) + the mechanics reference & gap analysis. Language-neutral. |
| [`golden/`](./golden/) | Cross-language **golden vectors** — the behavioral conformance contract. Language-neutral. |
| [`ts/`](./ts/) | The **reference implementation** (TypeScript, complete). See [`ts/README.md`](./ts/README.md). |
| [`rust/`](./rust/) | A **from-scratch rebuild** (in progress). See [`rust/README.md`](./rust/README.md) + [`rust/DESIGN.md`](./rust/DESIGN.md). |

The TypeScript is the disposable reference; the durable deliverables are the design (`docs/`), the pure
deterministic core, and the golden vectors (`golden/`). Every implementation must reproduce the
fixed-point reference table and replay every golden vector byte-identically — the porting contract is
[`ts/PORTING.md`](./ts/PORTING.md).

Run each implementation's commands from inside its own folder (`cd ts` / `cd rust`).
