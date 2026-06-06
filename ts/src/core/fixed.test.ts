/**
 * fixed.test.ts — the cross-language behavioral contract for 16.16 fixed-point (decision 10).
 *
 * These hand-authored reference tables (with negatives and rounding edges) pin down the EXACT
 * bits any port must reproduce. When the engine is reimplemented elsewhere, a translated copy of
 * this table is the acceptance test for that port's `Fixed` (see PORTING.md).
 */
import { describe, it, expect } from "vitest";
import {
  type Fixed,
  FRAC_BITS,
  ZERO,
  ONE,
  fromInt,
  toInt,
  toIntRound,
  fromRatio,
  add,
  sub,
  mul,
  div,
  compare,
  abs,
  min,
  max,
  clamp,
  toNumber,
  roundHalfUp,
} from "./fixed";

/** Build a Fixed directly from its raw integer (test helper — mirrors the wire representation). */
const raw = (n: number): Fixed => n as Fixed;

describe("representation constants", () => {
  it("ONE is raw 65536 and FRAC_BITS is 16", () => {
    expect(FRAC_BITS).toBe(16);
    expect(ONE as number).toBe(65536);
    expect(ZERO as number).toBe(0);
  });

  it("fromInt encodes whole numbers as raw * 2^16", () => {
    expect(fromInt(0) as number).toBe(0);
    expect(fromInt(1) as number).toBe(65536);
    expect(fromInt(2) as number).toBe(131072);
    expect(fromInt(-3) as number).toBe(-196608);
  });
});

describe("mul — reference table (floors toward −∞ via arithmetic shift)", () => {
  // [a_raw, b_raw, expected_raw]
  const table: ReadonlyArray<readonly [number, number, number]> = [
    [65536, 65536, 65536], //  1.0 * 1.0 = 1.0
    [131072, 196608, 393216], //  2.0 * 3.0 = 6.0
    [32768, 32768, 16384], //  0.5 * 0.5 = 0.25
    [-32768, 32768, -16384], // -0.5 * 0.5 = -0.25 (exact, no flooring)
    [-32768, -32768, 16384], // -0.5 * -0.5 = 0.25
    [21845, 65536, 21845], //  (≈1/3) * 1.0  = ≈1/3 (exact: x*1)
    // Flooring edges: tiny raws whose true product is a sub-ULP fraction.
    [1, 1, 0], //  +tiny * +tiny → floors to 0
    [-1, 1, -1], //  −tiny * +tiny → floors toward −∞ to −1 (NOT 0) — the key sign edge
    [-1, -1, 0], //  −tiny * −tiny → +tiny → floors to 0
    [3, 65535, 2], //  floor(3*65535/65536) = floor(2.99995) = 2
  ];

  for (const [a, b, expected] of table) {
    it(`mul(${a}, ${b}) === ${expected}`, () => {
      expect(mul(raw(a), raw(b)) as number).toBe(expected);
    });
  }

  it("commutativity: mul(a,b) === mul(b,a)", () => {
    const samples: ReadonlyArray<readonly [number, number]> = [
      [65536, 32768],
      [-131072, 21845],
      [99999, -1],
      [12345, 67890],
    ];
    for (const [a, b] of samples) {
      expect(mul(raw(a), raw(b)) as number).toBe(mul(raw(b), raw(a)) as number);
    }
  });
});

describe("div — reference table (truncates toward zero)", () => {
  // [a_raw, b_raw, expected_raw]
  const table: ReadonlyArray<readonly [number, number, number]> = [
    [65536, 131072, 32768], //  1.0 / 2.0 = 0.5
    [65536, 196608, 21845], //  1.0 / 3.0 = 0.333… → 21845 (trunc)
    [-65536, 196608, -21845], // -1.0 / 3.0 → -21845 (toward zero, NOT -21846)
    [-65536, 131072, -32768], // -1.0 / 2.0 = -0.5
    [458752, 131072, 229376], //  7.0 / 2.0 = 3.5
    [-458752, 131072, -229376], // -7.0 / 2.0 = -3.5
    [65536, 65536, 65536], //  1.0 / 1.0 = 1.0
  ];

  for (const [a, b, expected] of table) {
    it(`div(${a}, ${b}) === ${expected}`, () => {
      expect(div(raw(a), raw(b)) as number).toBe(expected);
    });
  }
});

