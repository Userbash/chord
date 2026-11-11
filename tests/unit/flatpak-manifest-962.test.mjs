// @ts-check
/**
 * Контрактные и модульные тесты манифеста Flatpak и песочницы Wayland/PipeWire (TASK-962):
 * - SUB-962-1: Валидация YAML-схемы `packaging/flatpak/com.discord_lab.DiscordGpui.yml`
 *   независимым оракулом (Python PyYAML), проверка `app-id`, `runtime: org.freedesktop.Platform`,
 *   `runtime-version: "23.08"`, `sdk: org.freedesktop.Sdk`, `command: discord-gpui`.
 * - SUB-962-2: Проверка обязательных прав песочницы (`--socket=wayland`, `--socket=fallback-x11`,
 *   `--filesystem=xdg-run/pipewire-0`, Secret Service portal `--talk-name=org.freedesktop.secrets`
 *   и `--talk-name=org.freedesktop.portal.Desktop`, `--device=dri`, `--share=network`, `--share=ipc`).
 * - SUB-962-3: Проверка строгой минимальности прав sandboxing (запрет `--filesystem=host`,
 *   `--filesystem=home`, прямого `--socket=x11`, `--device=all`, `--socket=session-bus`,
 *   `--socket=system-bus`, `--allow=devel`).
 * - SUB-962-4: Проверка модуля `discord-gpui`, команд установки и побайтовая сверка `sha256`
 *   всех источников с независимым оракулом `rust/dist/SHA256SUMS` и фактическими файлами.
 * - SUB-962-5: Негативные, граничные и защитные сценарии валидатора (9 измерений TEST-POLICY.md).
 */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const MANIFEST_DIR = join(ROOT, "packaging", "flatpak");
const MANIFEST_PATH = join(MANIFEST_DIR, "com.discord_lab.DiscordGpui.yml");
const DIST_DIR = join(ROOT, "rust", "dist");
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");

/** Обязательные аргументы песочницы Flatpak по контракту TASK-962. */
const REQUIRED_FINISH_ARGS = Object.freeze([
  "--socket=wayland",
  "--socket=fallback-x11",
  "--filesystem=xdg-run/pipewire-0",
  "--talk-name=org.freedesktop.secrets",
  "--talk-name=org.freedesktop.portal.Desktop",
  "--device=dri",
  "--share=network",
  "--share=ipc",
  "--socket=pulseaudio",
]);

/** Запрещённые избыточные права, нарушающие изоляцию песочницы Flatpak. */
const FORBIDDEN_FINISH_ARG_PATTERNS = Object.freeze([
  { pattern: /^--socket=x11$/, reason: "Прямой доступ --socket=x11 запрещён; используйте --socket=fallback-x11" },
  { pattern: /^--filesystem=(host|home|root|\/|xdg-config|xdg-data|~)/, reason: "Доступ к файловой системе хоста/домашней директории запрещён" },
  { pattern: /^--device=all$/, reason: "Полный доступ ко всем устройствам (--device=all) запрещён; используйте --device=dri" },
  { pattern: /^--socket=(system-bus|session-bus|ssh-auth|pcsc|cups)$/, reason: "Прямой доступ к системной/сессионной шине D-Bus или SSH-агенту запрещён" },
  { pattern: /^--allow=(devel|multiarch|canbus)$/, reason: "Флаги отладки/devel в релизной песочнице запрещены" },
  { pattern: /^--env=.*(mfa\.[A-Za-z0-9_-]{10,}|Bot\s+[A-Za-z0-9._-]{20,})/, reason: "Немаскированный секретный токен в переменных окружения запрещён" },
]);

/**
 * Вычисляет SHA-256 дайджест файла.
 * @param {string} filePath
 * @returns {string}
 */
function sha256File(filePath) {
  const buf = readFileSync(filePath);
  return createHash("sha256").update(buf).digest("hex");
}

/**
 * Парсит файл формата SHA256SUMS в карту filename -> sha256.
 * @param {string} content
 * @returns {Map<string, string>}
 */
