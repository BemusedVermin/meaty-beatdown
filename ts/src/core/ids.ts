/**
 * ids.ts — stable identifier types (a dependency leaf) [L0].
 *
 * Entities, moves, and (future) projectiles/status-effects are referenced by stable IDs, never by
 * object identity (portability contract). Strings keep golden vectors human-readable; a port may use
 * integers. Kept in their own leaf module so cost/cancel data (core/cost.ts) can name a MoveId
 * without importing entity.ts (which would form a cycle through frameprofile.ts).
 */
export type EntityId = string;
export type MoveId = string;
