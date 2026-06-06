/**
 * state.ts — the golden-vector schema + state/table codec (integers only) [serialize, pure].
 *
 * A golden vector is SELF-CONTAINED so a port can replay it without the TS content: it carries the
 * config constants, both move tables (resolved FrameProfiles), the initial MatchState, the recorded
 * per-agent decision streams, the run options, and the expected trace. Deserialization is JSON.parse
 * + a Map rebuild for the move tables; fixed-point values are plain integers, so they re-type as
 * `Fixed` without conversion.
 */
import { type MatchState, type MoveTable, type MatchOptions, type TraceEvent, type Decision } from "../core/engine";
import { type FrameProfile } from "../core/frameprofile";
import { type MoveId } from "../core/ids";
import { canonicalJson } from "./canonical";

export const SCHEMA_VERSION = 1;

export interface GoldenVector {
  readonly schemaVersion: number;
  /** The locked constants the trace depends on (documentation for porters; TS verify uses live CONFIG). */
  readonly config: unknown;
  /** Both fighters' resolved move tables (moveId → FrameProfile). */
  readonly moveTables: readonly [Record<MoveId, FrameProfile>, Record<MoveId, FrameProfile>];
  readonly initialState: MatchState;
  readonly options: MatchOptions;
  /** Each fighter's recorded decision stream, replayed by ReplayAgent. */
  readonly decisions: readonly [readonly Decision[], readonly Decision[]];
  /** The expected tick-by-tick event + entity-state stream. */
  readonly trace: readonly TraceEvent[];
}

export function tableToRecord(t: MoveTable): Record<MoveId, FrameProfile> {
  return Object.fromEntries(t.entries());
}

export function recordToTable(r: Record<MoveId, FrameProfile>): MoveTable {
  return new Map(Object.entries(r));
}

export function vectorToJson(v: GoldenVector): string {
  return canonicalJson(v as unknown);
}

export function vectorFromJson(json: string): GoldenVector {
  return JSON.parse(json) as GoldenVector;
}
