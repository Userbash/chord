// @ts-check
/**
 * Контрактные и модульные тесты спецификации пакета Snap и песочницы AppArmor/Plugs (TASK-967):
 * - SUB-967-1: Валидация YAML-схемы `packaging/snap/snapcraft.yaml` независимым оракулом
 *   (Python 3 PyYAML), проверка метаданных пакета (`name: discord-gpui`, `base: core22`,
 *   `version: 1.0.0-beta.1`, лимит `summary <= 78` символов, `grade: stable`, `confinement: strict`).
 * - SUB-967-2: Проверка обязательных плагов AppArmor/SecComp (`wayland`, `desktop`,
 *   `audio-playback`, `audio-record`, `password-manager-service`, `network`) и графического/медиа
 *   окружения для `apps.discord-gpui`.
 * - SUB-967-3: Верификация режимов изоляции `strict` vs `classic` (`verifySnapConfinement`)
 *   и строгой минимальности прав песочницы (запрет `home`, `system-files`, `personal-files`,
 *   `removable-media`, `process-control`, `network-control`, `ssh-keys`, `devmode`).
 * - SUB-967-4: Проверка секции `parts` (`discord-gpui`, `sbom`) и побайтовая сверка
 *   `source-checksum: sha256/<hex>` с независимым оракулом `rust/dist/SHA256SUMS` и файлами на диске.
 * - SUB-967-5: Негативные, граничные и защитные сценарии валидаторов (9 измерений TEST-POLICY.md).
 */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { basename, join, resolve } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SNAP_DIR = join(ROOT, "packaging", "snap");
const MANIFEST_PATH = join(SNAP_DIR, "snapcraft.yaml");
const DIST_DIR = join(ROOT, "rust", "dist");
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");

/** Максимальная длина поля summary по спецификации Snapcraft. */
const MAX_SUMMARY_LENGTH = 78;

/** Обязательные плаги Snap по контракту TASK-967. */
export const REQUIRED_SNAP_PLUGS = Object.freeze([
  "wayland",
  "desktop",
  "audio-playback",
  "audio-record",
  "password-manager-service",
  "network",
]);

/** Дополнительные разрешённые плаги для WGPU/WebRTC/X11-fallback/ScreenCast в песочнице. */
export const ALLOWED_OPTIONAL_SNAP_PLUGS = Object.freeze([
  "desktop-legacy",
  "x11",
  "opengl",
  "network-bind",
  "screen-inhibit-control",
  "upower-observe",
]);

/** Запрещённые избыточные плаги, нарушающие изоляцию песочницы AppArmor/SecComp. */
export const FORBIDDEN_SNAP_PLUGS = Object.freeze(
  new Map([
    ["home", "Полный доступ к домашней директории (home) запрещён по принципу минимальных привилегий"],
    ["removable-media", "Доступ к внешним накопителям (removable-media) не требуется клиенту"],
    ["system-files", "Произвольный доступ к системным файлам хоста (system-files) запрещён"],
    ["personal-files", "Прямой доступ к скрытым файлам хоста (personal-files) запрещён; используйте Secret Service"],
    ["process-control", "Управление чужими процессами хоста (process-control) запрещено"],
    ["system-observe", "Мониторинг всех процессов и состояния ядра хоста (system-observe) запрещён"],
    ["network-control", "Изменение сетевого стека и маршрутизации хоста (network-control) запрещено"],
    ["firewall-control", "Управление межсетевым экраном хоста (firewall-control) запрещено"],
    ["hardware-observe", "Сырой доступ к инвентаризации оборудования хоста (hardware-observe) запрещён"],
    ["kernel-module-control", "Загрузка модулей ядра (kernel-module-control) запрещена"],
    ["docker", "Доступ к сокету Docker-демона (docker) запрещён"],
    ["lxd", "Доступ к гипервизору LXD (lxd) запрещён"],
    ["snapd-control", "Управление демоном snapd (snapd-control) запрещено"],
    ["ssh-keys", "Доступ к приватным SSH-ключам (ssh-keys) запрещён"],
    ["gpg-keys", "Доступ к приватным GPG-ключам (gpg-keys) запрещён"],
  ]),
);

