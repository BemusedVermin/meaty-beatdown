/**
 * assert-never.ts — exhaustiveness guard for tagged unions (decision 11).
 *
 * Every sum type in this engine is a discriminated union with a literal `kind`/`tag` field,
 * matched by an exhaustive `switch` whose `default` calls `assertNever(x)`. If a new variant is
 * added without a matching case, the call site fails to type-check (the argument is no longer
 * `never`) — and `@typescript-eslint/switch-exhaustiveness-check` flags it too. This maps 1:1 onto
 * a Rust `match` with no wildcard arm / a C# switch expression that must be exhaustive.
 */
export function assertNever(value: never, message?: string): never {
  throw new Error(message ?? `assertNever: unexpected value ${JSON.stringify(value)}`);
}
