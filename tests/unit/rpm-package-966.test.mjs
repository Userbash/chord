// @ts-check
/**
 * Контрактные и модульные тесты спецификации RPM-пакета для Fedora/RHEL/openSUSE (TASK-966):
 * - Проверка структуры и метаданных `packaging/rpm/discord-gpui.spec` (Name, Version, Release,
 *   License, ExclusiveArch, Source0, совпадение SHA-256 с оракулом `rust/dist/SHA256SUMS`).
 * - Проверка обязательных зависимостей `pipewire-libs`, `libsecret`, `wayland`, `vulkan-loader`
 *   и условных блоков совместимости Fedora / RHEL (`%if 0%{?fedora} || 0%{?rhel}`) и openSUSE (`%if 0%{?suse_version}`).
 * - Реальное исполнение фаз `%build`, `%install` и `%check` из `discord-gpui.spec` во временном
 *   `BUILDROOT`, проверка `.desktop`-файла системным оракулом `desktop-file-validate`
 *   (схемы `x-scheme-handler/discord;x-scheme-handler/disclip;`, `StartupWMClass=discord-gpui`),
 *   проверка иконок (`hicolor/scalable/apps/discord-gpui.svg`, `hicolor/256x256/apps/discord-gpui.png`)
 *   системным оракулом `file` и прогон `--headless-smoke`.
 * - Проверка безопасности прав доступа (`0755` для бинарника, `0644` для `.desktop` и иконок),
 *   отсутствия сетевых вызовов в фазах сборки и прохождения мутационного гейта 5/5.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const SPEC_PATH = join(ROOT, "packaging", "rpm", "discord-gpui.spec");
const DIST_DIR = join(ROOT, "rust", "dist");
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");
const BIN_PATH = join(DIST_DIR, "discord-gpui");
const TARBALL_NAME = "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz";
const TARBALL_PATH = join(DIST_DIR, TARBALL_NAME);

const REQUIRED_RPM_DEPENDENCIES = [
  "pipewire-libs",
  "libsecret",
  "wayland",
  "vulkan-loader",
  "alsa-lib",
  "libxcb",
  "libxkbcommon",
  "libxkbcommon-x11",
  "xdg-desktop-portal",
  "hicolor-icon-theme",
  "desktop-file-utils",
];

const REQUIRED_SPEC_SECTIONS = [
  "%description",
  "%prep",
  "%build",
  "%install",
  "%check",
  "%post",
  "%postun",
  "%files",
  "%changelog",
];

/**
 * Вычисляет SHA-256 дайджест файла.
 * @param {string} filePath
 * @returns {string}
 */
function sha256File(filePath) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
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
 * Извлекает тело секции (`%build`, `%install`, `%check`, ...) из текста RPM `.spec`.
 * @param {string} specContent
 * @param {string} sectionName
 * @returns {string}
 */
function extractSpecSection(specContent, sectionName) {
  const lines = specContent.split(/\r?\n/);
  const startIdx = lines.findIndex((l) => l.trim() === sectionName);
  if (startIdx === -1) {
    throw new Error(`Секция ${sectionName} не найдена в спецификации RPM`);
  }
  /** @type {string[]} */
  const body = [];
  for (let i = startIdx + 1; i < lines.length; i += 1) {
    const line = lines[i] ?? "";
    if (/^%(description|prep|build|install|check|post|postun|files|changelog)\b/.test(line.trim())) {
      break;
    }
    body.push(line);
  }
  return body.join("\n").trim();
}

/**
 * Извлекает встроенный `.desktop` блок из текста `discord-gpui.spec`.
 * @param {string} specContent
 * @returns {string}
 */
function extractDesktopEntryFromSpec(specContent) {
  const match =
    /cat\s*<<\s*'EOF'\s*>\s*[^\n]*discord-gpui\.desktop\r?\n([\s\S]*?)\r?\nEOF\b/.exec(
      specContent,
    );
  if (!match?.[1]) {
    throw new Error("Не найден блок генерации discord-gpui.desktop (cat << 'EOF' > ... EOF)");
  }
  return `${match[1].trim()}\n`;
}

/**
 * Извлекает встроенную SVG-иконку из текста `discord-gpui.spec`.
 * @param {string} specContent
 * @returns {string}
 */
