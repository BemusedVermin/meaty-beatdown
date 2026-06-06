/**
 * trace.ts — canonical trace codec (integers only) [serialize, pure].
 *
 * The trace is the cross-language behavioral contract: golden:verify compares the canonical string of
 * the replayed trace against the stored one. Canonicalization is the shared integers-only JSON encoder.
 */
import { type TraceEvent } from "../core/engine";
import { canonicalJson } from "./canonical";

/** Canonical, integers-only string for a trace (stable key order, LF). */
export function canonicalTrace(trace: readonly TraceEvent[]): string {
  return canonicalJson(trace as unknown);
}
