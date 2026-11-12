// @ts-check
// Мета-тест (красный → зелёный): TASK-632 — матрица паритета Wave 42 (`MERGE-READINESS-V0.5.0.md`)
// и релизные ноты `RELEASE-NOTES-V0.5.0.md`.
// Проверяет: наличие и полноту обоих документов, все 8 обязательных доменов
// (Voice, REST, Gateway, UI, Platform, RPC, Media, Auth), уникальность ID строк,
// ссылки на эталонный RE-дамп `/home/sanya/re-deep/` (`rest-d02.md`, `frontend-d03.md`,
// `voice-deep.md`, `gateway-schema.md`, `decoded-index.json`), существование реальных
// файлов реализации/тестов в репозитории, покрытие ключевых фич Спринтов 1–13
// (`TASK-610..TASK-630`) и арифметическую сходимость сводной таблицы §9 с фактическими строками.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const MATRIX_PATH = "waves/RW-40/PARITY/MERGE-READINESS-V0.5.0.md";
const RELEASE_NOTES_PATH = "RELEASE-NOTES-V0.5.0.md";

/** Обязательные домены матрицы паритета v0.5.0 и их префиксы ID. */
const REQUIRED_DOMAINS = [
  { heading: "## 1. Voice", code: "VOICE", minRows: 6 },
  { heading: "## 2. REST", code: "REST", minRows: 7 },
  { heading: "## 3. Gateway", code: "GW", minRows: 6 },
  { heading: "## 4. UI", code: "UI", minRows: 8 },
  { heading: "## 5. Platform", code: "PLAT", minRows: 6 },
  { heading: "## 6. RPC", code: "RPC", minRows: 5 },
  { heading: "## 7. Media", code: "MEDIA", minRows: 5 },
  { heading: "## 8. Auth", code: "AUTH", minRows: 5 },
];

/** Обязательные артефакты эталонного RE-оракула `/home/sanya/re-deep/`. */
const REQUIRED_ORACLES = [
  "rest-d02.md",
  "frontend-d03.md",
  "voice-deep.md",
  "gateway-schema.md",
  "decoded-index.json",
];

/** Обязательные фичи и задачи Спринтов 1–13 в матрице и релизных нотах. */
const REQUIRED_FEATURES = [
  "TASK-610",
  "TASK-611",
  "TASK-612",
  "TASK-613",
  "TASK-614",
  "TASK-615",
  "TASK-616",
  "TASK-617",
  "TASK-618",
  "TASK-619",
  "TASK-620",
  "TASK-621",
  "TASK-622",
  "TASK-623",
  "TASK-624",
  "TASK-625",
  "TASK-626",
  "TASK-627",
  "TASK-628",
  "TASK-629",
  "TASK-630",
  "DAVE MLS v2",
  "discord://",
  "disclip://",
  "Nitro Gifts",
  "QR",
  "PipeWire",
  "LatestFrameSlot",
  "Xvfb",
  "Keybinds",
  "Soundboard",
];

/** Допустимые статусы паритета в строках матрицы. */
const ALLOWED_STATUSES = new Set([
  "Implemented (Verified)",
  "Implemented (Offline Verified)",
  "Partial (In Progress)",
]);

/**
 * @typedef {object} ParityRow
 * @property {string} id
 * @property {string} domainCode
 * @property {string} feature
 * @property {string} oracleRef
 * @property {string} implementationRef
 * @property {string} testRef
 * @property {string} status
 */

/**
 * Разбирает строки таблиц паритета вида `| PM-<CODE>-<NN> | ... |`.
 * @param {string} markdown содержимое `MERGE-READINESS-V0.5.0.md`
 * @returns {ParityRow[]}
 */
