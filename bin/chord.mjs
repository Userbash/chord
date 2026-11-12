#!/usr/bin/env node
// @ts-check
// Chord — High-performance native open-source client & protocol tools.
import { run } from "../src/cli/run.mjs";

process.exitCode = await run(process.argv.slice(2));