function extractSvgIconFromSpec(specContent) {
  const match =
    /cat\s*<<\s*'EOF'\s*>\s*[^\n]*discord-gpui\.svg\r?\n([\s\S]*?)\r?\nEOF\b/.exec(
      specContent,
    );
  if (!match?.[1]) {
    throw new Error("Не найден блок генерации SVG-иконки discord-gpui.svg (cat << 'EOF' > ... EOF)");
  }
  return `${match[1].trim()}\n`;
}

/**
 * Парсит и валидирует RPM `.spec` по контракту TASK-966.
 * @param {string} specContent
 * @param {{ expectedBinSha256: string, expectedTarballSha256: string }} oracle
 * @returns {{ passed: boolean, errors: string[], tags: Map<string, string>, requires: string[], provides: string[] }}
 */
function validateRpmSpecContract(specContent, oracle) {
  /** @type {string[]} */
  const errors = [];
  /** @type {Map<string, string>} */
  const tags = new Map();
  /** @type {string[]} */
  const requires = [];
  /** @type {string[]} */
  const provides = [];

  if (!specContent || !specContent.trim()) {
    return { passed: false, errors: ["Пустой файл спецификации RPM"], tags, requires, provides };
  }

  for (const rawLine of specContent.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const tagMatch = /^([A-Za-z0-9_]+)\s*:\s*(.+)$/.exec(line);
    if (!tagMatch) continue;
    const key = tagMatch[1] ?? "";
    const val = (tagMatch[2] ?? "").trim();
    if (!tags.has(key)) {
      tags.set(key, val);
    }
    if (key === "Requires") {
      for (const token of val.split(/,/)) {
        const pkg = token.trim().split(/\s+/)[0];
        if (pkg) requires.push(pkg);
      }
    }
    if (key === "Provides") {
      provides.push(val);
    }
  }

  // 1. Обязательные теги заголовка RPM
  if (tags.get("Name") !== "discord-gpui") {
    errors.push(`Ожидался Name: discord-gpui, получено: ${tags.get("Name") ?? "<missing>"}`);
  }
  if (!tags.get("Version") || !/^1\.0\.0/.test(tags.get("Version") ?? "")) {
    errors.push(`Некорректный Version: ${tags.get("Version") ?? "<missing>"}`);
  }
  if (!tags.get("Release") || !/%\{\?dist\}/.test(tags.get("Release") ?? "")) {
    errors.push("Поле Release должно содержать макрос дистрибутива %{?dist}");
  }
  if (tags.get("License") !== "MIT OR Apache-2.0") {
    errors.push(`Ожидалась лицензия SPDX "MIT OR Apache-2.0", получено: ${tags.get("License") ?? "<missing>"}`);
  }
  if (tags.get("ExclusiveArch") !== "x86_64") {
    errors.push(`Ожидался ExclusiveArch: x86_64, получено: ${tags.get("ExclusiveArch") ?? "<missing>"}`);
  }
  if (!tags.get("Source0") || !tags.get("Source0")?.includes("discord-gpui-v")) {
    errors.push("Поле Source0 должно ссылаться на релизный архив discord-gpui");
  }

  // 2. Сверка контрольных сумм с независимым оракулом rust/dist/SHA256SUMS
  const binHashMatch = /%global\s+bin_sha256\s+([a-f0-9]{64})/.exec(specContent);
  if (!binHashMatch || binHashMatch[1] !== oracle.expectedBinSha256) {
    errors.push(
      `Макрос %global bin_sha256 (${binHashMatch?.[1] ?? "missing"}) не совпадает с оракулом SHA256SUMS (${oracle.expectedBinSha256})`,
    );
  }
  const tarballHashMatch = /%global\s+tarball_sha256\s+([a-f0-9]{64})/.exec(specContent);
  if (!tarballHashMatch || tarballHashMatch[1] !== oracle.expectedTarballSha256) {
    errors.push(
      `Макрос %global tarball_sha256 (${tarballHashMatch?.[1] ?? "missing"}) не совпадает с оракулом SHA256SUMS (${oracle.expectedTarballSha256})`,
    );
  }

  // 3. Проверка обязательных зависимостей (pipewire-libs, libsecret, wayland, vulkan-loader и др.)
  for (const dep of REQUIRED_RPM_DEPENDENCIES) {
    if (!requires.includes(dep)) {
      errors.push(`Отсутствует обязательная зависимость Requires: ${dep}`);
    }
  }

  // 4. Проверка поддержки Fedora/RHEL и openSUSE
  if (!/%if\s+0%\{\?fedora\}\s*\|\|\s*0%\{\?rhel\}/.test(specContent)) {
    errors.push("Отсутствует условный блок для Fedora/RHEL (%if 0%{?fedora} || 0%{?rhel})");
  }
  if (!/%if\s+0%\{\?suse_version\}/.test(specContent)) {
    errors.push("Отсутствует условный блок для openSUSE (%if 0%{?suse_version})");
  }
  for (const suseDep of [
    "libpipewire-0_3-0",
    "libsecret-1-0",
    "libwayland-client0",
    "libvulkan1",
  ]) {
    if (!specContent.includes(suseDep)) {
      errors.push(`Для совместимости с openSUSE должна быть указана библиотека ${suseDep}`);
    }
  }

  // 5. Проверка обязательных секций RPM .spec
  for (const section of REQUIRED_SPEC_SECTIONS) {
    const sectionRegex = new RegExp(`^${section}\\b`, "m");
    if (!sectionRegex.test(specContent)) {
      errors.push(`Отсутствует обязательная секция ${section}`);
    }
  }

  // 6. Проверка установки бинарника, .desktop файла и иконок в %install и %files
  if (!/install\s+-D\s+-m\s+0?755\s+[^\n]*%\{buildroot\}%\{_bindir\}\/discord-gpui/.test(specContent)) {
    errors.push("В %install должна выполняться установка бинарника с правами 0755 в %{buildroot}%{_bindir}/discord-gpui");
  }
  if (!/%\{_bindir\}\/discord-gpui/.test(specContent)) {
    errors.push("В %files должен быть зарегистрирован путь %{_bindir}/discord-gpui");
  }
  if (!/%\{_datadir\}\/applications\/discord-gpui\.desktop/.test(specContent)) {
    errors.push("В %install / %files должен присутствовать desktop-файл %{_datadir}/applications/discord-gpui.desktop");
  }
  if (!/%\{_datadir\}\/icons\/hicolor\/scalable\/apps\/discord-gpui\.svg/.test(specContent)) {
    errors.push("Должна устанавливаться векторная иконка %{_datadir}/icons/hicolor/scalable/apps/discord-gpui.svg");
  }
  if (!/%\{_datadir\}\/icons\/hicolor\/256x256\/apps\/discord-gpui\.png/.test(specContent)) {
    errors.push("Должна устанавливаться растровая иконка %{_datadir}/icons/hicolor/256x256/apps/discord-gpui.png");
  }

  // 7. Проверка содержимого .desktop-файла и схем URI
  try {
    const desktopText = extractDesktopEntryFromSpec(specContent);
    if (!desktopText.startsWith("[Desktop Entry]\n")) {
      errors.push("Desktop-файл должен начинаться с заголовка [Desktop Entry]");
    }
    if (!/^Type=Application$/m.test(desktopText)) {
      errors.push("Desktop-файл должен содержать Type=Application");
    }
    if (!/^Exec=discord-gpui\s+%u$/m.test(desktopText)) {
      errors.push("Desktop-файл должен содержать Exec=discord-gpui %u для передачи deep-link URI");
    }
    if (!/^Icon=discord-gpui$/m.test(desktopText)) {
      errors.push("Desktop-файл должен содержать Icon=discord-gpui");
    }
    if (!/^StartupWMClass=discord-gpui$/m.test(desktopText)) {
      errors.push("Desktop-файл должен содержать StartupWMClass=discord-gpui");
    }
    if (
      !/^MimeType=.*x-scheme-handler\/discord;.*x-scheme-handler\/disclip;/m.test(
        desktopText,
      )
    ) {
      errors.push(
        "Desktop-файл должен регистрировать схемы MimeType=x-scheme-handler/discord;x-scheme-handler/disclip;",
      );
    }
  } catch (err) {
    errors.push(err instanceof Error ? err.message : String(err));
  }

  // 8. Проверка безопасности: отсутствие небезопасных прав 0777 и сетевых вызовов
  if (/\b(0?777|chmod\s+777)\b/.test(specContent)) {
    errors.push("Обнаружены небезопасные права доступа 777 в спецификации RPM");
  }
  if (/\b(curl|wget)\s+https?:\/\//.test(specContent)) {
    errors.push("Запрещены прямые сетевые вызовы curl/wget внутри фаз сборки RPM");
  }

  return {
    passed: errors.length === 0,
    errors,
    tags,
    requires,
    provides,
  };
}

