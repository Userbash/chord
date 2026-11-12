#!/usr/bin/env node
// @ts-check
// Chord — High-performance native open-source client & protocol tools.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { run } from "../src/cli/run.mjs";

const args = process.argv.slice(2);
const first = args[0];

if (first === "app" || first === "client" || first === "gui" || first === "start") {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const binPath = existsSync(join(root, "rust/dist/chord"))
    ? join(root, "rust/dist/chord")
    : join(root, "rust/target/release/chord");

  const child = spawn(binPath, args.slice(1), { stdio: "inherit" });
  child.on("exit", (code) => {
    process.exit(code ?? 0);
  });
} else {
  process.exitCode = await run(args);
}
