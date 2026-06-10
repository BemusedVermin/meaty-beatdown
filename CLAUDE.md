# Repo orientation

**TICK** — a party-based, partial-information, turn-based JRPG built on fighting-game frame data
(the imitated fighter is Tekken; the register is Dragonball / One-Punch Man; the world is a
fog-eaten xianxia setting). The docs in `docs/` are the source of truth; implementation was
**signed off 2026-06-10** and is being built in `rust/` phase-by-phase per
`docs/implementation-plan.md`.

## State of the repo (since the 2026-06-09 reboot)

The previous implementation — the Rust workspace (`rust/`), the golden vectors (`golden/`), and
the earlier TypeScript reference — was **deliberately removed** (clean-slate decision). It lives
in git history (`0e2eaae` for rust+golden) as *reference only*: do not cite it as the current
design, and do not restore it. The old golden vectors are dead as a contract; v2 vectors will be
regenerated from the new engine (see `docs/tech-plan.md` §2).

- **`docs/`** — the v2 design suite (the source of truth):
  - `vision-mda.md` — vision, MDA, pillars, presentation direction.
  - `frame_rpg_spec.md` — **the combat spec (v2)**; the canonical reference for all fighting
    mechanics. Its §1 charters (C-DET, C-FOG, C-AUTH, C-FIN, C-QUAR) bind all work in this repo.
  - `exploration.md` / `progression.md` — the fog-world hexcrawl and the no-XP progression.
  - `fsm.md` — the state machines (design-first; kept in lockstep with the spec).
  - `the-promise-plot-bible.md` — setting & narrative (v0.2, fog re-theme). Good as-is;
    edit only on explicit request.
  - `tech-plan.md` / `implementation-plan.md` — architecture and the phased build plan.
  - `archive/` — v1 prototype docs, historical only (banners explain the divergences).
- **`notes/`** — the committed decision log from the design sessions (`01-decisions.md`,
  `02-final-answers.md`, …). Check these before re-asking settled questions. They record *why*
  and *that it was decided* — they are not canon; on any conflict, `docs/` wins. Throwaway
  scratch goes in `notes/scratch/` (gitignored).

## Working agreement (binding)

1. **The sign-off gate is passed** (2026-06-10, recorded in `docs/implementation-plan.md`).
   Confirmed workflow: continuous build, gate commits pushed to origin/master, CI green per
   gate, linear history.
2. **Claude implements, the user monitors.** Build in the phase order of the
   implementation plan; every phase's exit criteria and the standing rules (audit + property
   tests as merge gates, no combat facts computed in `app`, every mechanic names its governor)
   apply.
3. Design changes go through the docs first — code never silently diverges from the spec.
4. When implementation exists: run `cargo` from inside `rust/`; the `app → engine` dependency
   arrow is one-way; the engine stays Bevy-free, float-free, and deterministic.
5. **Use external libraries where it makes sense** (user-directed, e.g. `fixed` for fixed-point
   arithmetic, `cordic` for fixed-point sqrt/trig, `petgraph` for the audit's graph algorithms,
   `proptest` for property tests). Bespoke code is reserved for the domain itself — combat
   rules, content, sim semantics. Reimplementing a crate's job is a review-blocking smell.
   Full policy + crate table: `docs/tech-plan.md` §1.1.
