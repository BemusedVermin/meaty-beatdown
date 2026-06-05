/**
 * fixed.ts — signed 16.16 fixed-point arithmetic. (Decision 10; PORTING.md "Fixed-point contract".)
 *
 * This module is the PORTABILITY LINCHPIN. Every positional/spatial quantity in gameplay is a
 * `Fixed`, never a float, so the engine produces byte-identical results in any language that
 * reimplements these exact semantics. Its tests (fixed.test.ts) are the cross-language contract.
 *
 * Representation
 * --------------
 * A `Fixed` value's underlying JS number IS its signed integer `raw`; the real value it denotes
 * is `raw / 2^16` (= raw / 65536). So `ONE === 65536`, `fromInt(2) === 131072`, half === 32768.
 * Keep world coordinates well within ±2^31 so the 64-bit (BigInt) intermediates used by mul/div
 * stay exact; the JS doubles that hold `raw` are exact for |raw| < 2^53.
 *
 * Rounding semantics (reproduce these EXACTLY in any port — see PORTING.md)
 * ------------------------------------------------------------------------
 *  - add / sub : integer add / sub of raw. Exact.
 *  - mul(a,b)  : 64-bit product, ARITHMETIC right shift by 16 → floors toward −∞.
 *                In a port: i64 intermediate + signed `>>` (Rust/C#/C++ all floor on signed >>).
 *  - div(a,b)  : (a << 16) / b with integer division → truncates toward ZERO.
 *                In a port: i64 intermediate + native integer division (truncates toward zero).
 *  - fromRatio : same as div of two integers → truncates toward ZERO.
 *  - toInt(f)  : floor(raw / 2^16) → floors toward −∞.
 *  - roundHalfUp(x): floor(x + 0.5) → ties round toward +∞ (used for stat→int conversions only).
 */

export type Fixed = number & { readonly __fixed: unique symbol };

/** Number of fractional bits in the 16.16 representation. */
export const FRAC_BITS = 16;

/** The fixed-point value 0. */
export const ZERO: Fixed = 0 as Fixed;

/** The fixed-point value 1.0 (raw 65536). */
export const ONE: Fixed = 65536 as Fixed;

const SHIFT = BigInt(FRAC_BITS); // 16n

/** Construct a Fixed from a whole integer. fromInt(2) denotes 2.0 (raw 131072). */
export function fromInt(n: number): Fixed {
  return (n * 65536) as Fixed;
}

/**
 * Convert a Fixed back to an integer, FLOORING toward −∞.
 * toInt(3.5) === 3, toInt(-3.5) === -4.
 */
export function toInt(f: Fixed): number {
  return Math.floor((f as number) / 65536);
}

/**
 * Convert a Fixed to an integer, rounding half-UP toward +∞ (pure integer: add half, then floor).
 * Used for damage scaling so e.g. 100 × 0.9 (which 16.16 holds as 89.9994) lands on 90, not 89.
 * toIntRound(12.5) === 13, toIntRound(-12.5) === -12. Stays in integer land (never calls toNumber).
 */
export function toIntRound(f: Fixed): number {
  return Math.floor(((f as number) + 32768) / 65536);
}

/**
 * Construct a Fixed from the rational num/den. TRUNCATES toward zero (like native integer
 * division in the target languages). fromRatio(1,2) === half, fromRatio(-1,3) truncates toward 0.
 */
export function fromRatio(num: number, den: number): Fixed {
  return Number((BigInt(num) << SHIFT) / BigInt(den)) as Fixed;
}

/** Integer add of raw. Exact. */
export function add(a: Fixed, b: Fixed): Fixed {
  return ((a as number) + (b as number)) as Fixed;
}

/** Integer sub of raw. Exact. */
export function sub(a: Fixed, b: Fixed): Fixed {
  return ((a as number) - (b as number)) as Fixed;
}

/**
 * Fixed-point multiply. 64-bit product then arithmetic right shift by 16 → FLOORS toward −∞.
 * (BigInt `>>` is an arithmetic shift, so negatives floor — matching signed `>>` in Rust/C#/C++.)
 */
export function mul(a: Fixed, b: Fixed): Fixed {
  return Number((BigInt(a as number) * BigInt(b as number)) >> SHIFT) as Fixed;
}

/**
 * Fixed-point divide. (a << 16) / b with integer division → TRUNCATES toward zero.
 * (BigInt `/` truncates toward zero, matching native integer division in the target languages.)
 */
export function div(a: Fixed, b: Fixed): Fixed {
  return Number((BigInt(a as number) << SHIFT) / BigInt(b as number)) as Fixed;
}

/** Three-way compare. */
export function compare(a: Fixed, b: Fixed): -1 | 0 | 1 {
  if ((a as number) < (b as number)) return -1;
  if ((a as number) > (b as number)) return 1;
  return 0;
}

/** Absolute value. */
export function abs(a: Fixed): Fixed {
  return ((a as number) < 0 ? -(a as number) : (a as number)) as Fixed;
}

/** Lesser of two. */
export function min(a: Fixed, b: Fixed): Fixed {
  return (a as number) < (b as number) ? a : b;
}

/** Greater of two. */
export function max(a: Fixed, b: Fixed): Fixed {
  return (a as number) > (b as number) ? a : b;
}

/** Clamp x into [lo, hi]. Assumes lo ≤ hi. */
export function clamp(x: Fixed, lo: Fixed, hi: Fixed): Fixed {
  if ((x as number) < (lo as number)) return lo;
  if ((x as number) > (hi as number)) return hi;
  return x;
}

/**
 * Convert a Fixed to a JS float — DISPLAY ONLY (decision 10). Never call this inside gameplay
 * logic; eslint bans importing it outside cli/ and balance/.
 */
export function toNumber(f: Fixed): number {
  return (f as number) / 65536;
}

/**
 * Round-half-up toward +∞: floor(x + 0.5). Used for the few stat→int conversions only
 * (e.g. tempoMod = roundHalfUp((dexMod + wisMod) / 2), decision 5). NOT fixed-point.
 *
 * Integer-equivalent for porters: for an integer `n`, roundHalfUp(n / 2) === floor((n + 1) / 2).
 * Examples: roundHalfUp(2.5) === 3, roundHalfUp(-2.5) === -2, roundHalfUp(-0.5) === 0.
 */
export function roundHalfUp(x: number): number {
  return Math.floor(x + 0.5);
}
