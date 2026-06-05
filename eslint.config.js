import js from "@eslint/js";
import tseslint from "typescript-eslint";

// Layers that are part of the pure, synchronous, value-based core (decision 12).
// They may never use async/await/Promise — that lives only at the cli/ and golden/ edges.
const SYNC_GLOBS = [
  "src/core/**/*.ts",
  "src/spatial/**/*.ts",
  "src/moves/**/*.ts",
  "src/rpg/**/*.ts",
  "src/balance/**/*.ts",
  "src/serialize/**/*.ts",
];

// Layers that must stay integer/fixed-point in gameplay logic (decision 10): they may not
// pull in `fixed.toNumber`, which produces a float for display only. balance/ is intentionally
// excluded — its budget linter is allowed floats for scoring (it is tooling, not gameplay).
const INTEGER_GLOBS = [
  "src/core/**/*.ts",
  "src/spatial/**/*.ts",
  "src/moves/**/*.ts",
  "src/rpg/**/*.ts",
  "src/serialize/**/*.ts",
];

const ASYNC_BAN = [
  {
    selector: "AwaitExpression",
    message:
      "Core layers are strictly synchronous (decision 12). Async/await lives only in cli/ and golden/.",
  },
  {
    selector: "FunctionDeclaration[async=true]",
    message: "Core layers are strictly synchronous (decision 12). No async functions outside cli/ and golden/.",
  },
  {
    selector: "FunctionExpression[async=true]",
    message: "Core layers are strictly synchronous (decision 12). No async functions outside cli/ and golden/.",
  },
  {
    selector: "ArrowFunctionExpression[async=true]",
    message: "Core layers are strictly synchronous (decision 12). No async functions outside cli/ and golden/.",
  },
  {
    selector: "TSTypeReference[typeName.name='Promise']",
    message: "Core layers are strictly synchronous (decision 12). No Promise types outside cli/ and golden/.",
  },
  {
    selector: "NewExpression[callee.name='Promise']",
    message: "Core layers are strictly synchronous (decision 12). No `new Promise` outside cli/ and golden/.",
  },
];

const TONUMBER_BAN = [
  {
    selector: "ImportSpecifier[imported.name='toNumber']",
    message:
      "fixed.toNumber produces a float for DISPLAY only (decision 10). Gameplay logic must stay in fixed-point; use it only in cli/ and balance/.",
  },
];

export default tseslint.config(
  { ignores: ["node_modules/**", "dist/**", "coverage/**"] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      // Decision 11: every tagged union is matched by an exhaustive switch (assertNever default).
      "@typescript-eslint/switch-exhaustiveness-check": [
        "error",
        { requireDefaultForNonUnion: true },
      ],
    },
  },
  {
    // Decision 12: synchronous, value-based core.
    files: SYNC_GLOBS,
    ignores: ["**/*.test.ts"],
    rules: {
      "no-restricted-syntax": ["error", ...ASYNC_BAN],
    },
  },
  {
    // Decision 10: keep float-producing fixed.toNumber out of gameplay logic.
    files: INTEGER_GLOBS,
    ignores: ["**/*.test.ts"],
    rules: {
      // Merge both bans for these layers (a later block overrides, so include ASYNC_BAN too).
      "no-restricted-syntax": ["error", ...ASYNC_BAN, ...TONUMBER_BAN],
    },
  },
);