export function parseParityRows(markdown) {
  /** @type {ParityRow[]} */
  const rows = [];
  for (const rawLine of markdown.split("\n")) {
    const line = rawLine.trim();
    const match = /^\|\s*(PM-([A-Z]+)-\d{2})\s*\|/.exec(line);
    if (!match) continue;
    const cells = line
      .replace(/^\|/, "")
      .replace(/\|$/, "")
      .split("|")
      .map((cell) => cell.trim());
    if (cells.length < 6) continue;
    rows.push({
      id: cells[0] ?? "",
      domainCode: match[2] ?? "",
      feature: cells[1] ?? "",
      oracleRef: cells[2] ?? "",
      implementationRef: cells[3] ?? "",
      testRef: cells[4] ?? "",
      status: cells[5] ?? "",
    });
  }
  return rows;
}

/**
 * Сверяет сводную таблицу `## 9. Сводная статистика паритета` с фактическими строками доменов.
 * @param {string} markdown содержимое `MERGE-READINESS-V0.5.0.md`
 * @param {ParityRow[]} rows разобранные строки паритета
 * @returns {string[]} список расхождений (пустой при полном совпадении)
 */
export function verifyDomainSummary(markdown, rows) {
  /** @type {string[]} */
  const mismatches = [];
  const summaryStart = markdown.indexOf("## 9. Сводная статистика паритета");
  if (summaryStart < 0) {
    mismatches.push("отсутствует раздел «## 9. Сводная статистика паритета»");
    return mismatches;
  }
  const summarySection = markdown.slice(summaryStart);

  /** @type {Map<string, { total: number, implemented: number, partial: number }>} */
  const actualByCode = new Map();
  for (const domain of REQUIRED_DOMAINS) {
    actualByCode.set(domain.code, { total: 0, implemented: 0, partial: 0 });
  }
  for (const row of rows) {
    const entry = actualByCode.get(row.domainCode);
    if (!entry) continue;
    entry.total += 1;
    if (row.status.startsWith("Implemented")) entry.implemented += 1;
    else if (row.status.startsWith("Partial")) entry.partial += 1;
  }

  /** @type {Map<string, { total: number, implemented: number, partial: number }>} */
  const declaredByCode = new Map();
  let declaredGrandTotal = -1;
  let declaredGrandImplemented = -1;
  let declaredGrandPartial = -1;

  for (const rawLine of summarySection.split("\n")) {
    const line = rawLine.trim();
    const domainMatch =
      /^\|\s*`?(VOICE|REST|GW|UI|PLAT|RPC|MEDIA|AUTH)`?\s*\|\s*[^|]+\|\s*(\d+)\s*\|\s*(\d+)\s*\|\s*(\d+)\s*\|/.exec(
        line,
      );
    if (domainMatch) {
      const code = domainMatch[1] ?? "";
      declaredByCode.set(code, {
        total: Number(domainMatch[2]),
        implemented: Number(domainMatch[3]),
        partial: Number(domainMatch[4]),
      });
      continue;
    }
    const totalMatch =
      /^\|\s*\*\*Итого\*\*\s*\|\s*[^|]+\|\s*\*\*(\d+)\*\*\s*\|\s*\*\*(\d+)\*\*\s*\|\s*\*\*(\d+)\*\*\s*\|/.exec(
        line,
      );
    if (totalMatch) {
      declaredGrandTotal = Number(totalMatch[1]);
      declaredGrandImplemented = Number(totalMatch[2]);
      declaredGrandPartial = Number(totalMatch[3]);
    }
  }

  let sumTotal = 0;
  let sumImplemented = 0;
  let sumPartial = 0;

  for (const domain of REQUIRED_DOMAINS) {
    const actual = actualByCode.get(domain.code) ?? { total: 0, implemented: 0, partial: 0 };
    const declared = declaredByCode.get(domain.code);
    sumTotal += actual.total;
    sumImplemented += actual.implemented;
    sumPartial += actual.partial;
    if (!declared) {
      mismatches.push(`в сводной таблице §9 нет строки домена ${domain.code}`);
      continue;
    }
    if (
      declared.total !== actual.total ||
      declared.implemented !== actual.implemented ||
      declared.partial !== actual.partial
    ) {
      mismatches.push(
        `${domain.code}: заявлено ${declared.total}/${declared.implemented}/${declared.partial}, фактически ${actual.total}/${actual.implemented}/${actual.partial}`,
      );
    }
  }

  if (
    declaredGrandTotal !== sumTotal ||
    declaredGrandImplemented !== sumImplemented ||
    declaredGrandPartial !== sumPartial
  ) {
    mismatches.push(
      `Итого §9: заявлено ${declaredGrandTotal}/${declaredGrandImplemented}/${declaredGrandPartial}, фактически ${sumTotal}/${sumImplemented}/${sumPartial}`,
    );
  }

  return mismatches;
}

