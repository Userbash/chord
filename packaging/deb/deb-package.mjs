#!/usr/bin/env node
// @ts-check
/**
 * Генератор, валидатор и сборщик пакета Debian/Ubuntu (.deb) для нативного клиента `discord-gpui` (TASK-963):
 * - Генерация и RFC-822 парсинг/валидация `DEBIAN/control` с обязательными зависимостями
 *   `libc6 (>= 2.31)` и `libpipewire-0.3-0`.
 * - Генерация и валидация `.desktop`-файла XDG с регистрацией URI-схемы `discord://`
 *   (`MimeType=x-scheme-handler/discord;...`, `Exec=/usr/bin/discord-gpui --url -- %u`).
 * - Генерация и валидация пакета XDG Shared MIME-info (`usr/share/mime/packages/discord-gpui.xml`)
 *   и maintainer-скриптов (`DEBIAN/postinst`, `DEBIAN/postrm`).
 * - Сборка стандартного `.deb` архива (`ar`: `debian-binary`, `control.tar.gz`, `data.tar.gz`)
 *   с подсчётом `DEBIAN/md5sums` и `Installed-Size`.
 */
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
export const DEFAULT_DEB_ROOT = join(ROOT, "packaging", "deb");
export const DEFAULT_BINARY_PATH = join(ROOT, "rust", "dist", "discord-gpui");

/**
 * @typedef {object} DebianControlOptions
 * @property {string} [packageName]
 * @property {string} [version]
 * @property {string} [section]
 * @property {string} [priority]
 * @property {string} [architecture]
 * @property {number} [installedSizeKb]
 * @property {readonly string[]} [depends]
 * @property {readonly string[]} [recommends]
 * @property {string} [maintainer]
 * @property {string} [homepage]
 * @property {string} [synopsis]
 * @property {readonly string[]} [descriptionLines]
 */

export const DEFAULT_DEPENDS = Object.freeze([
  "libc6 (>= 2.31)",
  "libpipewire-0.3-0",
  "libasound2 (>= 1.2.2) | libasound2t64 (>= 1.2.2)",
  "libxcb1 (>= 1.14)",
  "libxkbcommon0 (>= 1.0.0)",
  "libxkbcommon-x11-0 (>= 1.0.0)",
  "libgcc-s1 (>= 3.0)",
  "xdg-utils",
  "shared-mime-info",
  "desktop-file-utils",
]);

export const DEFAULT_RECOMMENDS = Object.freeze([
  "libvulkan1",
  "mesa-vulkan-drivers",
  "xdg-desktop-portal",
  "xdg-desktop-portal-gtk | xdg-desktop-portal-gnome | xdg-desktop-portal-kde | xdg-desktop-portal-wlr",
  "libsecret-1-0",
  "pipewire-pulse | pipewire-alsa",
]);

/**
 * Сравнивает две пунктирные числовые версии (например, `2.31` и `2.35`).
 * @param {string} a
 * @param {string} b
 * @returns {number}
 */
export function compareDebianVersions(a, b) {
  const cleanA = (a.split(/[-~+]/)[0] ?? "").trim();
  const cleanB = (b.split(/[-~+]/)[0] ?? "").trim();
  const partsA = cleanA.split(".").map((n) => Number.parseInt(n, 10) || 0);
  const partsB = cleanB.split(".").map((n) => Number.parseInt(n, 10) || 0);
  const maxLen = Math.max(partsA.length, partsB.length);
  for (let i = 0; i < maxLen; i += 1) {
    const va = partsA[i] ?? 0;
    const vb = partsB[i] ?? 0;
    if (va !== vb) return va < vb ? -1 : 1;
  }
  return 0;
}

/**
 * Генерирует содержимое файла `DEBIAN/control` по стандарту Debian Policy Chapter 5.
 * @param {DebianControlOptions} [options]
 * @returns {string}
 */
