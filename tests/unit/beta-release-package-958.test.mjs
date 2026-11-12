// @ts-check
/**
 * Контрактные и модульные тесты релизного пакета бета-версии (TASK-958):
 * - SUB-958-1: Сборка релизного архива rust/dist/discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz
 *   (stripped бинарник discord-gpui + лицензионные файлы LICENSE-MIT, LICENSE-APACHE, sbom.spdx.json)
 *   и верификация контрольных сумм в rust/dist/SHA256SUMS независимым оракулом sha256sum -c.
 * - SUB-958-2: Руководство бета-тестировщика docs/BETA-TESTING-GUIDE.md
 *   (системные требования, запуск в 1 команду ./discord-gpui, логи RUST_LOG=info ./discord-gpui,
 *   шаблон репорта об ошибке с версией Linux, GPU, логами и маскированием токенов).
 * - SUB-958-3: Выпуск Release Notes для Beta 1 в RELEASE-NOTES-v1.0.0-beta.1.md
 *   со сводкой всех реализованных возможностей нативного клиента discord-gpui.
 */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const DIST_DIR = join(ROOT, "rust", "dist");
const TARBALL_NAME = "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz";
const TARBALL_PATH = join(DIST_DIR, TARBALL_NAME);
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");
const BIN_PATH = join(DIST_DIR, "discord-gpui");
const SBOM_PATH = join(DIST_DIR, "sbom.spdx.json");
const BETA_GUIDE_PATH = join(ROOT, "docs", "BETA-TESTING-GUIDE.md");
const RELEASE_NOTES_PATH = join(ROOT, "RELEASE-NOTES-v1.0.0-beta.1.md");

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

