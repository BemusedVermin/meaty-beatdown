/**
 * verify.ts — standalone golden:verify report (also wired into `npm test` via golden.test.ts) [golden].
 *
 * Loads every committed vector, replays its decisions through ReplayAgents, and checks the produced
 * trace canonicalizes to the stored trace byte-for-byte. Exits non-zero if any vector fails.
 */
import { listVectorIds, readVectorFile } from "./io";
import { vectorFromJson } from "../serialize/state";
import { verifyVector } from "./harness";

const ids = listVectorIds();
console.log("");
let pass = 0;
for (const id of ids) {
  const r = verifyVector(vectorFromJson(readVectorFile(id)));
  console.log(`  ${r.ok ? "PASS" : "FAIL"}  ${id} — ${r.detail}`);
  if (r.ok) pass++;
}
console.log(`\n  golden:verify — ${pass}/${ids.length} vectors reproduced byte-identically\n`);
if (ids.length === 0 || pass !== ids.length) process.exitCode = 1;