export function generateDebianControl(options = {}) {
  const packageName = options.packageName ?? "discord-gpui";
  const version = options.version ?? "1.0.0~beta1-1";
  const section = options.section ?? "net";
  const priority = options.priority ?? "optional";
  const architecture = options.architecture ?? "amd64";
  const installedSizeKb = options.installedSizeKb ?? 42572;
  const depends = options.depends ?? DEFAULT_DEPENDS;
  const recommends = options.recommends ?? DEFAULT_RECOMMENDS;
  const maintainer =
    options.maintainer ?? "Discord Lab DevOps <devops@discord-lab.local>";
  const homepage =
    options.homepage ?? "https://github.com/discord-lab/discord-gpui";
  const synopsis =
    options.synopsis ??
    "Native Rust GPUI Discord client for Linux (Wayland/X11, PipeWire, WGPU 120 FPS)";
  const descriptionLines = options.descriptionLines ?? [
    "High-performance native Discord desktop client built on Rust and WGPU/GPUI.",
    ".",
    "Features hardware-accelerated 120 FPS rendering, PipeWire screen sharing via",
    "XDG Desktop Portal (DMA-BUF NV12), end-to-end voice encryption (DAVE MLS v1),",
    "and deep desktop integration including the discord:// URI handler.",
  ];

  const formattedBody = descriptionLines
    .map((line) => (line.trim() === "." ? " ." : ` ${line}`))
    .join("\n");

  return [
    `Package: ${packageName}`,
    `Version: ${version}`,
    `Section: ${section}`,
    `Priority: ${priority}`,
    `Architecture: ${architecture}`,
    `Installed-Size: ${Math.max(1, Math.ceil(installedSizeKb))}`,
    `Depends: ${depends.join(", ")}`,
    `Recommends: ${recommends.join(", ")}`,
    `Provides: discord-client`,
    `Maintainer: ${maintainer}`,
    `Homepage: ${homepage}`,
    `Description: ${synopsis}`,
    formattedBody,
    "",
  ].join("\n");
}

/**
 * Парсит RFC-822 структуру `DEBIAN/control`.
 * @param {string} content
 * @returns {{ fields: Map<string, string>, continuationLines: string[], syntaxErrors: string[] }}
 */
export function parseDebianControl(content) {
  /** @type {Map<string, string>} */
  const fields = new Map();
  /** @type {string[]} */
  const continuationLines = [];
  /** @type {string[]} */
  const syntaxErrors = [];

  const lines = content.split(/\r?\n/);
  let currentField = "";

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i] ?? "";
    if (line.trim() === "") continue;
    if (line.startsWith("#")) continue;

    if (line.startsWith(" ") || line.startsWith("\t")) {
      if (!currentField) {
        syntaxErrors.push(`Строка ${i + 1}: продолжение поля без заголовка`);
      } else {
        continuationLines.push(line);
        const prev = fields.get(currentField) ?? "";
        fields.set(currentField, `${prev}\n${line}`);
      }
      continue;
    }

    const match = /^([A-Za-z0-9-]+):\s*(.*)$/.exec(line);
    if (!match) {
      syntaxErrors.push(`Строка ${i + 1}: некорректный синтаксис поля "${line}"`);
      continue;
    }
    const key = match[1] ?? "";
    const val = match[2] ?? "";
    currentField = key;
    fields.set(key, val.trim());
  }

  return { fields, continuationLines, syntaxErrors };
}

/**
 * Валидирует `DEBIAN/control` на соответствие Debian Policy и требованиям TASK-963.
 * @param {string} content
 * @returns {{ valid: boolean, errors: string[], parsed: ReturnType<typeof parseDebianControl> }}
 */
