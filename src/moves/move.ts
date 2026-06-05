/**
 * move.ts — the Move authoring container and MoveList (spec §3.2, §3.3) [L3].
 *
 * A Move pairs a stable id + class + display name with the FrameProfile the engine runs. Requirements
 * and scaling (the L4 gates that turn a base profile into a resolved one) are added by the RPG layer
 * (Phase 5) and live there; keeping them out here means moves/ has zero coupling to rpg/.
 *
 * `toMoveTable` is the bridge to the engine's `MoveTable` (moveId → resolved FrameProfile).
 */
import { type MoveId } from "../core/ids";
import { type FrameProfile } from "../core/frameprofile";
import { type MoveTable } from "../core/engine";

/** Move taxonomy (spec §3.2). */
export type MoveClass =
  | "LIGHT"
  | "HEAVY"
  | "COMMAND"
  | "SPECIAL"
  | "REVERSAL"
  | "THROW"
  | "MOVEMENT";

export interface Move {
  readonly id: MoveId;
  readonly name: string;
  readonly moveClass: MoveClass;
  /** The resolved frame data the engine runs (already compiled, in this prototype). */
  readonly profile: FrameProfile;
}

export type MoveList = readonly Move[];

/** Index a MoveList by id into the engine's MoveTable. Throws on duplicate ids. */
export function toMoveTable(moves: MoveList): MoveTable {
  const map = new Map<MoveId, FrameProfile>();
  for (const m of moves) {
    if (map.has(m.id)) throw new Error(`duplicate move id "${m.id}"`);
    map.set(m.id, m.profile);
  }
  return map;
}

/** Look up a move by id (linear; move lists are small). */
export function findMove(moves: MoveList, id: MoveId): Move | undefined {
  return moves.find((m) => m.id === id);
}
