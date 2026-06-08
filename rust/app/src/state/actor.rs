//! `ActorState` — the per-fighter combat FSM. A **component** (one instance per fighter), *not* a
//! global `States`: every actor advances its own copy in parallel while `CombatState::Fight` is
//! live. When an actor reaches `KO` it is removed from its side; a side with no actors left is
//! eliminated, which drives the `Fight` outcome.
//!
//! Mirrors **Combat Actor State Diagram** in `docs/fsm.md`. In the diagram, ▶ marks the states in
//! which the actor *feeds the engine input* (`Idle`, the `Recovery` cancel window, `WakeUp`); every
//! other state is locked and only *receives*.

use bevy::prelude::*;

/// The per-actor combat FSM. Attach one to each fighter entity for the duration of a fight.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ActorState {
    // ── Offense (move execution) ──────────────
    /// ▶ Neutral and fully actionable; auto-faces the opponent and chooses the next action.
    #[default]
    Idle,
    /// Committed to a move, winding up (the move's `startup` frames). Vulnerable to counter-hits.
    Startup,
    /// The move's `active` frames; this actor's hitbox is live. May be parried, or trade into hit.
    Active,
    /// The move's `recovery` frames. Locked, except a cancel window (▶) chains into the next move.
    Recovery,

    // ── Block / guard ─────────────────────────
    /// Holding guard against a connected attack (the move's `blockstun`); takes chip.
    Blockstun,
    /// Guard shattered: a long, punishable stun.
    GuardBroken,

    // ── Throws ────────────────────────────────
    /// Grabbed (the tech window already passed). → `KnockedDown`.
    Thrown,
    /// Mutual throw clash; transient, no damage. → `Idle`.
    Teched,

    // ── Hit reactions ─────────────────────────
    /// Reeling from a clean hit (the move's `hitstun`).
    Hitstun,
    /// This actor's *own* attack was parried: frozen and punishable.
    Parried,

    // ── Launch / juggle / okizeme ─────────────
    /// Launched into a juggle; can be re-hit (the self-loop extends air hitstun).
    Airborne,
    /// On the ground (okizeme).
    KnockedDown,
    /// ▶ Getting up; may offer a reversal action.
    WakeUp,

    // ── End ───────────────────────────────────
    /// Health depleted; out of the fight (terminal). Removes the actor from its side.
    KO,
}
