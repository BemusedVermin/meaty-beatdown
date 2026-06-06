/**
 * canonical.ts — canonical, integers-only JSON for golden vectors [serialize, pure].
 *
 * The whole point of golden vectors is cross-language comparability, so the encoding is pinned:
 *  - stable key order (recursively sorted),
 *  - fixed-point values serialize as their integer raw (a Fixed IS its raw number),
 *  - NO floats anywhere (asserted — a float means a bug leaked real-number math into the wire),
 *  - LF newlines, two-space indent, trailing newline.
 * A port emits/reads the same bytes. (PORTING.md golden-vector schema.)
 */

/** Throw if any number in the structure is non-integer (the integers-only contract). */
export function assertIntegersOnly(value: unknown, path = "$"): void {
  if (typeof value === "number") {
    if (!Number.isInteger(value)) throw new Error(`non-integer on the wire at ${path}: ${value}`);
    return;
  }
  if (value === null || typeof value === "boolean" || typeof value === "string") return;
  if (Array.isArray(value)) {
    value.forEach((v, i) => assertIntegersOnly(v, `${path}[${i}]`));
    return;
  }
  if (typeof value === "object") {
    for (const [k, v] of Object.entries(value)) assertIntegersOnly(v, `${path}.${k}`);
    return;
  }
  throw new Error(`unserializable value at ${path}: ${typeof value}`);
}

function sortKeys(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(sortKeys);
  if (value !== null && typeof value === "object") {
    const out: Record<string, unknown> = {};
    for (const key of Object.keys(value as Record<string, unknown>).sort()) {
      out[key] = sortKeys((value as Record<string, unknown>)[key]);
    }
    return out;
  }
  return value;
}

/** Canonical JSON string: integers-only, key-sorted, 2-space indent, LF, trailing newline. */
export function canonicalJson(value: unknown): string {
  assertIntegersOnly(value);
  return JSON.stringify(sortKeys(value), null, 2) + "\n";
}
