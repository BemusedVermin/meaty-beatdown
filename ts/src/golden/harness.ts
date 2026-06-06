/**
 * harness.ts — emit a golden vector from a scenario, and verify a vector by replay [golden, edge].
 *
 * emit: run the scenario through RecordingAgents (capturing the decision stream) and snapshot the
 * config, move tables, initial state, options, and trace. verify: rebuild the engine inputs from the
 * vector, replay the decisions through ReplayAgents, and assert the produced trace canonicalizes to
 * the stored trace byte-for-byte. This is the cross-language behavioral contract (audit C-3).
 */
import { runMatch, type MoveTable } from "../core/engine";
import { RecordingAgent, ReplayAgent } from "../cli/agents";
import { CONFIG } from "../core/config";
import {
  type GoldenVector,
  SCHEMA_VERSION,
  tableToRecord,
  recordToTable,
} from "../serialize/state";
import { canonicalTrace } from "../serialize/trace";
import { type RunnableScenario } from "./scenarios";

export function emitVector(s: RunnableScenario): GoldenVector {
  const rec0 = new RecordingAgent(s.agents[0]);
  const rec1 = new RecordingAgent(s.agents[1]);
  const result = runMatch(s.initial, s.tables, [rec0, rec1], s.options);
  return {
    schemaVersion: SCHEMA_VERSION,
    config: CONFIG,
    moveTables: [tableToRecord(s.tables[0]), tableToRecord(s.tables[1])],
    initialState: s.initial,
    options: s.options,
    decisions: [rec0.decisions, rec1.decisions],
    trace: result.trace,
  };
}

export interface VerifyResult {
  readonly ok: boolean;
  readonly detail: string;
}

function firstDiff(expected: string, actual: string): string {
  const a = expected.split("\n");
  const b = actual.split("\n");
  for (let i = 0; i < Math.max(a.length, b.length); i++) {
    if (a[i] !== b[i]) return `trace differs at line ${i + 1}: expected ${a[i] ?? "<eof>"} got ${b[i] ?? "<eof>"}`;
  }
  return "trace differs (length only)";
}

export function verifyVector(v: GoldenVector): VerifyResult {
  if (v.schemaVersion !== SCHEMA_VERSION) {
    return { ok: false, detail: `schemaVersion ${v.schemaVersion} != ${SCHEMA_VERSION}` };
  }
  try {
    const tables: [MoveTable, MoveTable] = [
      recordToTable(v.moveTables[0]),
      recordToTable(v.moveTables[1]),
    ];
    const result = runMatch(
      v.initialState,
      tables,
      [new ReplayAgent(v.decisions[0]), new ReplayAgent(v.decisions[1])],
      v.options,
    );
    const expected = canonicalTrace(v.trace);
    const actual = canonicalTrace(result.trace);
    return expected === actual
      ? { ok: true, detail: `${v.trace.length} trace events reproduced` }
      : { ok: false, detail: firstDiff(expected, actual) };
  } catch (e) {
    return { ok: false, detail: `threw: ${(e as Error).message}` };
  }
}
