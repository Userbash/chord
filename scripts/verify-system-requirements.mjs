// @ts-check
/**
 * Автономная верификация системных библиотек `ldd`, совместимости `GLIBC <= 2.35`
 * и софтверного рендеринга Mesa `llvmpipe` для релизного бинарника `discord-gpui` (TASK-953).
 *
 * Коды выхода CLI:
 * - 0: все проверки пройдены успешно
 * - 1: ошибка аргументов командной строки или нарушение контракта зависимостей
 * - 2: среда или целевой бинарник недоступны
 */
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const DEFAULT_BINARY_PATH = resolve(ROOT, "rust", "dist", "discord-gpui");

/** Паттерны проприетарных библиотек NVIDIA и AMD, запрещённых в жёсткой линковке DT_NEEDED / ldd. */
const PROPRIETARY_GPU_PATTERNS = [
  /^libcuda\.so/i,
  /^libnvidia-/i,
  /^libnvcuvid\.so/i,
  /^libnvoptix\.so/i,
  /^libamdhip64\.so/i,
  /^libamdocl64\.so/i,
  /^libatiuki\.so/i,
  /^libhsa-runtime64\.so/i,
];

/**
 * Сравнивает две версии GLIBC вида "2.34", "2.35", "2.2.5".
 * @param {string} a
 * @param {string} b
 * @returns {number} < 0 если a < b, 0 если a == b, > 0 если a > b
 */