describe("TASK-632: матрица паритета v0.5.0 и релизные ноты", () => {
  it("документы MERGE-READINESS-V0.5.0.md и RELEASE-NOTES-V0.5.0.md существуют и непусты", async () => {
    for (const relPath of [MATRIX_PATH, RELEASE_NOTES_PATH]) {
      const info = await stat(join(root, relPath)).catch(() => null);
      assert.ok(info?.isFile(), `${relPath}: файл отсутствует`);
      assert.ok((info?.size ?? 0) >= 4000, `${relPath}: размер ${info?.size ?? 0} Б < 4000 Б`);
    }
  });

  it("матрица содержит все 8 обязательных доменов, сводку §9 и вердикт готовности §10", async () => {
    const text = await readFile(join(root, MATRIX_PATH), "utf8");
    for (const domain of REQUIRED_DOMAINS) {
      assert.ok(
        text.includes(domain.heading),
        `${MATRIX_PATH}: отсутствует обязательный раздел домена «${domain.heading}»`,
      );
    }
    assert.ok(
      text.includes("## 9. Сводная статистика паритета"),
      `${MATRIX_PATH}: отсутствует раздел «## 9. Сводная статистика паритета»`,
    );
    assert.ok(
      text.includes("## 10. Готовность к релизу v0.5.0 (Merge Readiness Verdict)"),
      `${MATRIX_PATH}: отсутствует раздел «## 10. Готовность к релизу v0.5.0 (Merge Readiness Verdict)»`,
    );
  });

  it("строки матрицы имеют уникальные ID, валидные статусы, оракулы /home/sanya/re-deep/ и живые пути файлов", async () => {
    const text = await readFile(join(root, MATRIX_PATH), "utf8");
    const rows = parseParityRows(text);
    assert.ok(rows.length >= 48, `ожидалось ≥ 48 строк паритета, найдено ${rows.length}`);

    const ids = rows.map((r) => r.id);
    const duplicates = [...new Set(ids.filter((id, idx) => ids.indexOf(id) !== idx))];
    assert.equal(duplicates.length, 0, `дублирующиеся ID строк матрицы: ${duplicates.join(", ")}`);

    for (const domain of REQUIRED_DOMAINS) {
      const domainRows = rows.filter((r) => r.domainCode === domain.code);
      assert.ok(
        domainRows.length >= domain.minRows,
        `домен ${domain.code}: найдено ${domainRows.length} строк, ожидалось ≥ ${domain.minRows}`,
      );
    }

    /** @type {string[]} */
    const invalidRows = [];
    /** @type {string[]} */
    const missingPaths = [];

    for (const row of rows) {
      if (
        !row.feature ||
        !row.oracleRef ||
        !row.implementationRef ||
        !row.testRef ||
        !ALLOWED_STATUSES.has(row.status)
      ) {
        invalidRows.push(`${row.id}: некорректные колонки или статус «${row.status}»`);
      }
      if (
        !row.oracleRef.includes("/home/sanya/re-deep/") &&
        !row.oracleRef.includes("docs/re/") &&
        !row.oracleRef.includes("clean-room-oracle/")
      ) {
        invalidRows.push(`${row.id}: колонка эталона не ссылается на clean-room oracle`);
      }

      const repoPaths = [
        ...row.implementationRef.matchAll(/`((?:rust|src|scripts|tests)\/[^`]+)`/g),
        ...row.testRef.matchAll(/`((?:rust|src|scripts|tests)\/[^`]+)`/g),
      ].map((m) => m[1] ?? "");

      if (repoPaths.length < 2) {
        invalidRows.push(`${row.id}: ожидались пути к файлу реализации и файлу теста`);
      }

      for (const repoPath of repoPaths) {
        const exists = await stat(join(root, repoPath)).then(
          () => true,
          () => false,
        );
        if (!exists) {
          missingPaths.push(`${row.id}: путь не существует в репозитории: ${repoPath}`);
        }
      }
    }

    assert.equal(invalidRows.length, 0, `ошибки строк матрицы:\n${invalidRows.join("\n")}`);
    assert.equal(missingPaths.length, 0, `несуществующие файлы в матрице:\n${missingPaths.join("\n")}`);
  });

  it("сводная таблица §9 точно сходится с количеством строк и статусов по всем 8 доменам", async () => {
    const text = await readFile(join(root, MATRIX_PATH), "utf8");
    const rows = parseParityRows(text);
    const mismatches = verifyDomainSummary(text, rows);
    assert.equal(
      mismatches.length,
      0,
      `расхождения сводной таблицы §9 и строк матрицы:\n${mismatches.join("\n")}`,
    );
  });

  it("матрица и RELEASE-NOTES-V0.5.0.md покрывают все ключевые оракулы /home/sanya/re-deep/ и фичи Спринтов 1–13", async () => {
    const matrix = await readFile(join(root, MATRIX_PATH), "utf8");
    const notes = await readFile(join(root, RELEASE_NOTES_PATH), "utf8");

    for (const oracle of REQUIRED_ORACLES) {
      assert.ok(
        matrix.includes(oracle),
        `${MATRIX_PATH}: не упомянут обязательный артефакт RE-оракула ${oracle}`,
      );
      assert.ok(
        notes.includes(oracle),
        `${RELEASE_NOTES_PATH}: не упомянут обязательный артефакт RE-оракула ${oracle}`,
      );
    }

    for (const feature of REQUIRED_FEATURES) {
      assert.ok(
        matrix.includes(feature),
        `${MATRIX_PATH}: не отражена обязательная фича/задача «${feature}»`,
      );
      assert.ok(
        notes.includes(feature),
        `${RELEASE_NOTES_PATH}: не отражена обязательная фича/задача «${feature}»`,
      );
    }

    for (const [ruHeading, enHeading] of [
      ["## 1. Обзор релиза v0.5.0", "## 1. Release Overview v0.5.0"],
      ["## 2. Новые возможности и функции (Спринты 1–13)", "## 2. New Features and Capabilities (Sprints 1–13)"],
      ["## 3. Архитектура клиента и подсистем", "## 3. Client and Subsystem Architecture"],
      ["## 4. Сверка с эталонным RE-оракулом", "## 4. Parity Verification Against Clean-Room RE Oracle"],
      ["## 5. Контур качества, мутации и верификация", "## 5. Quality Contour, Mutations, and Verification"],
      ["## 6. Системные требования и окружение", "## 6. System Requirements and Environment"],
    ]) {
      assert.ok(
        (ruHeading && notes.includes(ruHeading)) || (enHeading && notes.includes(enHeading)),
        `${RELEASE_NOTES_PATH}: отсутствует обязательный раздел «${enHeading}» / «${ruHeading}»`,
      );
    }
    assert.ok(notes.includes("```mermaid"), `${RELEASE_NOTES_PATH}: отсутствует Mermaid-схема архитектуры`);
  });
});