/** Паттерн обнаружения немаскированных токенов Discord в переменных окружения. */
const SECRET_TOKEN_PATTERN = /(mfa\.[A-Za-z0-9_-]{10,}|Bot\s+[A-Za-z0-9._-]{20,})/;

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
 * Верифицирует режим изоляции Snap (`strict` vs `classic`) и профиль AppArmor/Plugs.
 *
 * @param {any} manifest
 * @param {{ allowClassic?: boolean }} [options]
 * @returns {{
 *   valid: boolean,
 *   mode: "strict" | "classic" | "devmode" | "invalid",
 *   isStrictlyConfined: boolean,
 *   appArmorIsolated: boolean,
 *   errors: string[],
 *   warnings: string[]
 * }}
 */
export function verifySnapConfinement(manifest, options = {}) {
  const allowClassic = options.allowClassic ?? false;
  /** @type {string[]} */
  const errors = [];
  /** @type {string[]} */
  const warnings = [];

  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    return {
      valid: false,
      mode: "invalid",
      isStrictlyConfined: false,
      appArmorIsolated: false,
      errors: ["Манифест должен быть непустым объектом"],
      warnings,
    };
  }

  const rawConfinement = manifest.confinement;
  const rawGrade = manifest.grade;

  if (rawConfinement !== "strict" && rawConfinement !== "classic" && rawConfinement !== "devmode") {
    errors.push(
      `Недопустимое значение confinement: "${String(rawConfinement)}" (ожидалось "strict" или "classic")`,
    );
    return {
      valid: false,
      mode: "invalid",
      isStrictlyConfined: false,
      appArmorIsolated: false,
      errors,
      warnings,
    };
  }

  if (rawConfinement === "devmode") {
    errors.push(
      "Режим confinement: devmode запрещён для релизных сборок, так как отключает принудительную блокировку AppArmor",
    );
    return {
      valid: false,
      mode: "devmode",
      isStrictlyConfined: false,
      appArmorIsolated: false,
      errors,
      warnings,
    };
  }

  if (rawConfinement === "classic") {
    warnings.push(
      "Режим classic отключает изолированное пространство имён файловой системы и мандатный профиль AppArmor песочницы",
    );
    if (!allowClassic) {
      errors.push(
        "Политика безопасности discord-gpui требует confinement: strict (режим classic отклонён без явного флага allowClassic)",
      );
    }
    return {
      valid: errors.length === 0,
      mode: "classic",
      isStrictlyConfined: false,
      appArmorIsolated: false,
      errors,
      warnings,
    };
  }

  // Режим strict
  if (rawGrade !== "stable") {
    errors.push(
      `Для строго изолированного пакета (confinement: strict) поле grade обязано быть "stable", получено: "${String(rawGrade)}"`,
    );
  }

  const app = manifest.apps?.["discord-gpui"];
  const plugs = app?.plugs;
  if (!Array.isArray(plugs) || plugs.length === 0) {
    errors.push("В режиме strict для apps.discord-gpui должен быть объявлен непустой массив plugs");
  } else {
    const plugSet = new Set();
    for (const p of plugs) {
      if (typeof p !== "string" || !p.trim()) {
        errors.push(`Некорректное имя плага в apps.discord-gpui.plugs: "${String(p)}"`);
        continue;
      }
      if (plugSet.has(p)) {
        errors.push(`Дублирующийся плаг в apps.discord-gpui.plugs: "${p}"`);
      }
      plugSet.add(p);

      const forbiddenReason = FORBIDDEN_SNAP_PLUGS.get(p);
      if (forbiddenReason) {
        errors.push(`Запрещённый плаг "${p}" нарушает strict-песочницу: ${forbiddenReason}`);
      }
    }

    for (const reqPlug of REQUIRED_SNAP_PLUGS) {
      if (!plugSet.has(reqPlug)) {
        errors.push(`Отсутствует обязательный плаг песочницы: "${reqPlug}"`);
      }
    }
  }

  return {
    valid: errors.length === 0,
    mode: "strict",
    isStrictlyConfined: errors.length === 0,
    appArmorIsolated: errors.length === 0,
    errors,
    warnings,
  };
}