function parseSha256Sums(content) {
  /** @type {Map<string, string>} */
  const map = new Map();
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;
    const match = /^([a-f0-9]{64})\s+\*?(.+)$/.exec(line);
    const hash = match?.[1];
    const name = match?.[2];
    assert.ok(hash && name, `Некорректная строка в SHA256SUMS: "${line}"`);
    map.set(name.trim(), hash);
  }
  return map;
}

/**
 * Независимый оракул парсинга YAML через системный Python 3 (PyYAML).
 * @param {string} yamlPath
 * @returns {any}
 */
function parseYamlWithPythonOracle(yamlPath) {
  const pyScript = [
    "import json, sys, yaml",
    "with open(sys.argv[1], 'r', encoding='utf-8') as f:",
    "    data = yaml.safe_load(f)",
    "print(json.dumps(data))",
  ].join("\n");

  const res = spawnSync("python3", ["-c", pyScript, yamlPath], {
    encoding: "utf8",
  });
  assert.equal(
    res.status,
    0,
    `Ошибка парсинга YAML независимым оракулом PyYAML:\n${res.stderr}`,
  );
  return JSON.parse(res.stdout);
}

/**
 * Проверяет распарсенный манифест Flatpak на соответствие схеме, минимальности прав
 * и контрольным суммам артефактов.
 * @param {any} manifest
 * @param {Map<string, string>} [oracleSums]
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateFlatpakManifest(manifest, oracleSums) {
  /** @type {string[]} */
  const errors = [];

  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    return { valid: false, errors: ["Манифест должен быть непустым YAML-объектом"] };
  }

  const appId = manifest["app-id"] ?? manifest.id;
  if (appId !== "com.discord_lab.DiscordGpui") {
    errors.push(`Ожидался app-id "com.discord_lab.DiscordGpui", получено: "${String(appId)}"`);
  }

  if (manifest.runtime !== "org.freedesktop.Platform") {
    errors.push(`Ожидался runtime "org.freedesktop.Platform", получено: "${String(manifest.runtime)}"`);
  }

  if (String(manifest["runtime-version"]) !== "23.08") {
    errors.push(`Ожидалась runtime-version "23.08", получено: "${String(manifest["runtime-version"])}"`);
  }

  if (manifest.sdk !== "org.freedesktop.Sdk") {
    errors.push(`Ожидался sdk "org.freedesktop.Sdk", получено: "${String(manifest.sdk)}"`);
  }

  if (manifest.command !== "discord-gpui") {
    errors.push(`Ожидался command "discord-gpui", получено: "${String(manifest.command)}"`);
  }

  const finishArgs = manifest["finish-args"];
  if (!Array.isArray(finishArgs) || finishArgs.length === 0) {
    errors.push("Поле finish-args должно быть непустым массивом строк");
  } else {
    const seen = new Set();
    for (const arg of finishArgs) {
      if (typeof arg !== "string" || !arg.startsWith("--")) {
        errors.push(`Некорректный элемент finish-args: "${String(arg)}"`);
        continue;
      }
      if (seen.has(arg)) {
        errors.push(`Дублирующийся аргумент в finish-args: "${arg}"`);
      }
      seen.add(arg);

      for (const rule of FORBIDDEN_FINISH_ARG_PATTERNS) {
        if (rule.pattern.test(arg)) {
          errors.push(`Нарушение минимальности песочницы (${arg}): ${rule.reason}`);
        }
      }
    }

    for (const reqArg of REQUIRED_FINISH_ARGS) {
      if (!seen.has(reqArg)) {
        errors.push(`Отсутствует обязательное право песочницы: "${reqArg}"`);
      }
    }
  }

  const modules = manifest.modules;
  if (!Array.isArray(modules) || modules.length === 0) {
    errors.push("Поле modules должно быть непустым массивом модулей");
  } else {
    const mainMod = modules.find(
      (/** @type {any} */ m) => m && typeof m === "object" && m.name === "discord-gpui",
    );
    if (!mainMod) {
      errors.push("В секции modules должен присутствовать модуль с именем 'discord-gpui'");
    } else {
      if (mainMod.buildsystem !== "simple") {
        errors.push(`Модуль discord-gpui должен использовать buildsystem 'simple', получено: '${String(mainMod.buildsystem)}'`);
      }
      if (!Array.isArray(mainMod["build-commands"]) || mainMod["build-commands"].length === 0) {
        errors.push("Модуль discord-gpui должен содержать непустой список build-commands");
      } else {
        const installsBin = mainMod["build-commands"].some(
          (/** @type {any} */ cmd) =>
            typeof cmd === "string" &&
            cmd.includes("install") &&
            cmd.includes("/app/bin/discord-gpui"),
        );
        if (!installsBin) {
          errors.push("В build-commands должна присутствовать установка бинарника в /app/bin/discord-gpui");
        }
      }

      if (!Array.isArray(mainMod.sources) || mainMod.sources.length === 0) {
        errors.push("Модуль discord-gpui должен содержать непустой массив sources");
      } else {
        for (const src of mainMod.sources) {
          if (!src || typeof src !== "object") {
            errors.push("Элемент sources должен быть объектом");
            continue;
          }
          if (src.type !== "archive" && src.type !== "file") {
            errors.push(`Неподдерживаемый тип источника '${String(src.type)}' (ожидался 'archive' или 'file')`);
          }
          if (typeof src.path !== "string" || !src.path.trim()) {
            errors.push("Источник должен содержать непустой путь path");
          }
          if (typeof src.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(src.sha256)) {
            errors.push(`Источник '${String(src.path)}' должен содержать валидный 64-символьный sha256`);
          } else if (oracleSums && typeof src.path === "string") {
            const fileBase = basename(src.path);
            const expectedHash = oracleSums.get(fileBase);
            if (!expectedHash) {
              errors.push(`Файл источника '${fileBase}' отсутствует в оракуле rust/dist/SHA256SUMS`);
            } else if (src.sha256 !== expectedHash) {
              errors.push(
                `SHA-256 источника '${fileBase}' (${src.sha256}) не совпадает с оракулом SHA256SUMS (${expectedHash})`,
              );
            }
          }
        }
      }
    }
  }

  return { valid: errors.length === 0, errors };
}

