/**
 * dependency-cruiser — the module boundaries ARE the design and the portability story
 * (spec Appendix A; locked decisions, Modularity section). These rules fail the build.
 *
 * @type {import('dependency-cruiser').IConfiguration}
 */
module.exports = {
  forbidden: [
    {
      name: "rpg-bridge-is-compiler-only",
      comment:
        "Only rpg/compiler.ts may import core/, spatial/, or moves/. It is the single L4->engine bridge — the architectural linchpin (spec App. A; audit C-5).",
      severity: "error",
      from: { path: "^src/rpg/", pathNot: "^src/rpg/compiler\\.ts$" },
      to: { path: "^src/(core|spatial|moves)(/|$)" },
    },
    {
      name: "core-imports-nothing-upward",
      comment:
        "core/ is the foundation; it must not import rpg/, cli/, balance/, or golden/.",
      severity: "error",
      from: { path: "^src/core/" },
      to: { path: "^src/(rpg|cli|balance|golden)(/|$)" },
    },
    {
      name: "lower-layers-pure-of-rpg-and-cli",
      comment:
        "spatial/, moves/, and serialize/ must not import rpg/ or cli/.",
      severity: "error",
      from: { path: "^src/(spatial|moves|serialize)/" },
      to: { path: "^src/(rpg|cli)(/|$)" },
    },
    {
      name: "io-only-at-edges",
      comment:
        "Only cli/ and golden/ may touch node I/O builtins — the rest of the engine is pure and synchronous (decision 12).",
      severity: "error",
      from: { path: "^src/", pathNot: "^src/(cli|golden)/" },
      to: { dependencyTypes: ["core"] },
    },
    {
      name: "no-circular",
      comment: "No circular dependencies anywhere.",
      severity: "error",
      from: {},
      to: { circular: true },
    },
  ],
  options: {
    doNotFollow: { path: "node_modules" },
    exclude: { path: "(\\.test\\.ts$|node_modules)" },
    tsConfig: { fileName: "tsconfig.json" },
    tsPreCompilationDeps: true,
    enhancedResolveOptions: {
      extensions: [".ts", ".js", ".json"],
      mainFields: ["module", "main", "types"],
    },
  },
};