describe("fromRatio — reference table (truncates toward zero)", () => {
  // [num, den, expected_raw]
  const table: ReadonlyArray<readonly [number, number, number]> = [
    [1, 2, 32768],
    [1, 3, 21845],
    [-1, 3, -21845], // truncates toward zero, not −21846
    [2, 3, 43690],
    [7, 4, 114688], // 1.75
    [9, 10, 58982], // 0.9 → 58982 (trunc of 58982.4) — the juggle-decay constant
    [5, 4, 81920], // 1.25 → exact — the counter-hit constant
    [-7, 4, -114688],
  ];

  for (const [num, den, expected] of table) {
    it(`fromRatio(${num}, ${den}) === ${expected}`, () => {
      expect(fromRatio(num, den) as number).toBe(expected);
    });
  }
});

describe("toInt — floors toward −∞", () => {
  it("round-trips whole integers: toInt(fromInt(n)) === n", () => {
    for (let n = -5; n <= 5; n++) {
      expect(toInt(fromInt(n))).toBe(n);
    }
  });

  it("floors fractional values toward −∞", () => {
    expect(toInt(fromRatio(7, 2))).toBe(3); //  3.5 → 3
    expect(toInt(fromRatio(-7, 2))).toBe(-4); // -3.5 → -4 (floor, not trunc)
    expect(toInt(fromRatio(1, 3))).toBe(0); //  0.333 → 0
    expect(toInt(fromRatio(-1, 3))).toBe(-1); // -0.333 → -1
  });
});

describe("toIntRound — rounds half-up toward +∞ (pure integer)", () => {
  it("rounds at the half and matches toInt away from it", () => {
    expect(toIntRound(fromRatio(7, 2))).toBe(4); //  3.5 → 4
    expect(toIntRound(fromRatio(-7, 2))).toBe(-3); // -3.5 → -3 (half-up toward +∞)
    expect(toIntRound(fromRatio(5, 2))).toBe(3); //  2.5 → 3
    expect(toIntRound(fromInt(4))).toBe(4);
    expect(toIntRound(fromRatio(1, 3))).toBe(0); //  0.333 → 0
    expect(toIntRound(fromRatio(2, 3))).toBe(1); //  0.667 → 1
  });
});

describe("add / sub — integer-exact", () => {
  it("add and sub operate on raw exactly", () => {
    expect(add(fromInt(2), fromInt(3)) as number).toBe(fromInt(5) as number);
    expect(sub(fromInt(2), fromInt(3)) as number).toBe(fromInt(-1) as number);
    expect(add(fromRatio(1, 2), fromRatio(1, 2)) as number).toBe(ONE as number);
  });

  it("add is commutative and associative over sampled raws", () => {
    const xs = [0, 1, -1, 65536, -32768, 123456, -999999].map(raw);
    for (const a of xs) {
      for (const b of xs) {
        expect(add(a, b) as number).toBe(add(b, a) as number);
        for (const c of xs) {
          const left = add(add(a, b), c) as number;
          const right = add(a, add(b, c)) as number;
          expect(left).toBe(right);
        }
      }
    }
  });
});

describe("compare / abs / min / max / clamp", () => {
  it("compare is a correct three-way comparator", () => {
    expect(compare(fromInt(1), fromInt(2))).toBe(-1);
    expect(compare(fromInt(2), fromInt(2))).toBe(0);
    expect(compare(fromInt(3), fromInt(2))).toBe(1);
    expect(compare(fromInt(-1), fromInt(-2))).toBe(1);
  });

  it("abs", () => {
    expect(abs(fromInt(-3)) as number).toBe(fromInt(3) as number);
    expect(abs(fromInt(3)) as number).toBe(fromInt(3) as number);
    expect(abs(ZERO) as number).toBe(0);
  });

  it("min / max", () => {
    expect(min(fromInt(2), fromInt(5)) as number).toBe(fromInt(2) as number);
    expect(max(fromInt(2), fromInt(5)) as number).toBe(fromInt(5) as number);
    expect(min(fromInt(-2), fromInt(-5)) as number).toBe(fromInt(-5) as number);
  });

  it("clamp", () => {
    const lo = fromInt(0);
    const hi = fromInt(10);
    expect(clamp(fromInt(-3), lo, hi) as number).toBe(fromInt(0) as number);
    expect(clamp(fromInt(5), lo, hi) as number).toBe(fromInt(5) as number);
    expect(clamp(fromInt(99), lo, hi) as number).toBe(fromInt(10) as number);
  });
});

