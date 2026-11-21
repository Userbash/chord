#!/usr/bin/env node
/**
 * scripts/generate-cleanroom-tokens.mjs
 * 
 * Автоматический скрипт генерации cleanroom CSS-токенов из эталонного JSON Discord.
 * Полностью исключает участие ИИ при обновлении токенов темы, цветов и размеров.
 * 
 * Вход: /home/sanya/re-deep/ui/tokens.json (4795 переменных темы)
 * Выход: rust/gui-spike/ui/tokens-cleanroom.css
 */

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const SOURCE_PATH = "/home/sanya/re-deep/ui/tokens.json";
const TARGET_PATH = join(process.cwd(), "rust/gui-spike/ui/tokens-cleanroom.css");

console.log(`[Token-Gen] Чтение источника: ${SOURCE_PATH}`);
if (!existsSync(SOURCE_PATH)) {
  console.error(`[Token-Gen] Ошибка: Источник ${SOURCE_PATH} не найден.`);
  process.exit(1);
}

const raw = readFileSync(SOURCE_PATH, "utf8");
const tokens = JSON.parse(raw);

const lines = [
  "/* Сгенерировано автоматически скриптом scripts/generate-cleanroom-tokens.mjs */",
  "/* Источник: cleanroom reference /home/sanya/re-deep/ui/tokens.json */",
  "/* Без участия LLM / Zero-token generation */",
  "",
  ":root {"
];

let count = 0;
for (const [key, value] of Object.entries(tokens)) {
  if (typeof value !== "string" && typeof value !== "number") continue;
  
  // Нормализация имени CSS-переменной
  const varName = key.startsWith("--") ? key : `--${key}`;
  
  // Защита от опасных инъекций
  const safeVal = String(value).replace(/[;{}]/g, "").trim();
  if (safeVal) {
    lines.push(`  ${varName}: ${safeVal};`);
    count++;
  }
}

lines.push("}");
lines.push("");

writeFileSync(TARGET_PATH, lines.join("\n"), "utf8");
console.log(`[Token-Gen] Успешно сгенерировано ${count} CSS-переменных в ${TARGET_PATH}`);
