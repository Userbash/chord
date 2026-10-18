#!/usr/bin/env node
// @ts-check
// discord-lab — CLI лаборатории разбора Discord-клиента.
// Тонкая точка входа: вся логика — в src/cli/run.mjs.
import { run } from "../src/cli/run.mjs";

process.exitCode = await run(process.argv.slice(2));