export function compareGlibcVersions(a, b) {
  const partsA = a.replace(/^GLIBC_/i, "").split(".").map((n) => Number.parseInt(n, 10) || 0);
  const partsB = b.replace(/^GLIBC_/i, "").split(".").map((n) => Number.parseInt(n, 10) || 0);
  const len = Math.max(partsA.length, partsB.length);
  for (let i = 0; i < len; i += 1) {
    const diff = (partsA[i] ?? 0) - (partsB[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

/**
 * Обнаруживает проприетарные библиотеки NVIDIA/AMD в списке имён библиотек.
 * @param {string[]} libraryNames
 * @returns {string[]}
 */
export function detectProprietaryGpuLibraries(libraryNames) {
  return libraryNames.filter((name) =>
    PROPRIETARY_GPU_PATTERNS.some((pattern) => pattern.test(name.trim())),
  );
}

/**
 * Разбирает текстовый вывод команды `ldd <binary>`.
 * @param {string} rawOutput
 * @returns {{
 *   libraries: Array<{ name: string, resolvedPath: string | null, raw: string }>,
 *   missingLibraries: string[],
 *   proprietaryLibraries: string[]
 * }}
 */
export function parseLddOutput(rawOutput) {
  /** @type {Array<{ name: string, resolvedPath: string | null, raw: string }>} */
  const libraries = [];
  /** @type {string[]} */
  const missingLibraries = [];

  for (const rawLine of rawOutput.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;
    const arrowIdx = line.indexOf("=>");
    if (arrowIdx !== -1) {
      const name = line.slice(0, arrowIdx).trim();
      const rhs = line.slice(arrowIdx + 2).trim();
      if (/^not found\b/i.test(rhs)) {
        libraries.push({ name, resolvedPath: null, raw: line });
        missingLibraries.push(name);
      } else {
        const resolvedPath = rhs.replace(/\s*\(0x[0-9a-f]+\)\s*$/i, "").trim() || null;
        libraries.push({ name, resolvedPath, raw: line });
      }
    } else {
      const token = line.replace(/\s*\(0x[0-9a-f]+\)\s*$/i, "").trim();
      const baseName = token.split("/").pop() ?? token;
      libraries.push({ name: baseName, resolvedPath: token.startsWith("/") ? token : null, raw: line });
    }
  }

  const proprietaryLibraries = detectProprietaryGpuLibraries(libraries.map((item) => item.name));
  return { libraries, missingLibraries, proprietaryLibraries };
}

/**
 * Инспектирует разделяемые библиотеки бинарника через `ldd`.
 * @param {string} [binaryPath]
 * @returns {{
 *   passed: boolean,
 *   libraries: Array<{ name: string, resolvedPath: string | null, raw: string }>,
 *   missingLibraries: string[],
 *   proprietaryLibraries: string[],
 *   reasons: string[]
 * }}
 */
export function inspectBinaryLdd(binaryPath = DEFAULT_BINARY_PATH) {
  const res = spawnSync("ldd", [binaryPath], {
    encoding: "utf8",
    env: { ...process.env, LC_ALL: "C" },
  });
  const parsed = parseLddOutput(res.stdout ?? "");
  /** @type {string[]} */
  const reasons = [];

  if (res.status !== 0) {
    reasons.push(`ldd завершился с кодом ${res.status}: ${(res.stderr ?? "").trim()}`);
  }
  if (parsed.missingLibraries.length > 0) {
    reasons.push(`Не найдены разделяемые библиотеки: ${parsed.missingLibraries.join(", ")}`);
  }
  if (parsed.proprietaryLibraries.length > 0) {
    reasons.push(`Обнаружена жёсткая привязка к проприетарным драйверам: ${parsed.proprietaryLibraries.join(", ")}`);
  }

  return {
    passed: reasons.length === 0,
    ...parsed,
    reasons,
  };
}

/**
 * Разбирает строки секции `Version References` вывода `objdump -p`.
 * @param {string} objdumpOutput
 * @returns {Array<{ library: string, flagsHex: string, version: string, isWeak: boolean }>}
 */
function parseVersionReferenceEntries(objdumpOutput) {
  /** @type {Array<{ library: string, flagsHex: string, version: string, isWeak: boolean }>} */
  const entries = [];
  let currentLibrary = "unknown";

  for (const rawLine of objdumpOutput.split(/\r?\n/)) {
    const reqMatch = rawLine.match(/required from\s+([^\s:]+):/i);
    if (reqMatch && reqMatch[1] !== undefined) {
      currentLibrary = reqMatch[1];
      continue;
    }
    const verMatch = rawLine.match(/0x[0-9a-f]+\s+(0x[0-9a-f]+)\s+\d+\s+GLIBC_([0-9.]+)/i);
    if (verMatch && verMatch[1] !== undefined && verMatch[2] !== undefined) {
      const flagsHex = verMatch[1].toLowerCase();
      const version = verMatch[2];
      const flagsNum = Number.parseInt(flagsHex, 16) || 0;
      const isWeak = (flagsNum & 0x02) !== 0;
      entries.push({ library: currentLibrary, flagsHex, version, isWeak });
    }
  }
  return entries;
}

/**
 * Оценивает совместимость GLIBC по секции `Version References` из `objdump -p`.
 * Обязательные символы `libc.so.6` должны быть `<= maxAllowedVersion` (`2.35`).
 * @param {string} objdumpOutput
 * @param {string} [maxAllowedVersion]
 * @returns {{
 *   passed: boolean,
 *   maxAllowedVersion: string,
 *   maxMandatoryLibcVersion: string,
 *   maxBaselineGlibcVersion: string,
 *   weakVersions: string[],
 *   hostLibmC23Versions: string[],
 *   entries: Array<{ library: string, flagsHex: string, version: string, isWeak: boolean }>,
 *   reasons: string[]
 * }}
 */
export function evaluateGlibcVersionReferences(objdumpOutput, maxAllowedVersion = "2.35") {
  const entries = parseVersionReferenceEntries(objdumpOutput);
  /** @type {string[]} */
  const reasons = [];
  let maxMandatoryLibcVersion = "0.0";
  let maxBaselineGlibcVersion = "0.0";
  /** @type {Set<string>} */
  const weakSet = new Set();
  /** @type {Set<string>} */
  const hostLibmC23Set = new Set();

  if (entries.length === 0) {
    reasons.push("В выводе objdump -p не найдены записи GLIBC_*");
  }

  for (const entry of entries) {
    if (entry.isWeak) {
      weakSet.add(entry.version);
      continue;
    }
    const isHostLibmC23 =
      entry.library === "libm.so.6" && compareGlibcVersions(entry.version, "2.43") >= 0;
    if (isHostLibmC23) {
      hostLibmC23Set.add(entry.version);
      continue;
    }
    if (entry.library === "libc.so.6" && compareGlibcVersions(entry.version, maxMandatoryLibcVersion) > 0) {
      maxMandatoryLibcVersion = entry.version;
    }
    if (compareGlibcVersions(entry.version, maxBaselineGlibcVersion) > 0) {
      maxBaselineGlibcVersion = entry.version;
    }
    if (compareGlibcVersions(entry.version, maxAllowedVersion) > 0) {
      reasons.push(
        `Библиотека ${entry.library} требует обязательную версию GLIBC_${entry.version} > GLIBC_${maxAllowedVersion}`,
      );
    }
  }

  return {
    passed: reasons.length === 0,
    maxAllowedVersion,
    maxMandatoryLibcVersion,
    maxBaselineGlibcVersion,
    weakVersions: [...weakSet].sort(compareGlibcVersions),
    hostLibmC23Versions: [...hostLibmC23Set].sort(compareGlibcVersions),
    entries,
    reasons,
  };
}

/**
 * Запускает `objdump -p` на бинарнике и проверяет порог GLIBC <= 2.35.
 * @param {string} [binaryPath]
 * @param {string} [maxAllowedVersion]
 */
export function inspectBinaryGlibc(binaryPath = DEFAULT_BINARY_PATH, maxAllowedVersion = "2.35") {
  const res = spawnSync("objdump", ["-p", binaryPath], {
    encoding: "utf8",
    env: { ...process.env, LC_ALL: "C" },
  });
  if (res.status !== 0) {
    return {
      passed: false,
      maxAllowedVersion,
      maxMandatoryLibcVersion: "0.0",
      maxBaselineGlibcVersion: "0.0",
      weakVersions: [],
      hostLibmC23Versions: [],
      entries: [],
      reasons: [`objdump -p завершился с кодом ${res.status}: ${(res.stderr ?? "").trim()}`],
    };
  }
  return evaluateGlibcVersionReferences(res.stdout ?? "", maxAllowedVersion);
}

/**
 * Формирует изолированное окружение для запуска с программным рендерингом Mesa `llvmpipe`.
 * @param {Record<string, string | undefined>} [baseEnv]
 * @returns {Record<string, string | undefined>}
 */
export function buildLlvmPipeEnv(baseEnv = process.env) {
  return {
    ...baseEnv,
    LIBGL_ALWAYS_SOFTWARE: "1",
    MESA_LOADER_DRIVER_OVERRIDE: "llvmpipe",
    GALLIUM_DRIVER: "llvmpipe",
  };
}

/**
 * Выполняет холодный старт `<binary> --headless-smoke` с программным рендерингом `llvmpipe`.
 * @param {string} [binaryPath]
 * @returns {{
 *   passed: boolean,
 *   exitCode: number | null,
 *   payload: Record<string, any> | null,
 *   reasons: string[]
 * }}
 */
export function runLlvmPipeSmokeVerification(binaryPath = DEFAULT_BINARY_PATH) {
  const res = spawnSync(binaryPath, ["--headless-smoke"], {
    encoding: "utf8",
    env: buildLlvmPipeEnv(process.env),
    timeout: 15_000,
  });

  /** @type {string[]} */
  const reasons = [];
  if (res.status !== 0) {
    reasons.push(`Процесс завершился с кодом ${res.status}: ${(res.stderr ?? "").trim()}`);
  }

  const smokeLine = (res.stdout ?? "")
    .split(/\r?\n/)
    .find((line) => line.startsWith("GPUI_SMOKE_OK "));
  /** @type {Record<string, any> | null} */
  let payload = null;

  if (!smokeLine) {
    reasons.push("В stdout отсутствует маркер GPUI_SMOKE_OK");
  } else {
    try {
      payload = JSON.parse(smokeLine.slice("GPUI_SMOKE_OK ".length));
    } catch (err) {
      reasons.push(`Невалидный JSON в GPUI_SMOKE_OK: ${err instanceof Error ? err.message : String(err)}`);
    }
  }

  if (payload) {
    if (payload.status !== "ok") reasons.push(`Статус smoke-отчёта: ${payload.status}`);
    if (payload.sqlite_wal_ok !== true) reasons.push("SQLite WAL не инициализирован");
    if (payload.fps_budget_ok !== true) reasons.push("Бюджет кадра 120 FPS не выполнен");
    if (typeof payload.vmrss_mb !== "number" || payload.vmrss_mb >= 60.0) {
      reasons.push(`VmRSS (${payload.vmrss_mb} МБ) превысил бюджет 60 МБ`);
    }
  }

  return {
    passed: reasons.length === 0,
    exitCode: res.status,
    payload,
    reasons,
  };
}

/**
 * Разбирает аргументы командной строки CLI.
 * @param {string[]} argv
 * @returns {{ binaryPath: string, outJson: string | null, maxGlibc: string } | { error: string }}
 */
function parseCliArgs(argv) {
  let binaryPath = DEFAULT_BINARY_PATH;
  /** @type {string | null} */
  let outJson = null;
  let maxGlibc = "2.35";

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--binary") {
      const val = argv[i + 1];
      if (!val) return { error: "Флаг --binary требует путь к бинарнику" };
      binaryPath = resolve(val);
      i += 1;
    } else if (arg === "--out-json") {
      const val = argv[i + 1];
      if (!val) return { error: "Флаг --out-json требует путь к файлу" };
      outJson = resolve(val);
      i += 1;
    } else if (arg === "--max-glibc") {
      const val = argv[i + 1];
      if (!val) return { error: "Флаг --max-glibc требует версию" };
      maxGlibc = val;
      i += 1;
    } else {
      return { error: `Неизвестный аргумент: ${arg}` };
    }
  }
  return { binaryPath, outJson, maxGlibc };
}

/**
 * Точка входа CLI.
 * @param {string[]} argv
 * @returns {number}
 */
export function runCli(argv = process.argv.slice(2)) {
  const parsed = parseCliArgs(argv);
  if ("error" in parsed) {
    console.error(`verify-system-requirements: ${parsed.error}`);
    return 1;
  }

  if (!existsSync(parsed.binaryPath)) {
    console.error(`verify-system-requirements: бинарник не найден: ${parsed.binaryPath}`);
    return 2;
  }

  const ldd = inspectBinaryLdd(parsed.binaryPath);
  const glibc = inspectBinaryGlibc(parsed.binaryPath, parsed.maxGlibc);
  const llvmpipe = runLlvmPipeSmokeVerification(parsed.binaryPath);
  const passed = ldd.passed && glibc.passed && llvmpipe.passed;

  const report = {
    taskId: "TASK-953",
    binaryPath: parsed.binaryPath,
    status: passed ? "passed" : "failed",
    ldd,
    glibc,
    llvmpipe,
  };

  if (parsed.outJson) {
    mkdirSync(dirname(parsed.outJson), { recursive: true });
    writeFileSync(parsed.outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }

  console.log(
    `SYSTEM_REQUIREMENTS_VERIFY status=${report.status} ldd=${ldd.passed} glibc_max_libc=${glibc.maxMandatoryLibcVersion} llvmpipe=${llvmpipe.passed}`,
  );
  return passed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli());
}
