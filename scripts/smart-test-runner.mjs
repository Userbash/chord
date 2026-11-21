#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/smart-test-runner.mjs
 * 
 * Умный фильтр вывода тестов для экономии токенов ИИ.
 * Вырезает сотни строк успешных тестов и выдает ИИ только компактную суть:
 * при успехе — 2 строки, при ошибке — файл, строку и точный diff.
 * 
 * Использование:
 *   node scripts/smart-test-runner.mjs node --test tests/unit/my.test.mjs
 *   node scripts/smart-test-runner.mjs cargo test -p chord-client
 */

import { spawn } from "node:child_process";

const args = process.argv.slice(2);
if (args.length === 0) {
  console.error("Usage: node scripts/smart-test-runner.mjs <command> [args...]");
  process.exit(1);
}

const cmd = args[0];
const cmdArgs = args.slice(1);

const child = spawn(cmd, cmdArgs, { stdio: ["inherit", "pipe", "pipe"] });

let stdout = "";
let stderr = "";

child.stdout.on("data", (d) => { stdout += d.toString(); });
child.stderr.on("data", (d) => { stderr += d.toString(); });

child.on("close", (code) => {
  if (code === 0) {
    // Успех: извлекаем только итоговую строчку статистики
    const passMatch = stdout.match(/ℹ pass \d+|test result: ok/i);
    const durationMatch = stdout.match(/ℹ duration_ms [\d.]+/i);
    console.log("=========================================");
    console.log(`✔ SUCCESS (Exit Code 0)`);
    if (passMatch) console.log(`  ${passMatch[0]}`);
    if (durationMatch) console.log(`  ${durationMatch[0]}`);
    console.log("=========================================");
    process.exit(0);
  } else {
    // Ошибка: извлекаем только блок failing tests и стек
    console.error("=========================================");
    console.error(`✖ FAILED (Exit Code ${code})`);
    
    const lines = (stdout + "\n" + stderr).split("\n");
    let capturing = false;
    const errorSnippet = [];

    for (const line of lines) {
      if (line.includes("failing tests:") || line.includes("failures:") || line.includes("ERR_ASSERTION")) {
        capturing = true;
      }
      if (capturing) {
        errorSnippet.push(line);
        if (errorSnippet.length > 30) {
          errorSnippet.push("... [вывод обрезан скриптом smart-test-runner для экономии токенов]");
          break;
        }
      }
    }

    if (errorSnippet.length > 0) {
      console.error(errorSnippet.join("\n"));
    } else {
      // Фолбек: последние 25 строк
      console.error(lines.slice(-25).join("\n"));
    }
    console.error("=========================================");
    process.exit(code || 1);
  }
});