describe("TASK-966: Спецификация RPM пакета packaging/rpm/discord-gpui.spec (Fedora/RHEL/openSUSE) и desktop-интеграция", () => {
  it("Шаг 1: файл packaging/rpm/discord-gpui.spec существует, валиден и синхронизирован с оракулом rust/dist/SHA256SUMS", () => {
    assert.ok(
      existsSync(SPEC_PATH),
      `Файл спецификации RPM должен существовать: ${SPEC_PATH}`,
    );
    assert.ok(
      existsSync(SHA256SUMS_PATH),
      `Эталонный файл контрольных сумм должен существовать: ${SHA256SUMS_PATH}`,
    );

    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const expectedBinSha256 = sums.get("discord-gpui") ?? "";
    const expectedTarballSha256 = sums.get(TARBALL_NAME) ?? "";
    assert.equal(expectedBinSha256.length, 64, "В SHA256SUMS должен быть 64-символьный хеш discord-gpui");
    assert.equal(expectedTarballSha256.length, 64, `В SHA256SUMS должен быть 64-символьный хеш ${TARBALL_NAME}`);

    // Сверка с фактическими файлами на диске через независимый крипто-оракул
    assert.equal(sha256File(BIN_PATH), expectedBinSha256);
    assert.equal(sha256File(TARBALL_PATH), expectedTarballSha256);

    const specContent = readFileSync(SPEC_PATH, "utf8");
    const result = validateRpmSpecContract(specContent, {
      expectedBinSha256,
      expectedTarballSha256,
    });

    assert.equal(
      result.passed,
      true,
      `Ошибки валидации packaging/rpm/discord-gpui.spec:\n- ${result.errors.join("\n- ")}`,
    );
  });

  it("Шаг 2: зависимости pipewire-libs, libsecret, wayland, vulkan-loader и профили Fedora/RHEL/openSUSE заданы корректно", () => {
    const specContent = readFileSync(SPEC_PATH, "utf8");
    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const result = validateRpmSpecContract(specContent, {
      expectedBinSha256: sums.get("discord-gpui") ?? "",
      expectedTarballSha256: sums.get(TARBALL_NAME) ?? "",
    });

    for (const coreDep of [
      "pipewire-libs",
      "libsecret",
      "wayland",
      "vulkan-loader",
    ]) {
      assert.ok(
        result.requires.includes(coreDep),
        `В списке Requires должна присутствовать зависимость ${coreDep}`,
      );
    }

    // Проверка MIME-обработчиков в Provides
    assert.ok(
      result.provides.some((p) => p.includes("mimehandler(x-scheme-handler/discord)")),
      "В Provides должен быть объявлен mimehandler(x-scheme-handler/discord)",
    );
    assert.ok(
      result.provides.some((p) => p.includes("mimehandler(x-scheme-handler/disclip)")),
      "В Provides должен быть объявлен mimehandler(x-scheme-handler/disclip)",
    );

    // Проверка условных профилей Fedora/RHEL и openSUSE
    assert.match(specContent, /%if\s+0%\{\?fedora\}\s*\|\|\s*0%\{\?rhel\}/);
    assert.match(specContent, /%if\s+0%\{\?suse_version\}/);
    assert.match(specContent, /Requires:\s+pipewire-libs\s*>=\s*0\.3\.65/);
    assert.match(specContent, /Requires:\s+libpipewire-0_3-0/);
  });

  it("Шаг 3: реальное выполнение %build, %install и %check из spec-файла, верификация desktop-file-validate, иконок 256x256 и --headless-smoke", () => {
    const specContent = readFileSync(SPEC_PATH, "utf8");
    const svgText = extractSvgIconFromSpec(specContent);
    const buildScript = extractSpecSection(specContent, "%build");
    const installScript = extractSpecSection(specContent, "%install");
    const checkScript = extractSpecSection(specContent, "%check");

    const tmpRoot = mkdtempSync(join(tmpdir(), "rpm-stage-966-"));
    try {
      const buildDir = join(tmpRoot, "BUILD");
      const buildRootDir = join(tmpRoot, "BUILDROOT");
      mkdirSync(buildDir, { recursive: true });
      mkdirSync(buildRootDir, { recursive: true });

      // Помещаем эталонный бинарник в каталог BUILD (аналог %prep)
      copyFileSync(BIN_PATH, join(buildDir, "discord-gpui"));

      /**
       * Раскрывает базовые RPM-макросы для запуска секций %build / %install / %check.
       * @param {string} script
       * @returns {string}
       */
      const expandRpmMacros = (script) =>
        script
          .replaceAll("%{buildroot}", buildRootDir)
          .replaceAll("%{_bindir}", "/usr/bin")
          .replaceAll("%{_datadir}", "/usr/share");

      // 1. Исполнение секции %build
      const buildRun = spawnSync("bash", ["-euo", "pipefail", "-c", expandRpmMacros(buildScript)], {
        cwd: buildDir,
        encoding: "utf8",
      });
      assert.equal(buildRun.status, 0, `Ошибка выполнения %build: ${buildRun.stderr}`);

      // 2. Исполнение секции %install
      const installRun = spawnSync("bash", ["-euo", "pipefail", "-c", expandRpmMacros(installScript)], {
        cwd: buildDir,
        encoding: "utf8",
      });
      assert.equal(installRun.status, 0, `Ошибка выполнения %install: ${installRun.stderr}`);

      const stagedBinPath = join(buildRootDir, "usr", "bin", "discord-gpui");
      const desktopPath = join(buildRootDir, "usr", "share", "applications", "discord-gpui.desktop");
      const svgPath = join(
        buildRootDir,
        "usr",
        "share",
        "icons",
        "hicolor",
        "scalable",
        "apps",
        "discord-gpui.svg",
      );
      const pngPath = join(
        buildRootDir,
        "usr",
        "share",
        "icons",
        "hicolor",
        "256x256",
        "apps",
        "discord-gpui.png",
      );

      assert.ok(existsSync(stagedBinPath), "Бинарник должен быть установлен в %{_bindir}/discord-gpui");
      assert.ok(existsSync(desktopPath), "Desktop-файл должен быть установлен в %{_datadir}/applications/discord-gpui.desktop");
      assert.ok(existsSync(svgPath), "SVG-иконка должна быть установлена в hicolor/scalable/apps/discord-gpui.svg");
      assert.ok(existsSync(pngPath), "PNG-иконка должна быть установлена в hicolor/256x256/apps/discord-gpui.png");

      // 3. Проверка прав доступа (0755 для бинарника, 0644 для .desktop и иконок)
      assert.equal(statSync(stagedBinPath).mode & 0o777, 0o755);
      assert.equal(statSync(desktopPath).mode & 0o777, 0o644);
      assert.equal(statSync(svgPath).mode & 0o777, 0o644);
      assert.equal(statSync(pngPath).mode & 0o777, 0o644);

      // 4. Независимый системный оракул `file` для PNG-иконки 256x256 и SVG
      assert.match(svgText, /<svg\b[^>]*xmlns="http:\/\/www\.w3\.org\/2000\/svg"/);
      assert.match(svgText, /#5865F2/i, "SVG-иконка должна содержать фирменный цвет Discord Blurple #5865F2");

      const filePngOut = spawnSync("file", [pngPath], { encoding: "utf8" });
      assert.equal(filePngOut.status, 0);
      assert.match(filePngOut.stdout, /PNG image data,\s*256\s*x\s*256/);

      // 5. Исполнение секции %check (системный оракул desktop-file-validate + --headless-smoke)
      const checkRun = spawnSync("bash", ["-euo", "pipefail", "-c", expandRpmMacros(checkScript)], {
        cwd: buildDir,
        env: { ...process.env, WAYLAND_DISPLAY: "wayland-1", DISPLAY: ":0" },
        encoding: "utf8",
      });
      assert.equal(checkRun.status, 0, `Ошибка выполнения %check: ${checkRun.stderr}`);
      const smokeMatch = /^GPUI_SMOKE_OK\s+(\{.+\})$/m.exec(checkRun.stdout);
      assert.ok(smokeMatch?.[1], `Ожидалась строка GPUI_SMOKE_OK {...}, получено: ${checkRun.stdout}`);
      const payload = JSON.parse(smokeMatch[1]);
      assert.equal(payload.status, "ok");
      assert.equal(payload.sqlite_wal_ok, true);
      assert.equal(payload.fps_budget_ok, true);
      assert.ok(payload.vmrss_mb < 60.0, `VmRSS (${payload.vmrss_mb} МБ) должен быть < 60 МБ`);

      // 6. Негативная проверка системного оракула desktop-file-validate на повреждённом .desktop
      const brokenDesktopPath = join(tmpRoot, "broken-discord-gpui.desktop");
      writeFileSync(
        brokenDesktopPath,
        "[Desktop Entry]\nType=InvalidType\nName=Broken\n",
        "utf8",
      );
      const brokenValidate = spawnSync("desktop-file-validate", [brokenDesktopPath], {
        encoding: "utf8",
      });
      assert.notEqual(
        brokenValidate.status,
        0,
        "desktop-file-validate обязан отклонять некорректный .desktop файл",
      );
    } finally {
      rmSync(tmpRoot, { recursive: true, force: true });
    }
  });

  it("Шаг 4: негативные, граничные проверки и мутационный гейт 5/5 (100% kill rate)", () => {
    const specContent = readFileSync(SPEC_PATH, "utf8");
    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const oracle = {
      expectedBinSha256: sums.get("discord-gpui") ?? "",
      expectedTarballSha256: sums.get(TARBALL_NAME) ?? "",
    };

    // Граничный сценарий: пустой вход
    assert.equal(validateRpmSpecContract("", oracle).passed, false);
    assert.equal(validateRpmSpecContract("   \n  ", oracle).passed, false);

    // 5 намеренных мутаций спецификации RPM (5/5 должны быть убиты валидатором контракта)
    const mutations = [
      {
        id: "M1-remove-pipewire-libs",
        mutate: (/** @type {string} */ s) => s.replace(/Requires:\s*pipewire-libs[^\n]*\n/g, ""),
        expectedError: /pipewire-libs/,
      },
      {
        id: "M2-remove-vulkan-loader-and-wayland",
        mutate: (/** @type {string} */ s) =>
          s
            .replace(/Requires:\s*vulkan-loader[^\n]*\n/g, "")
            .replace(/Requires:\s*wayland[^\n]*\n/g, ""),
        expectedError: /wayland|vulkan-loader/,
      },
      {
        id: "M3-corrupt-desktop-mime-scheme",
        mutate: (/** @type {string} */ s) =>
          s.replace("x-scheme-handler/discord;", "x-scheme-handler/broken-scheme;"),
        expectedError: /MimeType=x-scheme-handler\/discord/,
      },
      {
        id: "M4-remove-hicolor-svg-icon",
        mutate: (/** @type {string} */ s) =>
          s.replace(/%\{_datadir\}\/icons\/hicolor\/scalable\/apps\/discord-gpui\.svg/g, ""),
        expectedError: /discord-gpui\.svg/,
      },
      {
        id: "M5-corrupt-sha256-and-permissions",
        mutate: (/** @type {string} */ s) =>
          s
            .replace(oracle.expectedBinSha256, "0".repeat(64))
            .replace("install -D -m 0755", "install -D -m 0777"),
        expectedError: /bin_sha256|0755|777/,
      },
    ];

    let killed = 0;
    for (const m of mutations) {
      const mutatedSpec = m.mutate(specContent);
      assert.notEqual(mutatedSpec, specContent, `Мутация ${m.id} должна изменять текст спецификации`);
      const res = validateRpmSpecContract(mutatedSpec, oracle);
      assert.equal(res.passed, false, `Мутация ${m.id} обязана быть обнаружена и убита`);
      assert.match(
        res.errors.join("; "),
        m.expectedError,
        `Мутация ${m.id} должна фиксировать профильную ошибку`,
      );
      killed += 1;
    }
    assert.equal(killed, 5, "Все 5/5 мутаций должны быть убиты");
  });
});