export function validateDebianControl(content) {
  /** @type {string[]} */
  const errors = [];
  if (!content || content.trim().length === 0) {
    return {
      valid: false,
      errors: ["Файл DEBIAN/control пуст"],
      parsed: { fields: new Map(), continuationLines: [], syntaxErrors: [] },
    };
  }

  const parsed = parseDebianControl(content);
  errors.push(...parsed.syntaxErrors);

  const requiredFields = [
    "Package",
    "Version",
    "Architecture",
    "Maintainer",
    "Depends",
    "Description",
  ];
  for (const req of requiredFields) {
    const val = parsed.fields.get(req);
    if (!val || val.trim() === "") {
      errors.push(`Отсутствует обязательное поле ${req}`);
    }
  }

  const pkg = parsed.fields.get("Package") ?? "";
  if (pkg && !/^[a-z0-9][a-z0-9+.-]+$/.test(pkg)) {
    errors.push(
      `Некорректное имя пакета Package: "${pkg}" (разрешены только строчные латинские буквы, цифры и +.-)`,
    );
  }

  const ver = parsed.fields.get("Version") ?? "";
  if (ver && !/^[0-9][A-Za-z0-9.+:~-]*$/.test(ver)) {
    errors.push(`Некорректный формат Version: "${ver}"`);
  }

  const arch = parsed.fields.get("Architecture") ?? "";
  if (arch && !["amd64", "arm64", "all", "any"].includes(arch)) {
    errors.push(`Неподдерживаемая архитектура Architecture: "${arch}"`);
  }

  const depends = parsed.fields.get("Depends") ?? "";
  const libcMatch = /\blibc6\s*\(>=\s*([0-9][0-9A-Za-z.+:~-]*)\)/.exec(depends);
  if (!libcMatch) {
    errors.push("Поле Depends должно содержать зависимость libc6 (>= 2.31)");
  } else {
    const libcVer = libcMatch[1] ?? "0";
    if (compareDebianVersions(libcVer, "2.31") < 0) {
      errors.push(
        `Версия зависимости libc6 (${libcVer}) ниже минимально допустимой 2.31`,
      );
    }
  }

  if (!/\blibpipewire-0\.3-0\b/.test(depends)) {
    errors.push("Поле Depends должно содержать зависимость libpipewire-0.3-0");
  }

  return {
    valid: errors.length === 0,
    errors,
    parsed,
  };
}

/**
 * Генерирует содержимое `.desktop` файла с регистрацией URI-схемы `discord://`.
 * @returns {string}
 */
export function generateDesktopEntry() {
  return [
    "[Desktop Entry]",
    "Version=1.0",
    "Name=Discord GPUI",
    "GenericName=Internet Messenger",
    "Comment=All-in-one voice, video, and text chat for gamers powered by native Rust GPUI",
    "Comment[ru]=Голосовой, видео и текстовый чат на базе нативного движка Rust GPUI",
    "Exec=/usr/bin/discord-gpui --url -- %u",
    "TryExec=/usr/bin/discord-gpui",
    "Icon=discord-gpui",
    "Terminal=false",
    "Type=Application",
    "Categories=Network;InstantMessaging;Chat;",
    "MimeType=x-scheme-handler/discord;x-scheme-handler/disclip;application/x-discord-gpui-invite;",
    "Keywords=discord;chat;voice;video;gpui;wayland;pipewire;",
    "StartupWMClass=discord-gpui",
    "StartupNotify=true",
    "",
  ].join("\n");
}

/**
 * Проверяет корректность `.desktop` файла и регистрацию `discord://` + XDG mime-типов.
 * @param {string} content
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateDesktopEntry(content) {
  /** @type {string[]} */
  const errors = [];
  if (!content.includes("[Desktop Entry]")) {
    errors.push("Отсутствует секция [Desktop Entry]");
  }
  if (!/^Type=Application$/m.test(content)) {
    errors.push("Поле Type должно быть равно Application");
  }
  if (!/^Name=.+$/m.test(content)) {
    errors.push("Отсутствует поле Name");
  }
  if (!/^Exec=\/usr\/bin\/discord-gpui\s+--url\s+--\s+%[uU]$/m.test(content)) {
    errors.push(
      "Поле Exec должно содержать '/usr/bin/discord-gpui --url -- %u' для передачи URI discord://",
    );
  }
  const mimeMatch = /^MimeType=(.+)$/m.exec(content);
  if (!mimeMatch) {
    errors.push("Отсутствует поле MimeType");
  } else {
    const mimes = (mimeMatch[1] ?? "")
      .split(";")
      .map((s) => s.trim())
      .filter(Boolean);
    if (!mimes.includes("x-scheme-handler/discord")) {
      errors.push("MimeType не содержит x-scheme-handler/discord");
    }
    if (!mimes.includes("application/x-discord-gpui-invite")) {
      errors.push("MimeType не содержит application/x-discord-gpui-invite");
    }
  }
  return { valid: errors.length === 0, errors };
}