/**
 * Полная валидация манифеста `packaging/snap/snapcraft.yaml` по схеме Snapcraft,
 * политике strict confinement и оракулу контрольных сумм `rust/dist/SHA256SUMS`.
 *
 * @param {any} manifest
 * @param {Map<string, string>} [oracleSums]
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateSnapcraftManifest(manifest, oracleSums) {
  /** @type {string[]} */
  const errors = [];

  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    return { valid: false, errors: ["Манифест Snapcraft должен быть непустым YAML-объектом"] };
  }

  if (manifest.name !== "discord-gpui") {
    errors.push(`Ожидалось имя пакета name: "discord-gpui", получено: "${String(manifest.name)}"`);
  }

  if (manifest.base !== "core22" && manifest.base !== "core24") {
    errors.push(`Ожидался базовый образ base: "core22" (или "core24"), получено: "${String(manifest.base)}"`);
  }

  if (typeof manifest.version !== "string" || !/^\d+\.\d+\.\d+(-[a-z0-9.]+)?$/i.test(manifest.version)) {
    errors.push(`Некорректный формат version: "${String(manifest.version)}"`);
  }

  if (typeof manifest.summary !== "string" || manifest.summary.trim().length === 0) {
    errors.push("Поле summary должно быть непустой строкой");
  } else if (manifest.summary.length > MAX_SUMMARY_LENGTH) {
    errors.push(
      `Длина поля summary (${manifest.summary.length}) превышает лимит Snapcraft в ${MAX_SUMMARY_LENGTH} символов`,
    );
  }

  if (typeof manifest.description !== "string" || manifest.description.trim().length < 20) {
    errors.push("Поле description должно содержать подробное описание пакета (не менее 20 символов)");
  }

  if (manifest.license !== "MIT OR Apache-2.0") {
    errors.push(`Ожидалась лицензия SPDX "MIT OR Apache-2.0", получено: "${String(manifest.license)}"`);
  }

  if (manifest.compression !== "lzo" && manifest.compression !== "xz") {
    errors.push(`Ожидался алгоритм сжатия compression: "lzo" (или "xz"), получено: "${String(manifest.compression)}"`);
  }

  // Проверка секции architectures
  if (!Array.isArray(manifest.architectures) || manifest.architectures.length === 0) {
    errors.push("Секция architectures должна быть непустым массивом");
  } else {
    const hasAmd64 = manifest.architectures.some((/** @type {any} */ arch) => {
      if (typeof arch === "string") return arch === "amd64";
      if (arch && typeof arch === "object") {
        const buildOn = arch["build-on"];
        return Array.isArray(buildOn) ? buildOn.includes("amd64") : buildOn === "amd64";
      }
      return false;
    });
    if (!hasAmd64) {
      errors.push("Секция architectures должна включать целевую архитектуру amd64");
    }
  }

  // Проверка изоляции strict/classic и плагов
  const confinementCheck = verifySnapConfinement(manifest, { allowClassic: false });
  if (!confinementCheck.valid) {
    errors.push(...confinementCheck.errors);
  }

  // Проверка приложения apps.discord-gpui
  const app = manifest.apps?.["discord-gpui"];
  if (!app || typeof app !== "object") {
    errors.push("В секции apps должно быть определено приложение 'discord-gpui'");
  } else {
    if (typeof app.command !== "string" || !app.command.includes("discord-gpui")) {
      errors.push(
        `Поле apps.discord-gpui.command должно запускать бинарник discord-gpui, получено: "${String(app.command)}"`,
      );
    }

    if (app.environment && typeof app.environment === "object") {
      for (const [envKey, envVal] of Object.entries(app.environment)) {
        if (SECRET_TOKEN_PATTERN.test(String(envVal))) {
          errors.push(
            `Обнаружен немаскированный секретный токен в переменной окружения apps.discord-gpui.environment.${envKey}`,
          );
        }
      }
    }
  }

  // Проверка секции top-level plugs (например, комментариев/конфигурации автоподключения, если задана)
  if (manifest.plugs !== undefined && (typeof manifest.plugs !== "object" || Array.isArray(manifest.plugs))) {
    errors.push("Секция верхнего уровня plugs (если присутствует) должна быть объектом");
  }

  // Проверка секции parts и контрольных сумм SHA-256
  const parts = manifest.parts;
  if (!parts || typeof parts !== "object" || Array.isArray(parts)) {
    errors.push("Секция parts должна быть непустым объектом");
  } else {
    const mainPart = parts["discord-gpui"];
    if (!mainPart || typeof mainPart !== "object") {
      errors.push("В секции parts должна присутствовать часть 'discord-gpui'");
    } else {
      if (mainPart.plugin !== "dump" && mainPart.plugin !== "nil") {
        errors.push(
          `Часть parts.discord-gpui должна использовать plugin "dump" или "nil", получено: "${String(mainPart.plugin)}"`,
        );
      }

      if (typeof mainPart.source !== "string" || !mainPart.source.trim()) {
        errors.push("Часть parts.discord-gpui должна содержать непустой путь source");
      }

      const checksum = mainPart["source-checksum"];
      if (typeof checksum !== "string" || !/^sha256\/[a-f0-9]{64}$/.test(checksum)) {
        errors.push(
          `Поле parts.discord-gpui.source-checksum должно иметь формат "sha256/<64-hex>", получено: "${String(checksum)}"`,
        );
      } else if (oracleSums && typeof mainPart.source === "string") {
        const digest = checksum.slice("sha256/".length);
        const fileBase = basename(mainPart.source);
        const expectedHash = oracleSums.get(fileBase);
        if (!expectedHash) {
          errors.push(`Артефакт источника "${fileBase}" отсутствует в оракуле rust/dist/SHA256SUMS`);
        } else if (digest !== expectedHash) {
          errors.push(
            `Хеш source-checksum для "${fileBase}" (${digest}) не совпадает с оракулом SHA256SUMS (${expectedHash})`,
          );
        }
      }
    }

    // Проверка всех остальных частей, у которых указан source-checksum
    for (const [partName, partObj] of Object.entries(parts)) {
      if (!partObj || typeof partObj !== "object" || partName === "discord-gpui") continue;
      if (partObj["source-checksum"] !== undefined) {
        const cs = partObj["source-checksum"];
        if (typeof cs !== "string" || !/^sha256\/[a-f0-9]{64}$/.test(cs)) {
          errors.push(
            `Поле parts.${partName}.source-checksum должно иметь формат "sha256/<64-hex>", получено: "${String(cs)}"`,
          );
        } else if (oracleSums && typeof partObj.source === "string") {
          const digest = cs.slice("sha256/".length);
          const fileBase = basename(partObj.source);
          const expectedHash = oracleSums.get(fileBase);
          if (!expectedHash) {
            errors.push(`Артефакт источника "${fileBase}" (part ${partName}) отсутствует в rust/dist/SHA256SUMS`);
          } else if (digest !== expectedHash) {
            errors.push(
              `Хеш source-checksum для "${fileBase}" (part ${partName}: ${digest}) не совпадает с оракулом SHA256SUMS (${expectedHash})`,
            );
          }
        }
      }
    }
  }

  return { valid: errors.length === 0, errors };
}

