#!/usr/bin/env node
// @ts-nocheck
/**
 * scripts/hooks/post-tool-condenser.mjs
 * 
 * PostToolUse Hook: Автоматический сжиматель вывода инструментов.
 * 
 * Проблема: Вывод `npm test`, `cargo test`, `git log` возвращает сотни строк,
 * забивая контекст LLM мусором. После 8-10 таких итераций контекст переполняется,
 * внимание модели деградирует и начинаются галлюцинации.
 * 
 * Решение:
 * 1. При успешном выполнении длинных команд (>30 строк) сжимает вывод до 3-5 ключевых строк.
 * 2. При ошибке сохраняет файл, номер строки, assertion diff и отрезает нерелевантный шум.
 */

export function condenseToolOutput(toolName, outputText) {
  if (typeof outputText !== "string" || outputText.length < 1500) {
    return outputText;
  }

  const lines = outputText.split("\n");
  if (lines.length <= 35) {
    return outputText;
  }

  // 1. Поиск ошибок сборки и тестов
  const hasFailures =
    outputText.includes("failing tests:") ||
    outputText.includes("ERR_ASSERTION") ||
    outputText.includes("error[E") ||
    outputText.includes("FAILED");

  if (hasFailures) {
    // Сохраняем первые 10 строк контекста и блок самой ошибки
    const result = [];
    result.push(...lines.slice(0, 5));
    result.push("... [пропущен успешный вывод] ...");

    let inErrorBlock = false;
    let errorCount = 0;
    for (const line of lines) {
      if (
        line.includes("failing tests:") ||
        line.includes("ERR_ASSERTION") ||
        line.includes("error[E") ||
        line.includes("✖")
      ) {
        inErrorBlock = true;
      }
      if (inErrorBlock) {
        result.push(line);
        errorCount++;
        if (errorCount > 35) {
          result.push("... [остальные строки обрезаны post-tool-condenser для экономии токенов]");
          break;
        }
      }
    }
    return result.join("\n");
  }

  // 2. Успешный длинный вывод: берем первые 5 и последние 10 строк
  const condensed = [
    ...lines.slice(0, 5),
    `... [скрыто ${lines.length - 15} строк успешного вывода для защиты от галлюцинаций] ...`,
    ...lines.slice(-10),
  ];
  return condensed.join("\n");
}

// Запуск из CLI для тестов
if (process.argv[1]?.endsWith("post-tool-condenser.mjs")) {
  const dummy = new Array(100).fill("✔ passing test line").join("\n") + "\n✖ failing test at file.js:42: Expected 1 to be 2";
  console.log("Original lines:", dummy.split("\n").length);
  const res = condenseToolOutput("run_command", dummy);
  console.log("Condensed output:\n" + res);
}
