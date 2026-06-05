/**
 * emit.ts — `npm run golden:emit` [golden, edge].
 *
 * Writes one canonical, integers-only JSON vector per scenario to golden/. Re-run after an intended
 * behavior change to re-baseline; golden:verify then re-locks it.
 */
import { goldenScenarios } from "./scenarios";
import { emitVector } from "./harness";
import { vectorToJson } from "../serialize/state";
import { writeVectorFile, VECTORS_DIR } from "./io";

let count = 0;
console.log("");
for (const scenario of goldenScenarios()) {
  const vector = emitVector(scenario);
  writeVectorFile(scenario.id, vectorToJson(vector));
  console.log(`  emitted golden/${scenario.id}.json — ${vector.trace.length} trace events`);
  count++;
}
console.log(`\n  ${count} golden vectors written to ${VECTORS_DIR}\n`);