/**
 * Генерирует XML-манифест XDG Shared MIME-info (`usr/share/mime/packages/discord-gpui.xml`).
 * @returns {string}
 */
export function generateMimeXml() {
  return [
    '<?xml version="1.0" encoding="UTF-8"?>',
    '<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">',
    '  <mime-type type="x-scheme-handler/discord">',
    "    <comment>Discord URI Scheme (discord://)</comment>",
    '    <comment xml:lang="ru">Схема URI Discord (discord://)</comment>',
    "  </mime-type>",
    '  <mime-type type="x-scheme-handler/disclip">',
    "    <comment>Discord Clip URI Scheme (disclip://)</comment>",
    '    <comment xml:lang="ru">Схема URI клипов Discord (disclip://)</comment>',
    "  </mime-type>",
    '  <mime-type type="application/x-discord-gpui-invite">',
    "    <comment>Discord GPUI Server Invite</comment>",
    '    <comment xml:lang="ru">Приглашение на сервер Discord GPUI</comment>',
    '    <glob pattern="*.discord-invite"/>',
    "  </mime-type>",
    "</mime-info>",
    "",
  ].join("\n");
}

/**
 * Валидирует XML-манифест XDG MIME-типов.
 * @param {string} content
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateMimeXml(content) {
  /** @type {string[]} */
  const errors = [];
  if (
    !content.includes(
      'xmlns="http://www.freedesktop.org/standards/shared-mime-info"',
    )
  ) {
    errors.push("Отсутствует пространство имён shared-mime-info");
  }
  if (!content.includes('<mime-type type="x-scheme-handler/discord">')) {
    errors.push("В XML не зарегистрирован тип x-scheme-handler/discord");
  }
  if (
    !content.includes('<mime-type type="application/x-discord-gpui-invite">')
  ) {
    errors.push(
      "В XML не зарегистрирован тип application/x-discord-gpui-invite",
    );
  }
  if (!content.includes('<glob pattern="*.discord-invite"/>')) {
    errors.push("В XML отсутствует glob pattern *.discord-invite");
  }
  return { valid: errors.length === 0, errors };
}

/**
 * Валидирует maintainer-скрипты `DEBIAN/postinst` и `DEBIAN/postrm`.
 * @param {string} postinst
 * @param {string} postrm
 * @returns {{ valid: boolean, errors: string[] }}
 */
export function validateMaintainerScripts(postinst, postrm) {
  /** @type {string[]} */
  const errors = [];
  if (!postinst.startsWith("#!/bin/sh") && !postinst.startsWith("#!/usr/bin/env sh")) {
    errors.push("postinst должен начинаться с #!/bin/sh");
  }
  if (!postinst.includes("set -e")) {
    errors.push("postinst должен содержать 'set -e' согласно Debian Policy §6.1");
  }
  if (!postinst.includes("update-desktop-database")) {
    errors.push("postinst должен обновлять базу desktop-файлов (update-desktop-database)");
  }
  if (!postinst.includes("update-mime-database")) {
    errors.push("postinst должен обновлять базу XDG mime-типов (update-mime-database)");
  }
  if (
    !postinst.includes(
      "xdg-mime default discord-gpui.desktop x-scheme-handler/discord",
    )
  ) {
    errors.push(
      "postinst должен регистрировать обработчик URI discord:// через xdg-mime",
    );
  }
  if (!postrm.startsWith("#!/bin/sh") && !postrm.startsWith("#!/usr/bin/env sh")) {
    errors.push("postrm должен начинаться с #!/bin/sh");
  }
  if (!postrm.includes("set -e")) {
    errors.push("postrm должен содержать 'set -e'");
  }
  return { valid: errors.length === 0, errors };
}

