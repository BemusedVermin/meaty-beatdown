/**
 * io.ts — read/write golden vector files [golden, edge].
 *
 * Vectors live in the repo-root `golden/` directory (data artifacts, committed). npm scripts run from
 * the repo root, so `process.cwd()` resolves there.
 */
import { readFileSync, writeFileSync, mkdirSync, readdirSync, existsSync } from "node:fs";
import { join } from "node:path";

export const VECTORS_DIR = join(process.cwd(), "golden");

export function writeVectorFile(id: string, json: string): void {
  mkdirSync(VECTORS_DIR, { recursive: true });
  writeFileSync(join(VECTORS_DIR, `${id}.json`), json, "utf8");
}

export function listVectorIds(): readonly string[] {
  if (!existsSync(VECTORS_DIR)) return [];
  return readdirSync(VECTORS_DIR)
    .filter((f) => f.endsWith(".json"))
    .map((f) => f.slice(0, -".json".length))
    .sort();
}

export function readVectorFile(id: string): string {
  return readFileSync(join(VECTORS_DIR, `${id}.json`), "utf8");
}
