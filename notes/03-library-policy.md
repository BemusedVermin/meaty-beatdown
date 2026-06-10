# Library policy — 2026-06-10

User directive (second time on record; first was "for the love of God, use a library" re:
collision, 2026-06-07): **WE WILL USE external libraries where it makes sense** — e.g. `fixed`
for fixed-point arithmetic rather than writing our own.

## Reassessment of the tech plan against the policy

- ✅ Already compliant: `fixed`, `hexx`, `fastrand`, `serde` (+`ron`), Bevy on the app side.
- ❌ **Violation found & fixed:** the plan had us hand-rolling "deterministic sqrt/projection
  helpers" in `core::fx`. Replaced with **`cordic`** (fixed-point sqrt/trig over `fixed`
  types; `fixed-sqrt` as the narrower fallback) — evaluated and frozen at Phase 0 behind the
  determinism gate. `core::fx` is now defined as *thin glue only* (a vec2 newtype delegating
  arithmetic to `fixed`, sqrt/trig to `cordic`).
- ➕ Named libraries where the plan was silent: **`petgraph`** for the audit's cancel-graph
  cycle scans (R-5/R-6), **`proptest`** for the anti-infinite property suites, **`insta`** as
  a Phase-1 candidate for trace snapshots; app-side candidates (input-manager, `bevy_egui`,
  tweening) flagged for Phase 7 evaluation.
- 🚫 The one principled refusal stands: no `f32` math (e.g. `bevy_math`) inside the engine —
  the v1 prototype took that trade and lost strict determinism; v2 doesn't. Declining a
  library is only legitimate when it would cost correctness.

Written into: `docs/tech-plan.md` §1.1 (policy + crate table), implementation-plan Phase 0/2,
CLAUDE.md working agreement #5, memory `prefer-external-libraries`.