/**
 * Рекурсивно собирает список всех файлов внутри директории.
 * @param {string} dir
 * @returns {string[]}
 */
function listAllFiles(dir) {
  /** @type {string[]} */
  const out = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) out.push(...listAllFiles(full));
    else if (entry.isFile()) out.push(full);
  }
  return out.sort();
}

/**
 * Собирает `.deb` пакет (`ar` архив: `debian-binary`, `control.tar.gz`, `data.tar.gz`).
 * @param {{
 *   debRootDir?: string,
 *   binaryPath?: string,
 *   outputDebPath: string,
 *   version?: string,
 *   architecture?: string,
 * }} params
 * @returns {{ status: "ok", outputDebPath: string, installedSizeKb: number, md5Entries: number }}
 */
export function buildDebPackage(params) {
  const debRootDir = params.debRootDir ?? DEFAULT_DEB_ROOT;
  const binaryPath = params.binaryPath ?? DEFAULT_BINARY_PATH;
  const outputDebPath = resolve(params.outputDebPath);
  const version = params.version ?? "1.0.0~beta1-1";
  const architecture = params.architecture ?? "amd64";

  if (!existsSync(debRootDir)) {
    throw new Error(`Каталог шаблона пакета не найден: ${debRootDir}`);
  }
  if (!existsSync(binaryPath)) {
    throw new Error(`Бинарный файл для упаковки не найден: ${binaryPath}`);
  }

  const stageTmp = mkdtempSync(join(tmpdir(), "deb-stage-"));
  try {
    const dataStage = join(stageTmp, "data");
    const controlStage = join(stageTmp, "control");
    mkdirSync(dataStage, { recursive: true });
    mkdirSync(controlStage, { recursive: true });

    // 1. Копируем usr/ дерево из packaging/deb/usr в dataStage/usr
    const srcUsr = join(debRootDir, "usr");
    for (const file of listAllFiles(srcUsr)) {
      const relPath = relative(debRootDir, file);
      const destPath = join(dataStage, relPath);
      mkdirSync(dirname(destPath), { recursive: true });
      copyFileSync(file, destPath);
      chmodSync(destPath, 0o644);
    }

    // 2. Устанавливаем исполняемый файл в usr/bin/discord-gpui (0755)
    const stagedBin = join(dataStage, "usr", "bin", "discord-gpui");
    mkdirSync(dirname(stagedBin), { recursive: true });
    copyFileSync(binaryPath, stagedBin);
    chmodSync(stagedBin, 0o755);

    // 3. Считаем суммарный размер файлов и формируем DEBIAN/md5sums
    const dataFiles = listAllFiles(dataStage);
    let totalBytes = 0;
    /** @type {string[]} */
    const md5Lines = [];
    for (const f of dataFiles) {
      const buf = readFileSync(f);
      totalBytes += buf.byteLength;
      const md5 = createHash("md5").update(buf).digest("hex");
      const rel = relative(dataStage, f).split(sep).join("/");
      md5Lines.push(`${md5}  ${rel}`);
    }
    const installedSizeKb = Math.max(1, Math.ceil(totalBytes / 1024));

    // 4. Генерируем DEBIAN/control, копируем postinst, postrm и записываем md5sums
    const controlText = generateDebianControl({
      packageName: "discord-gpui",
      version,
      architecture,
      installedSizeKb,
    });
    const checkControl = validateDebianControl(controlText);
    if (!checkControl.valid) {
      throw new Error(
        `Сгенерированный DEBIAN/control не прошёл валидацию: ${checkControl.errors.join("; ")}`,
      );
    }

    writeFileSync(join(controlStage, "control"), controlText, { mode: 0o644 });
    writeFileSync(join(controlStage, "md5sums"), `${md5Lines.join("\n")}\n`, {
      mode: 0o644,
    });

    const postinstSrc = join(debRootDir, "DEBIAN", "postinst");
    const postrmSrc = join(debRootDir, "DEBIAN", "postrm");
    copyFileSync(postinstSrc, join(controlStage, "postinst"));
    chmodSync(join(controlStage, "postinst"), 0o755);
    copyFileSync(postrmSrc, join(controlStage, "postrm"));
    chmodSync(join(controlStage, "postrm"), 0o755);

    // 5. Упаковываем control.tar.gz и data.tar.gz с владельцем root:root
    const debianBinaryPath = join(stageTmp, "debian-binary");
    const controlTarPath = join(stageTmp, "control.tar.gz");
    const dataTarPath = join(stageTmp, "data.tar.gz");

    writeFileSync(debianBinaryPath, "2.0\n", "utf8");

    const ctrlTar = spawnSync(
      "tar",
      ["--owner=0", "--group=0", "-czf", controlTarPath, "-C", controlStage, "."],
      { encoding: "utf8" },
    );
    if (ctrlTar.status !== 0) {
      throw new Error(`Ошибка создания control.tar.gz: ${ctrlTar.stderr}`);
    }

    const dataTar = spawnSync(
      "tar",
      ["--owner=0", "--group=0", "-czf", dataTarPath, "-C", dataStage, "."],
      { encoding: "utf8" },
    );
    if (dataTar.status !== 0) {
      throw new Error(`Ошибка создания data.tar.gz: ${dataTar.stderr}`);
    }

    mkdirSync(dirname(outputDebPath), { recursive: true });
    if (existsSync(outputDebPath)) rmSync(outputDebPath, { force: true });

    const arRes = spawnSync(
      "ar",
      ["rcD", outputDebPath, debianBinaryPath, controlTarPath, dataTarPath],
      { encoding: "utf8" },
    );
    if (arRes.status !== 0) {
      throw new Error(`Ошибка упаковки .deb через ar: ${arRes.stderr}`);
    }

    return {
      status: "ok",
      outputDebPath,
      installedSizeKb,
      md5Entries: md5Lines.length,
    };
  } finally {
    rmSync(stageTmp, { recursive: true, force: true });
  }
}