describe("roundHalfUp — ties toward +∞", () => {
  const table: ReadonlyArray<readonly [number, number]> = [
    [2.5, 3],
    [-2.5, -2],
    [2.4, 2],
    [-2.4, -2],
    [0.5, 1],
    [-0.5, 0],
    [1.5, 2],
    [-1.5, -1],
    [0, 0],
    [3, 3],
  ];
  for (const [x, expected] of table) {
    it(`roundHalfUp(${x}) === ${expected}`, () => {
      expect(roundHalfUp(x)).toBe(expected);
    });
  }

  it("integer-equivalent for porters: roundHalfUp(n/2) === floor((n+1)/2)", () => {
    for (let n = -8; n <= 8; n++) {
      expect(roundHalfUp(n / 2)).toBe(Math.floor((n + 1) / 2));
    }
  });
});

describe("toNumber — display only", () => {
  it("divides raw by 2^16", () => {
    expect(toNumber(ONE)).toBe(1);
    expect(toNumber(fromRatio(1, 2))).toBe(0.5);
    expect(toNumber(fromInt(-3))).toBe(-3);
  });
});

// ---------------------------------------------------------------------------
// Property tests — mul/div agree with rational arithmetic within the defined rounding.
// Deterministic PRNG (seeded mulberry32) so failures are reproducible across runs/machines.
// ---------------------------------------------------------------------------

function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Independent floor-toward-−∞ division on BigInt (NOT a shift — so it cross-checks mul). */
function bigFloorDiv(n: bigint, d: bigint): bigint {
  let q = n / d;
  const r = n % d;
  if (r !== 0n && r < 0n !== d < 0n) q -= 1n;
  return q;
}

describe("property: mul floors a*b/2^16 toward −∞", () => {
  it("matches an independent BigInt floor-div over 5000 seeded samples", () => {
    const rng = mulberry32(0x1234abcd);
    // Coordinates kept well within ±2^31 (raws up to ±~1.05e6 ≈ ±16.0 in world units).
    const range = 1 << 21;
    for (let i = 0; i < 5000; i++) {
      const a = Math.floor((rng() - 0.5) * 2 * range);
      const b = Math.floor((rng() - 0.5) * 2 * range);
      const expected = Number(bigFloorDiv(BigInt(a) * BigInt(b), 65536n));
      expect(mul(raw(a), raw(b)) as number).toBe(expected);
    }
  });
});

describe("property: div truncates (a<<16)/b toward zero and reduces magnitude", () => {
  it("matches an independent BigInt trunc-div over 5000 seeded samples", () => {
    const rng = mulberry32(0x0badf00d);
    const range = 1 << 21;
    for (let i = 0; i < 5000; i++) {
      const a = Math.floor((rng() - 0.5) * 2 * range);
      let b = Math.floor((rng() - 0.5) * 2 * range);
      if (b === 0) b = 1;
      // BigInt `/` truncates toward zero — the documented div semantics.
      const expected = Number((BigInt(a) << 16n) / BigInt(b));
      expect(div(raw(a), raw(b)) as number).toBe(expected);
    }
  });
});

describe("property: mul/div round-trip within one ULP for exact-ish values", () => {
  it("div then mul recovers the dividend to within 1 raw unit", () => {
    const rng = mulberry32(0xfeedface);
    for (let i = 0; i < 2000; i++) {
      const a = raw(Math.floor((rng() - 0.5) * 2 * (1 << 18)));
      let bRaw = Math.floor((rng() - 0.5) * 2 * (1 << 16));
      if (bRaw === 0) bRaw = 1;
      const b = raw(bRaw);
      const back = mul(div(a, b), b);
      // (a/b)*b reconstructs a; div's trunc (<1 ULP) scaled by |b|≤1.0 plus mul's floor (<1 ULP)
      // bounds the total error below 2 raw units.
      expect(Math.abs((back as number) - (a as number))).toBeLessThanOrEqual(2);
    }
  });
});
