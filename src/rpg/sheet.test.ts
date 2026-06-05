import { describe, it, expect } from "vitest";
import { roundHalfUp } from "../core/fixed";
import { type Attributes, type Sheet, tempoMod, skillRank, hasFocus } from "./sheet";

const attrs = (o: Partial<Attributes> = {}): Attributes => ({
  str: 0,
  dex: 0,
  con: 0,
  int: 0,
  wis: 0,
  cha: 0,
  ...o,
});

describe("tempoMod — derived blend of DEX and WIS (decision 5)", () => {
  it("equals roundHalfUp((dex + wis) / 2)", () => {
    const cases: ReadonlyArray<readonly [number, number]> = [
      [2, 3],
      [1, 2],
      [0, 0],
      [-1, 2],
      [3, 3],
      [-2, -1],
    ];
    for (const [dex, wis] of cases) {
      expect(tempoMod(attrs({ dex, wis }))).toBe(roundHalfUp((dex + wis) / 2));
    }
  });

  it("does not depend on any other attribute (tempo is derived, not a 7th stat)", () => {
    const base = tempoMod(attrs({ dex: 2, wis: 2 }));
    expect(tempoMod(attrs({ dex: 2, wis: 2, str: 5, cha: 5, con: 5, int: 5 }))).toBe(base);
  });
});

describe("skills and foci", () => {
  const sheet: Sheet = { attributes: attrs({ dex: 2 }), skills: { dagger: 3 }, foci: ["whirlwind"] };
  it("skillRank returns the rank or 0 when untrained", () => {
    expect(skillRank(sheet, "dagger")).toBe(3);
    expect(skillRank(sheet, "greatsword")).toBe(0);
  });
  it("hasFocus checks the build-defining unlocks", () => {
    expect(hasFocus(sheet, "whirlwind")).toBe(true);
    expect(hasFocus(sheet, "iron_guard")).toBe(false);
  });
});
