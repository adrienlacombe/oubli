import { build } from "esbuild";

await build({
  entryPoints: ["src/index.ts"],
  bundle: true,
  outfile: "../crates/oubli-swap/js/bundle.js",
  format: "iife",
  globalName: "OubliSwap",
  platform: "browser",
  target: "es2020",
  minify: true,
  sourcemap: false,
  // Resolve main fields — prefer browser/module over CJS main
  mainFields: ["browser", "module", "main"],
  // Map Node.js conditions to browser equivalents
  conditions: ["browser", "import", "default"],
  // Polyfill Node.js built-ins that don't exist in QuickJS
  define: {
    "process.env.NODE_ENV": '"production"',
    "global": "globalThis",
  },
});

console.log("Bundle built → ../crates/oubli-swap/js/bundle.js");
