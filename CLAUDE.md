# Repo orientation

This repository contains the language-neutral design plus **two implementations** of the same engine.
Work within whichever subfolder is relevant, and read that folder's own instructions.

- **`docs/`** — the design (source of truth: `frame_rpg_spec.md`) + the mechanics docs. Language-neutral.
- **`golden/`** — the cross-language golden vectors (the behavioral contract). Language-neutral; shared.
- **`ts/`** — the reference implementation (TypeScript, complete). **Its full agent instructions — the
  12 locked decisions, the layered architecture, the enforced module boundaries, and the green gate —
  live in [`ts/CLAUDE.md`](./ts/CLAUDE.md).** Read that before working in `ts/`. Run all npm / just
  commands from inside `ts/`; it reads the shared `golden/` via `../golden`.
- **`rust/`** — a from-scratch rebuild the user is implementing themselves (see `rust/DESIGN.md`).
  Default to scaffolding, reviewing, and answering questions here — do **not** write the engine logic
  unless explicitly asked.