/**
 * Проверяет структуру собранного `.deb` файла.
 * @param {string} debFilePath
 * @returns {{ valid: boolean, debianBinaryVersion: string, members: string[], errors: string[] }}
 */
export function inspectDebArchive(debFilePath) {
  /** @type {string[]} */
  const errors = [];
  if (!existsSync(debFilePath)) {
    return {
      valid: false,
      debianBinaryVersion: "",
      members: [],
      errors: [`Файл не существует: ${debFilePath}`],
    };
  }

  const listOut = spawnSync("ar", ["-t", debFilePath], { encoding: "utf8" });
  if (listOut.status !== 0) {
    return {
      valid: false,
      debianBinaryVersion: "",
      members: [],
      errors: [`Ошибка чтения ar: ${listOut.stderr}`],
    };
  }

  const members = listOut.stdout
    .split(/\r?\n/)
    .map((s) => s.trim())
    .filter(Boolean);

  if (
    members[0] !== "debian-binary" ||
    members[1] !== "control.tar.gz" ||
    members[2] !== "data.tar.gz"
  ) {
    errors.push(`Неверный порядок секций в .deb: ${members.join(", ")}`);
  }

  const verOut = spawnSync("ar", ["-p", debFilePath, "debian-binary"], {
    encoding: "utf8",
  });
  const debianBinaryVersion = verOut.stdout.trim();
  if (debianBinaryVersion !== "2.0") {
    errors.push(`Неверная версия debian-binary: "${debianBinaryVersion}"`);
  }

  return {
    valid: errors.length === 0,
    debianBinaryVersion,
    members,
    errors,
  };
}