describe("TASK-967: Спецификация пакета Snap packaging/snap/snapcraft.yaml и верификация strict/classic confinement", () => {
  it("SUB-967-1: манифест packaging/snap/snapcraft.yaml существует, валиден по YAML-схеме (оракул PyYAML) и задаёт базовый образ core22", () => {
    assert.ok(
      existsSync(MANIFEST_PATH),
      `Манифест Snapcraft должен существовать по пути: ${MANIFEST_PATH}`,
    );

    const st = statSync(MANIFEST_PATH);
    assert.ok(st.size > 300, "Манифест Snapcraft не должен быть пустым или тривиальным");

    const rawText = readFileSync(MANIFEST_PATH, "utf8");
    assert.doesNotMatch(rawText, /\t/, "YAML-манифест не должен содержать символов табуляции");

    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    const oracleSums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const validation = validateSnapcraftManifest(manifest, oracleSums);
    assert.equal(
      validation.valid,
      true,
      `Ошибки валидации манифеста Snapcraft:\n${validation.errors.join("\n")}`,
    );

    assert.equal(manifest.name, "discord-gpui");
    assert.equal(manifest.base, "core22");
    assert.equal(manifest.version, "1.0.0-beta.1");
    assert.equal(manifest.grade, "stable");
    assert.equal(manifest.confinement, "strict");
    assert.equal(manifest.compression, "lzo");
    assert.equal(manifest.license, "MIT OR Apache-2.0");
    assert.ok(
      manifest.summary.length <= MAX_SUMMARY_LENGTH,
      `Поле summary не должно превышать ${MAX_SUMMARY_LENGTH} символов`,
    );
  });

  it("SUB-967-2: в apps.discord-gpui объявлены все обязательные плаги wayland, desktop, audio-playback, audio-record, password-manager-service, network", () => {
    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    const app = manifest.apps?.["discord-gpui"];
    assert.ok(app, "Приложение apps.discord-gpui должно быть определено");

    /** @type {string[]} */
    const plugs = app.plugs;
    assert.ok(Array.isArray(plugs), "apps.discord-gpui.plugs должен быть массивом");

    // 6 обязательных плагов из контракта TASK-967
    for (const requiredPlug of REQUIRED_SNAP_PLUGS) {
      assert.ok(
        plugs.includes(requiredPlug),
        `В apps.discord-gpui.plugs обязан присутствовать плаг "${requiredPlug}"`,
      );
    }

    // Проверка сопутствующих плагов графического ускорения WGPU и WebRTC-транспорта
    assert.ok(plugs.includes("desktop-legacy"), "Должен быть подключён плаг desktop-legacy (трей SNI / XDG)");
    assert.ok(plugs.includes("x11"), "Должен быть подключён резервный плаг x11");
    assert.ok(plugs.includes("opengl"), "Должен быть подключён плаг opengl (WGPU / DRI / VA-API)");
    assert.ok(plugs.includes("network-bind"), "Должен быть подключён плаг network-bind (WebRTC UDP ICE)");

    // Все объявленные плаги входят только в белый список (обязательные + разрешённые опциональные)
    const allowedSet = new Set([...REQUIRED_SNAP_PLUGS, ...ALLOWED_OPTIONAL_SNAP_PLUGS]);
    for (const plug of plugs) {
      assert.ok(
        allowedSet.has(plug),
        `Обнаружен неразрешённый или неизвестный плаг "${plug}" в apps.discord-gpui.plugs`,
      );
    }

    // Отсутствие дубликатов
    assert.equal(new Set(plugs).size, plugs.length, "В списке plugs не должно быть дубликатов");
  });

  it("SUB-967-3: верификация strict vs classic confinement подтверждает строгую изоляцию AppArmor и отклоняет classic/devmode", () => {
    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);

    // 1. Текущий манифест проходит строгую верификацию strict confinement
    const strictReport = verifySnapConfinement(manifest);
    assert.equal(strictReport.valid, true, `Ошибки strict confinement: ${strictReport.errors.join("; ")}`);
    assert.equal(strictReport.mode, "strict");
    assert.equal(strictReport.isStrictlyConfined, true);
    assert.equal(strictReport.appArmorIsolated, true);
    assert.equal(strictReport.warnings.length, 0);

    // 2. Мутация в classic confinement при стандартной политике (allowClassic: false) блокируется
    const classicManifest = structuredClone(manifest);
    classicManifest.confinement = "classic";
    const classicRejected = verifySnapConfinement(classicManifest, { allowClassic: false });
    assert.equal(classicRejected.valid, false);
    assert.equal(classicRejected.mode, "classic");
    assert.equal(classicRejected.isStrictlyConfined, false);
    assert.equal(classicRejected.appArmorIsolated, false);
    assert.ok(classicRejected.warnings.length > 0);
    assert.match(classicRejected.errors.join("; "), /confinement: strict/);

    // 3. Верификация classic confinement при явном разрешении (allowClassic: true) помечает отсутствие изоляции AppArmor
    const classicAllowed = verifySnapConfinement(classicManifest, { allowClassic: true });
    assert.equal(classicAllowed.valid, true);
    assert.equal(classicAllowed.mode, "classic");
    assert.equal(classicAllowed.isStrictlyConfined, false);
    assert.equal(classicAllowed.appArmorIsolated, false);
    assert.match(classicAllowed.warnings.join("; "), /AppArmor/);

    // 4. Режим devmode безусловно отклоняется в любом режиме проверки
    const devmodeManifest = structuredClone(manifest);
    devmodeManifest.confinement = "devmode";
    const devmodeRes = verifySnapConfinement(devmodeManifest, { allowClassic: true });
    assert.equal(devmodeRes.valid, false);
    assert.equal(devmodeRes.mode, "devmode");
    assert.match(devmodeRes.errors.join("; "), /devmode запрещён/);
  });

  it("SUB-967-4: секция parts ссылается на реальные артефакты rust/dist/ с точным совпадением SHA-256 по оракулу SHA256SUMS", () => {
    const manifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    const oracleSums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));

    const parts = manifest.parts;
    assert.ok(parts && typeof parts === "object", "Секция parts должна существовать");

    const checkedFiles = [];
    for (const [partName, part] of Object.entries(parts)) {
      assert.ok(typeof part.source === "string" && part.source.length > 0, `Часть ${partName} должна иметь source`);
      const resolvedPath = resolve(SNAP_DIR, part.source);
      assert.ok(
        existsSync(resolvedPath),
        `Файл источника для parts.${partName} должен физически существовать на диске: ${resolvedPath}`,
      );

      const fileName = basename(resolvedPath);
      const expectedHash = oracleSums.get(fileName);
      assert.ok(
        expectedHash,
        `Артефакт ${fileName} должен присутствовать в оракуле rust/dist/SHA256SUMS`,
      );

      const actualHash = sha256File(resolvedPath);
      assert.equal(
         part["source-checksum"],
        `sha256/${expectedHash}`,
        `source-checksum в parts.${partName} должен совпадать с оракулом rust/dist/SHA256SUMS`,
      );
      assert.equal(
        part["source-checksum"],
        `sha256/${actualHash}`,
        `source-checksum в parts.${partName} должен совпадать с фактическим SHA-256 файла на диске`,
      );
      checkedFiles.push(fileName);
    }

    assert.ok(
      checkedFiles.includes("discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz"),
      "Секция parts должна включать релизный архив discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz",
    );
    assert.ok(
      checkedFiles.includes("sbom.spdx.json"),
      "Секция parts должна включать верифицированный SBOM-манифест sbom.spdx.json",
    );
  });

  it("SUB-967-5: валидатор validateSnapcraftManifest отсекает негативные, граничные и небезопасные мутации манифеста", () => {
    const baseManifest = parseYamlWithPythonOracle(MANIFEST_PATH);
    const oracleSums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));

    // 1. Невалидные корневые структуры
    assert.equal(validateSnapcraftManifest(null, oracleSums).valid, false);
    assert.equal(validateSnapcraftManifest([], oracleSums).valid, false);
    assert.equal(validateSnapcraftManifest("invalid", oracleSums).valid, false);

    // 2. Граничные значения длины summary (78 символов -> valid, 79 символов -> invalid)
    const boundary78 = structuredClone(baseManifest);
    boundary78.summary = "A".repeat(78);
    assert.equal(validateSnapcraftManifest(boundary78, oracleSums).valid, true);

    const boundary79 = structuredClone(baseManifest);
    boundary79.summary = "A".repeat(79);
    const res79 = validateSnapcraftManifest(boundary79, oracleSums);
    assert.equal(res79.valid, false);
    assert.match(res79.errors.join("; "), /78 символов/);

    // 3. Удаление каждого из 6 обязательных плагов по очереди ловится валидатором
    for (const requiredPlug of REQUIRED_SNAP_PLUGS) {
      const missingPlugManifest = structuredClone(baseManifest);
      missingPlugManifest.apps["discord-gpui"].plugs =
        missingPlugManifest.apps["discord-gpui"].plugs.filter(
          (/** @type {string} */ p) => p !== requiredPlug,
        );
      const resMissing = validateSnapcraftManifest(missingPlugManifest, oracleSums);
      assert.equal(
        resMissing.valid,
        false,
        `Удаление обязательного плага "${requiredPlug}" должно приводить к ошибке валидации`,
      );
      assert.match(resMissing.errors.join("; "), new RegExp(requiredPlug));
    }

    // 4. Добавление запрещённых плагов (попытка побега из песочницы AppArmor)
    for (const forbiddenPlug of [
      "home",
      "system-files",
      "personal-files",
      "removable-media",
      "process-control",
      "network-control",
      "docker",
      "ssh-keys",
    ]) {
      const overPrivileged = structuredClone(baseManifest);
      overPrivileged.apps["discord-gpui"].plugs.push(forbiddenPlug);
      const resForbidden = validateSnapcraftManifest(overPrivileged, oracleSums);
      assert.equal(
        resForbidden.valid,
        false,
        `Манифест с избыточным плагом "${forbiddenPlug}" должен отклоняться`,
      );
      assert.match(resForbidden.errors.join("; "), new RegExp(forbiddenPlug));
    }

    // 5. Несоответствие grade: devel при strict confinement
    const develGrade = structuredClone(baseManifest);
    develGrade.grade = "devel";
    const resDevel = validateSnapcraftManifest(develGrade, oracleSums);
    assert.equal(resDevel.valid, false);
    assert.match(resDevel.errors.join("; "), /grade обязано быть "stable"/);

    // 6. Утечка секретного токена mfa.* в секции environment
    const leakedToken = structuredClone(baseManifest);
    leakedToken.apps["discord-gpui"].environment = {
      ...leakedToken.apps["discord-gpui"].environment,
      DISCORD_TOKEN: "mfa.secret_unmasked_token_value_12345",
    };
    const resLeak = validateSnapcraftManifest(leakedToken, oracleSums);
    assert.equal(resLeak.valid, false);
    assert.match(resLeak.errors.join("; "), /немаскированный секретный токен/);

    // 7. Несовпадение контрольной суммы source-checksum с оракулом SHA256SUMS
    const badChecksum = structuredClone(baseManifest);
    badChecksum.parts["discord-gpui"]["source-checksum"] = `sha256/${"f".repeat(64)}`;
    const resBadSum = validateSnapcraftManifest(badChecksum, oracleSums);
    assert.equal(resBadSum.valid, false);
    assert.match(resBadSum.errors.join("; "), /не совпадает с оракулом SHA256SUMS/);
  });
});