describe("TASK-958: Релизный пакет бета-версии, контрольные суммы SHA256SUMS и руководство тестировщика", () => {
  it("SUB-958-1 (Шаг 1.1): архив discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz содержит stripped бинарник и лицензионные файлы", () => {
    assert.ok(
      existsSync(TARBALL_PATH),
      `Релизный архив должен существовать: ${TARBALL_PATH}`,
    );

    const st = statSync(TARBALL_PATH);
    assert.ok(
      st.size > 5 * 1024 * 1024,
      `Размер архива (${st.size} байт) должен быть > 5 МБ (содержит нативный бинарник discord-gpui)`,
    );

    // Проверка списка файлов внутри архива через tar -tzf
    const listOut = spawnSync("tar", ["-tzf", TARBALL_PATH], {
      cwd: DIST_DIR,
      encoding: "utf8",
    });
    assert.equal(listOut.status, 0, `Ошибка чтения tar.gz: ${listOut.stderr}`);

    const entries = listOut.stdout
      .split(/\r?\n/)
      .map((s) => s.trim())
      .filter(Boolean);

    const hasEntry = (/** @type {string} */ suffix) =>
      entries.some((e) => e === suffix || e.endsWith(`/${suffix}`));

    assert.ok(hasEntry("discord-gpui"), "В архиве должен присутствовать бинарник discord-gpui");
    assert.ok(hasEntry("LICENSE-MIT"), "В архиве должен присутствовать лицензионный файл LICENSE-MIT");
    assert.ok(hasEntry("LICENSE-APACHE"), "В архиве должен присутствовать лицензионный файл LICENSE-APACHE");
    assert.ok(hasEntry("sbom.spdx.json"), "В архиве должен присутствовать манифест лицензий sbom.spdx.json");
    assert.ok(
      hasEntry("BETA-TESTING-GUIDE.md"),
      "В архиве должно присутствовать руководство BETA-TESTING-GUIDE.md",
    );
    assert.ok(
      hasEntry("RELEASE-NOTES-v1.0.0-beta.1.md"),
      "В архиве должны присутствовать релизные заметки RELEASE-NOTES-v1.0.0-beta.1.md",
    );

    // Распаковка во временную директорию и побайтовая сверка SHA-256 бинарника с эталоном rust/dist/discord-gpui
    const tmp = mkdtempSync(join(tmpdir(), "beta-pkg-958-"));
    try {
      const extractOut = spawnSync("tar", ["-xzf", TARBALL_PATH, "-C", tmp], {
        encoding: "utf8",
      });
      assert.equal(extractOut.status, 0, `Ошибка распаковки tar.gz: ${extractOut.stderr}`);

      const extractedBin = entries.find((e) => e === "discord-gpui" || e.endsWith("/discord-gpui"));
      assert.ok(extractedBin, "Путь к discord-gpui внутри архива должен определяться");
      const extractedBinPath = join(tmp, extractedBin);
      assert.ok(existsSync(extractedBinPath), "Распакованный бинарник должен существовать");

      // Проверка побайтового совпадения с эталоном rust/dist/discord-gpui
      const expectedBinSha = sha256File(BIN_PATH);
      const actualBinSha = sha256File(extractedBinPath);
      assert.equal(
        actualBinSha,
        expectedBinSha,
        "SHA-256 бинарника внутри tar.gz должен совпадать с эталонным rust/dist/discord-gpui",
      );

      // Проверка stripped статуса ELF и smoke-запуска из распакованного пакета
      const fileOut = spawnSync("file", [extractedBinPath], { encoding: "utf8" });
      assert.equal(fileOut.status, 0);
      assert.match(fileOut.stdout, /ELF 64-bit LSB/);
      assert.match(fileOut.stdout, /stripped/);
      assert.doesNotMatch(fileOut.stdout, /not stripped/);

      const smokeOut = spawnSync(extractedBinPath, ["--headless-smoke"], {
        env: { ...process.env, WAYLAND_DISPLAY: "wayland-1", DISPLAY: ":0" },
        encoding: "utf8",
      });
      assert.equal(smokeOut.status, 0, `Smoke-прогон из архива упал: ${smokeOut.stderr}`);
      assert.match(smokeOut.stdout, /GPUI_SMOKE_OK /);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-958-1 (Шаг 1.2): rust/dist/SHA256SUMS содержит актуальный хеш архива и проходит независимый оракул sha256sum -c", () => {
    assert.ok(existsSync(SHA256SUMS_PATH), "Файл rust/dist/SHA256SUMS должен существовать");

    const shaContent = readFileSync(SHA256SUMS_PATH, "utf8");
    const sums = parseSha256Sums(shaContent);

    // Обязательные артефакты в SHA256SUMS
    assert.ok(
      sums.has(TARBALL_NAME),
      `В rust/dist/SHA256SUMS должна присутствовать запись для ${TARBALL_NAME}`,
    );
    assert.ok(
      sums.has("discord-gpui"),
      "В rust/dist/SHA256SUMS должна присутствовать запись для discord-gpui",
    );
    assert.ok(
      sums.has("sbom.spdx.json"),
      "В rust/dist/SHA256SUMS должна присутствовать запись для sbom.spdx.json",
    );

    // Сверка SHA-256 архива и бинарника через node:crypto
    assert.equal(
      sums.get(TARBALL_NAME),
      sha256File(TARBALL_PATH),
      `Хеш ${TARBALL_NAME} в SHA256SUMS не совпадает с фактическим файлом`,
    );
    assert.equal(
      sums.get("discord-gpui"),
      sha256File(BIN_PATH),
      "Хеш discord-gpui в SHA256SUMS не совпадает с фактическим файлом",
    );
    assert.equal(
      sums.get("sbom.spdx.json"),
      sha256File(SBOM_PATH),
      "Хеш sbom.spdx.json в SHA256SUMS не совпадает с фактическим файлом",
    );

    // Независимый системный оракул: sha256sum -c SHA256SUMS
    const shaCheck = spawnSync("sha256sum", ["-c", "SHA256SUMS"], {
      cwd: DIST_DIR,
      encoding: "utf8",
    });
    assert.equal(
      shaCheck.status,
      0,
      `sha256sum -c SHA256SUMS завершился с ошибкой:\nstdout: ${shaCheck.stdout}\nstderr: ${shaCheck.stderr}`,
    );
  });

  it("SUB-958-2: руководство docs/BETA-TESTING-GUIDE.md опубликовано и содержит все 4 обязательных раздела", () => {
    assert.ok(
      existsSync(BETA_GUIDE_PATH),
      `Руководство тестировщика должно существовать: ${BETA_GUIDE_PATH}`,
    );

    const guide = readFileSync(BETA_GUIDE_PATH, "utf8");
    assert.ok(guide.length >= 2500, "Руководство BETA-TESTING-GUIDE.md должно быть подробным (>= 2500 символов)");

    // 1. Системные требования и поддерживаемые дистрибутивы
    assert.match(guide, /GLIBC_?2\.35|glibc\s*2\.35/i, "Должно быть указано требование GLIBC 2.35+");
    assert.match(guide, /Ubuntu 22\.04|Debian 12|Fedora|Arch/i, "Должны быть перечислены поддерживаемые дистрибутивы");
    assert.match(guide, /GNOME[\s\S]*KDE[\s\S]*Hyprland[\s\S]*X11/i, "Должна быть описана матрица 4 окружений Linux Desktop");
    assert.match(guide, /llvmpipe/i, "Должен быть описан fallback на софтверный рендеринг llvmpipe");

    // 2. Быстрый старт: проверка SHA256SUMS и запуск в 1 команду (./discord-gpui)
    assert.match(guide, /discord-gpui-v1\.0\.0-beta\.1-linux-x86_64\.tar\.gz/, "Должно быть указано точное имя релизного архива");
    assert.match(guide, /sha256sum -c (?:rust\/dist\/)?SHA256SUMS/, "Должна быть приведена команда проверки контрольных сумм");
    assert.match(guide, /\.\/discord-gpui\b/, "Должна быть приведена команда быстрого запуска ./discord-gpui");
    assert.match(guide, /--headless-smoke/, "Должна быть описана быстрая самодиагностика --headless-smoke");

    // 3. Инструкция по включению отладочных логов
    assert.match(
      guide,
      /RUST_LOG=info\s+\.\/discord-gpui/,
      "Должна быть приведена точная команда включения логов RUST_LOG=info ./discord-gpui",
    );
    assert.match(guide, /mfa\.\*+|mfa\.\[REDACTED\]/i, "Должно быть описано автоматическое маскирование токенов в логах");

    // 4. Шаблон репорта о найденной ошибке (версия Linux, GPU, логи)
    assert.match(guide, /Шаблон\s+(репорта|отчёта|баг-репорта)|Bug Report Template/i, "Должен присутствовать раздел с шаблоном баг-репорта");
    assert.match(guide, /GPU|видеокарт|драйвер/i, "В шаблоне баг-репорта должно быть поле GPU / драйвера");
    assert.match(guide, /gdb -batch -ex "thread apply all bt full"/, "В шаблоне должна быть команда снятия GDB backtrace");
    assert.match(guide, /P0[\s\S]*P1[\s\S]*P2[\s\S]*P3/, "Должна быть описана классификация критичности дефектов P0–P3");
  });

  it("SUB-958-3: релизные заметки RELEASE-NOTES-v1.0.0-beta.1.md опубликованы и содержат полную сводку возможностей Beta 1", () => {
    assert.ok(
      existsSync(RELEASE_NOTES_PATH),
      `Файл релизных заметок должен существовать: ${RELEASE_NOTES_PATH}`,
    );

    const notes = readFileSync(RELEASE_NOTES_PATH, "utf8");
    assert.ok(notes.length >= 3000, "Файл RELEASE-NOTES-v1.0.0-beta.1.md должен содержать развёрнутую сводку (>= 3000 символов)");

    // Проверка ключевых метаданных и возможностей v1.0.0-beta.1
    assert.match(notes, /v1\.0\.0-beta\.1/, "Должна быть указана версия v1.0.0-beta.1");
    assert.match(notes, /discord-gpui-v1\.0\.0-beta\.1-linux-x86_64\.tar\.gz/, "Должен быть указан релизный архив Beta 1");
    assert.match(notes, /d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e/, "Должен быть зафиксирован SHA-256 бинарника discord-gpui");
    assert.match(notes, /120\s*FPS/i, "Должен быть отражён рендеринг 120 FPS на GPUI / WGPU");
    assert.match(notes, /DAVE\s+MLS|RFC\s*9420/i, "Должно быть описано сквозное шифрование голоса DAVE MLS v1");
    assert.match(notes, /PipeWire|ScreenCast/i, "Должна быть описана демонстрация экрана через PipeWire Portal и X11 fallback");
    assert.match(notes, /AT-SPI|org\.a11y\.Bus/i, "Должна быть описана поддержка доступности AT-SPI и WCAG 2.2");
    assert.match(notes, /SQLite\s+FTS5|WAL/i, "Должно быть описано локальное хранилище SQLite WAL / FTS5");
    assert.match(notes, /5\/5\s*(мутац|mutation)/i, "Должна быть отражена верификация 5/5 мутационного гейта");
  });
});