/**
 * CLI-точка входа для проверки и сборки пакета Debian (`TASK-963`).
 * Коды выхода: 0 — успех, 1 — ошибка аргументов/валидации, 2 — среда/файлы недоступны.
 * @param {readonly string[]} argv
 * @returns {number}
 */
export function runCli(argv) {
  let debRoot = DEFAULT_DEB_ROOT;
  let outJson = "";
  let buildOut = "";
  let verifyMode = false;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--verify") {
      verifyMode = true;
    } else if (arg === "--deb-root") {
      const next = argv[i + 1];
      if (!next) {
        console.error("Ошибка: флаг --deb-root требует путь к каталогу.");
        return 1;
      }
      debRoot = resolve(next);
      i += 1;
    } else if (arg === "--out-json") {
      const next = argv[i + 1];
      if (!next) {
        console.error("Ошибка: флаг --out-json требует путь к файлу.");
        return 1;
      }
      outJson = resolve(next);
      i += 1;
    } else if (arg === "--build") {
      const next = argv[i + 1];
      if (!next) {
        console.error("Ошибка: флаг --build требует путь к выходному .deb файлу.");
        return 1;
      }
      buildOut = resolve(next);
      i += 1;
    } else if (arg === "--help" || arg === "-h") {
      console.log(
        "Использование: node packaging/deb/deb-package.mjs [--verify] [--deb-root DIR] [--out-json FILE] [--build OUT.deb]",
      );
      return 0;
    } else {
      console.error(`Неизвестный аргумент: ${arg}`);
      return 1;
    }
  }

  if (!existsSync(debRoot)) {
    console.error(`Среда недоступна: каталог ${debRoot} не существует.`);
    return 2;
  }

  const controlPath = join(debRoot, "DEBIAN", "control");
  const postinstPath = join(debRoot, "DEBIAN", "postinst");
  const postrmPath = join(debRoot, "DEBIAN", "postrm");
  const desktopPath = join(
    debRoot,
    "usr",
    "share",
    "applications",
    "discord-gpui.desktop",
  );
  const mimeXmlPath = join(
    debRoot,
    "usr",
    "share",
    "mime",
    "packages",
    "discord-gpui.xml",
  );

  for (const p of [controlPath, postinstPath, postrmPath, desktopPath, mimeXmlPath]) {
    if (!existsSync(p)) {
      console.error(`Отсутствует обязательный файл пакета: ${p}`);
      return 2;
    }
  }

  const controlCheck = validateDebianControl(readFileSync(controlPath, "utf8"));
  const desktopCheck = validateDesktopEntry(readFileSync(desktopPath, "utf8"));
  const mimeCheck = validateMimeXml(readFileSync(mimeXmlPath, "utf8"));
  const scriptsCheck = validateMaintainerScripts(
    readFileSync(postinstPath, "utf8"),
    readFileSync(postrmPath, "utf8"),
  );

  const allPassed =
    controlCheck.valid &&
    desktopCheck.valid &&
    mimeCheck.valid &&
    scriptsCheck.valid;

  if (buildOut) {
    buildDebPackage({
      debRootDir: debRoot,
      outputDebPath: buildOut,
    });
  }

  const report = {
    taskId: "TASK-963",
    status: allPassed ? "passed" : "failed",
    verifyMode,
    control: { valid: controlCheck.valid, errors: controlCheck.errors },
    desktop: { valid: desktopCheck.valid, errors: desktopCheck.errors },
    mimeXml: { valid: mimeCheck.valid, errors: mimeCheck.errors },
    scripts: { valid: scriptsCheck.valid, errors: scriptsCheck.errors },
  };

  if (outJson) {
    mkdirSync(dirname(outJson), { recursive: true });
    writeFileSync(outJson, `${JSON.stringify(report, null, 2)}\n`, "utf8");
  }

  return allPassed ? 0 : 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exit(runCli(process.argv.slice(2)));
}
