/**
 * tick.ts — the atomic unit of time (spec §0.1) [L0].
 *
 * 1 tick = 1 frame at 60 Hz by convention, so frame data ports 1:1 from real fighting games. The
 * wall clock is irrelevant; the tick clock is everything. There is a single global tick counter `T`
 * shared by both combatants — the shared clock that makes whiff-punishing and spacing legible.
 *
 * The scheduler / advanceUntilNextDecision() live in the engine (Phase 3, L2); this L0 module only
 * fixes the time vocabulary. Ticks are plain non-negative integers (a port uses i32); they are not
 * branded (unlike Fixed) because mixing tick counts is not a meaningful bug class and the brand would
 * add cast friction everywhere durations are summed.
 */

/** An absolute index on the global clock `T` (≥ 0). */
export type Tick = number;

/** A duration measured in ticks (≥ 0). Same machine type as Tick; the alias documents intent. */
export type Ticks = number;

/** The clock's origin. */
export const T_ZERO: Tick = 0;
