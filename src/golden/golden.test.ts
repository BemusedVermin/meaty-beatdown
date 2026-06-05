import { describe, it, expect } from "vitest";
import { listVectorIds, readVectorFile } from "./io";
import { vectorFromJson } from "../serialize/state";
import { verifyVector } from "./harness";

/**
 * The cross-language behavioral contract, wired into `npm test`: every committed golden vector must
 * replay byte-identically. A failure means either a real behavior change (re-run `npm run golden:emit`
 * if intended) or a regression. If this is empty, the vectors were never emitted/committed.
 */
describe("golden vectors replay byte-identically", () => {
  const ids = listVectorIds();

  it("has committed golden vectors", () => {
    expect(ids.length).toBeGreaterThan(0);
  });

  for (const id of ids) {
    it(`${id} reproduces its stored trace`, () => {
      const result = verifyVector(vectorFromJson(readVectorFile(id)));
      expect(result.ok, result.detail).toBe(true);
    });
  }
});