describe("TASK-962: Манифест Flatpak com.discord_lab.DiscordGpui.yml и песочница Wayland/PipeWire", () => {
  it("SUB-962-1: манифест packaging/flatpak/com.discord_lab.DiscordGpui.yml существует, валиден по YAML-схеме и задаёт runtime org.freedesktop.Platform 23.08", () => {
    assert.ok(
      existsSync(MANIFEST_PATH),
      `Манифест Flatpak должен существовать по пути: ${MANIFEST_PATH}`,
    );

    const st = statSync(MANIFEST_PATH);
    assert.ok(st.size > 200, "Манифест Flatpak не должен быть пустым или тривиальным");

    const rawText = readFileSync(MANIFEST_PATH, "utf8");
    assert.doesNotMatch(rawText, /\t/, "YAML-манифест не должен содержать символов табуляции");

    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    const oracleSums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const validation = validateFlatpakManifest(manifest, oracleSums);
    assert.equal(
      validation.valid,
      true,
      `Ошибки валидации манифеста Flatpak:\n${validation.errors.join("\n")}`,
    );

    assert.equal(manifest["app-id"], "com.discord_lab.DiscordGpui");
    assert.equal(manifest.runtime, "org.freedesktop.Platform");
    assert.equal(String(manifest["runtime-version"]), "23.08");
    assert.equal(manifest.sdk, "org.freedesktop.Sdk");
    assert.equal(manifest.command, "discord-gpui");
  });

  it("SUB-962-2: finish-args включает обязательные права --socket=wayland, --socket=fallback-x11, --filesystem=xdg-run/pipewire-0 и Secret Service portal", () => {
    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    /** @type {string[]} */
    const finishArgs = manifest["finish-args"];
    assert.ok(Array.isArray(finishArgs), "finish-args должен быть массивом");

    // Явные требования из задания TASK-962
    assert.ok(
      finishArgs.includes("--socket=wayland"),
      "Должно быть разрешено подключение к сокету Wayland (--socket=wayland)",
    );
    assert.ok(
      finishArgs.includes("--socket=fallback-x11"),
      "Должен быть настроен резервный сокет X11 (--socket=fallback-x11)",
    );
    assert.ok(
      finishArgs.includes("--filesystem=xdg-run/pipewire-0"),
      "Должен быть открыт доступ к сокету PipeWire (--filesystem=xdg-run/pipewire-0)",
    );
    assert.ok(
      finishArgs.includes("--talk-name=org.freedesktop.secrets"),
      "Должен быть разрешён доступ к Secret Service API (--talk-name=org.freedesktop.secrets)",
    );
    assert.ok(
      finishArgs.includes("--talk-name=org.freedesktop.portal.Desktop"),
      "Должен быть разрешён доступ к порталу XDG Desktop / Secret / ScreenCast (--talk-name=org.freedesktop.portal.Desktop)",
    );

    // Аппаратное ускорение WGPU, сеть, IPC и системная интеграция
    assert.ok(finishArgs.includes("--device=dri"), "Должно быть включено GPU-ускорение DRI (--device=dri)");
    assert.ok(finishArgs.includes("--share=network"), "Должен быть разрешён сетевой доступ (--share=network)");
    assert.ok(finishArgs.includes("--share=ipc"), "Должен быть разрешён IPC для X11 MIT-SHM fallback (--share=ipc)");
    assert.ok(finishArgs.includes("--socket=pulseaudio"), "Должен быть разрешён аудиосокет (--socket=pulseaudio)");
    assert.ok(
      finishArgs.includes("--talk-name=org.freedesktop.Notifications"),
      "Должны быть разрешены системные уведомления (--talk-name=org.freedesktop.Notifications)",
    );
    assert.ok(
      finishArgs.includes("--talk-name=org.kde.StatusNotifierWatcher"),
      "Должен быть разрешён системный трей SNI (--talk-name=org.kde.StatusNotifierWatcher)",
    );
    assert.ok(
      finishArgs.includes("--talk-name=org.a11y.Bus"),
      "Должна быть разрешена шина доступности AT-SPI (--talk-name=org.a11y.Bus)",
    );
  });

  it("SUB-962-3: права sandboxing строго минимальны (отсутствуют --filesystem=host/home, прямой --socket=x11, --device=all, --socket=session-bus/system-bus)", () => {
    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    /** @type {string[]} */
    const finishArgs = manifest["finish-args"];

    assert.ok(
      !finishArgs.includes("--socket=x11"),
      "Запрещён небезопасный --socket=x11 при наличии Wayland (допустим только --socket=fallback-x11)",
    );
    assert.ok(
      !finishArgs.some((a) => a.startsWith("--filesystem=host") || a.startsWith("--filesystem=home")),
      "Запрещён полный доступ к файловой системе хоста или домашней директории",
    );
    assert.ok(
      !finishArgs.includes("--device=all"),
      "Запрещён полный доступ ко всем устройствам (--device=all)",
    );
    assert.ok(
      !finishArgs.includes("--socket=session-bus") && !finishArgs.includes("--socket=system-bus"),
      "Запрещён нефильтрованный доступ к сессионной или системной шине D-Bus",
    );
    assert.ok(
      !finishArgs.some((a) => a.startsWith("--allow=")),
      "Запрещены привилегированные флаги --allow=devel / --allow=multiarch",
    );

    // Проверка отсутствия дубликатов
    assert.equal(
      new Set(finishArgs).size,
      finishArgs.length,
      "В finish-args не должно быть дублирующихся разрешений",
    );
  });

  it("SUB-962-4: модуль discord-gpui и его источники ссылаются на реальные артефакты rust/dist/ с точным совпадением SHA-256 по оракулу SHA256SUMS", () => {
    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    const oracleSums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));

    const mod = manifest.modules.find((/** @type {any} */ m) => m.name === "discord-gpui");
    assert.ok(mod, "Модуль discord-gpui должен присутствовать в секции modules");
    assert.equal(mod.buildsystem, "simple");

    /** @type {any[]} */
    const sources = mod.sources;
    assert.ok(Array.isArray(sources) && sources.length >= 2, "Должны быть описаны источники сборки");

    for (const src of sources) {
      const resolvedPath = resolve(MANIFEST_DIR, src.path);
      assert.ok(
        existsSync(resolvedPath),
        `Файл источника должен физически существовать на диске: ${resolvedPath}`,
      );

      const fileName = basename(resolvedPath);
      const expectedFromSums = oracleSums.get(fileName);
      assert.ok(
        expectedFromSums,
        `Артефакт ${fileName} должен быть зарегистрирован в оракуле rust/dist/SHA256SUMS`,
      );

      const actualFileHash = sha256File(resolvedPath);
      assert.equal(
        src.sha256,
        expectedFromSums,
        `Поле sha256 в манифесте для ${fileName} должно совпадать с rust/dist/SHA256SUMS`,
      );
      assert.equal(
        src.sha256,
        actualFileHash,
        `Поле sha256 в манифесте для ${fileName} должно совпадать с фактическим SHA-256 файла`,
      );
    }
  });

  it("SUB-962-5: валидатор validateFlatpakManifest отсекает негативные, граничные и небезопасные мутации манифеста", () => {
    const baseManifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    const oracleSums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));

    // 1. Пустой / некорректный корень
    assert.equal(validateFlatpakManifest(null, oracleSums).valid, false);
    assert.equal(validateFlatpakManifest([], oracleSums).valid, false);

    // 2. Неверная версия рантайма (например, 22.08 вместо 23.08)
    const wrongRuntimeVer = structuredClone(baseManifest);
    wrongRuntimeVer["runtime-version"] = "22.08";
    const resRuntime = validateFlatpakManifest(wrongRuntimeVer, oracleSums);
    assert.equal(resRuntime.valid, false);
    assert.match(resRuntime.errors.join("; "), /23\.08/);

    // 3. Потеря обязательного права --filesystem=xdg-run/pipewire-0
    const missingPipewire = structuredClone(baseManifest);
    missingPipewire["finish-args"] = missingPipewire["finish-args"].filter(
      (/** @type {string} */ a) => a !== "--filesystem=xdg-run/pipewire-0",
    );
    const resNoPw = validateFlatpakManifest(missingPipewire, oracleSums);
    assert.equal(resNoPw.valid, false);
    assert.match(resNoPw.errors.join("; "), /--filesystem=xdg-run\/pipewire-0/);

    // 4. Потеря Secret Service portal (--talk-name=org.freedesktop.secrets)
    const missingSecrets = structuredClone(baseManifest);
    missingSecrets["finish-args"] = missingSecrets["finish-args"].filter(
      (/** @type {string} */ a) => a !== "--talk-name=org.freedesktop.secrets",
    );
    const resNoSec = validateFlatpakManifest(missingSecrets, oracleSums);
    assert.equal(resNoSec.valid, false);
    assert.match(resNoSec.errors.join("; "), /org\.freedesktop\.secrets/);

    // 5. Попытка побега из песочницы (--filesystem=host, --socket=x11, --device=all)
    for (const forbiddenArg of [
      "--filesystem=host",
      "--filesystem=home",
      "--socket=x11",
      "--device=all",
      "--socket=session-bus",
      "--allow=devel",
    ]) {
      const overPrivileged = structuredClone(baseManifest);
      overPrivileged["finish-args"].push(forbiddenArg);
      const resForbidden = validateFlatpakManifest(overPrivileged, oracleSums);
      assert.equal(
        resForbidden.valid,
        false,
        `Манифест с избыточным правом ${forbiddenArg} должен отклоняться`,
      );
    }

    // 6. Несовпадение контрольной суммы источника с оракулом SHA256SUMS
    const badSha = structuredClone(baseManifest);
    badSha.modules[0].sources[0].sha256 = "0".repeat(64);
    const resBadSha = validateFlatpakManifest(badSha, oracleSums);
    assert.equal(resBadSha.valid, false);
    assert.match(resBadSha.errors.join("; "), /не совпадает с оракулом SHA256SUMS/);
  });
});
