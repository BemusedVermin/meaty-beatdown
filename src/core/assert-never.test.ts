import { describe, it, expect } from "vitest";
import { assertNever } from "./assert-never";

describe("assertNever", () => {
  it("throws with a default message", () => {
    // Cast through unknown because at a real call site the argument is statically `never`.
    expect(() => assertNever(42 as unknown as never)).toThrow(/unexpected value/i);
  });

  it("throws with a custom message", () => {
    expect(() => assertNever("x" as unknown as never, "bad tag")).toThrow("bad tag");
  });

  it("demonstrates exhaustive-switch usage over a tagged union", () => {
    type Shape =
      | { kind: "circle"; r: number }
      | { kind: "square"; s: number };

    const area = (shape: Shape): number => {
      switch (shape.kind) {
        case "circle":
          return 3 * shape.r * shape.r;
        case "square":
          return shape.s * shape.s;
        default:
          return assertNever(shape);
      }
    };

    expect(area({ kind: "circle", r: 2 })).toBe(12);
    expect(area({ kind: "square", s: 3 })).toBe(9);
  });
});
