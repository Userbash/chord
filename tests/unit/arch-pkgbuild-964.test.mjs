// @ts-check
/**
 * Контрактные и модульные тесты спецификации пакета Arch Linux PKGBUILD и .SRCINFO (TASK-964):
 * - Проверка наличия и структуры `packaging/arch/PKGBUILD` и `packaging/arch/.SRCINFO`.
 * - Проверка обязательных зависимостей (`pipewire`, `libsecret`, `wayland`, `vulkan-icd-loader`,
 *   а также базовых системных библиотек `glibc`, `gcc-libs`, `alsa-lib`, `libxcb`, `libxkbcommon`, `libxkbcommon-x11`).
 * - Сверка контрольных сумм `sha256sums` с независимым оракулом `rust/dist/SHA256SUMS` и реальными SHA-256 дайджестами артефактов.
 * - Положительная и негативная верификация через `makepkg --verifysource` и синхронизация `.SRCINFO` с `makepkg --printsrcinfo`.
 * - Проверка функции `package()` в изолированном `$srcdir`/`$pkgdir` (бинарник, лицензии, SBOM, `.desktop` и схемы `discord://`/`disclip://`).
 */
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
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
const ARCH_DIR = join(ROOT, "packaging", "arch");
const PKGBUILD_PATH = join(ARCH_DIR, "PKGBUILD");
const SRCINFO_PATH = join(ARCH_DIR, ".SRCINFO");
const DIST_DIR = join(ROOT, "rust", "dist");
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");

const REQUIRED_DEPENDS = [
  "pipewire",
  "libsecret",
  "wayland",
  "vulkan-icd-loader",
  "glibc",
  "gcc-libs",
  "alsa-lib",
  "libxcb",
  "libxkbcommon",
  "libxkbcommon-x11",
];

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
 * Парсит `rust/dist/SHA256SUMS` в Map<filename, sha256>.
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
 * Парсит `.SRCINFO` в словарь ключ -> массив значений.
 * @param {string} content
 * @returns {Map<string, string[]>}
 */
function parseSrcInfo(content) {
  /** @type {Map<string, string[]>} */
  const fields = new Map();
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const eqIdx = line.indexOf("=");
    assert.ok(eqIdx > 0, `Некорректная строка в .SRCINFO: "${line}"`);
    const key = line.slice(0, eqIdx).trim();
    const val = line.slice(eqIdx + 1).trim();
    const existing = fields.get(key) ?? [];
    existing.push(val);
    fields.set(key, existing);
  }
  return fields;
}

describe("TASK-964: Спецификация пакета Arch Linux PKGBUILD и .SRCINFO", () => {
  it("Шаг 1: файлы packaging/arch/PKGBUILD и packaging/arch/.SRCINFO существуют и имеют валидную структуру", () => {
    assert.ok(existsSync(PKGBUILD_PATH), `Файл PKGBUILD должен существовать: ${PKGBUILD_PATH}`);
    assert.ok(existsSync(SRCINFO_PATH), `Файл .SRCINFO должен существовать: ${SRCINFO_PATH}`);

    const pkgbuild = readFileSync(PKGBUILD_PATH, "utf8");
    const srcinfo = readFileSync(SRCINFO_PATH, "utf8");

    assert.ok(!pkgbuild.includes("\r"), "PKGBUILD не должен содержать CRLF (\\r)");
    assert.ok(!srcinfo.includes("\r"), ".SRCINFO не должен содержать CRLF (\\r)");

    assert.match(pkgbuild, /^pkgname=discord-gpui$/m, "pkgname должен быть discord-gpui");
    assert.match(pkgbuild, /^pkgver=1\.0\.0_beta\.1$/m, "pkgver должен быть 1.0.0_beta.1 (без дефисов по стандарту Arch)");
    assert.match(pkgbuild, /^pkgrel=1$/m, "pkgrel должен быть 1");
    assert.match(pkgbuild, /^arch=\(['"]?x86_64['"]?\)/m, "arch должен содержать x86_64");
    assert.match(pkgbuild, /license=\(.*MIT.*Apache-2\.0.*\)/s, "license должен указывать MIT и Apache-2.0");
    assert.match(pkgbuild, /^check\(\)\s*\{/m, "PKGBUILD должен определять функцию check()");
    assert.match(pkgbuild, /^package\(\)\s*\{/m, "PKGBUILD должен определять функцию package()");

    const parsedSrc = parseSrcInfo(srcinfo);
    assert.deepEqual(parsedSrc.get("pkgbase"), ["discord-gpui"]);
    assert.deepEqual(parsedSrc.get("pkgname"), ["discord-gpui"]);
    assert.deepEqual(parsedSrc.get("pkgver"), ["1.0.0_beta.1"]);
    assert.deepEqual(parsedSrc.get("pkgrel"), ["1"]);
    assert.deepEqual(parsedSrc.get("arch"), ["x86_64"]);
    assert.deepEqual(parsedSrc.get("license"), ["MIT", "Apache-2.0"]);
  });

  it("Шаг 2: PKGBUILD и .SRCINFO объявляют обязательные зависимости (pipewire, libsecret, wayland, vulkan-icd-loader)", () => {
    const pkgbuild = readFileSync(PKGBUILD_PATH, "utf8");
    const srcinfo = readFileSync(SRCINFO_PATH, "utf8");
    const parsedSrc = parseSrcInfo(srcinfo);

    const srcDepends = parsedSrc.get("depends") ?? [];
    for (const dep of REQUIRED_DEPENDS) {
      assert.ok(
        srcDepends.includes(dep),
        `В .SRCINFO depends должна присутствовать зависимость "${dep}" (фактически: ${srcDepends.join(", ")})`,
      );
      assert.match(
        pkgbuild,
        new RegExp(`['"]${dep.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}['"]`),
        `В PKGBUILD depends должна присутствовать зависимость '${dep}'`,
      );
    }

    const optDepends = parsedSrc.get("optdepends") ?? [];
    assert.ok(
      optDepends.some((entry) => entry.startsWith("xdg-desktop-portal:")),
      "В optdepends должен быть указан xdg-desktop-portal для ScreenCast Wayland",
    );
  });

  it("Шаг 3: контрольные суммы sha256sums в PKGBUILD и .SRCINFO строго соответствуют оракулу rust/dist/SHA256SUMS и реальным файлам", () => {
    assert.ok(existsSync(SHA256SUMS_PATH), "Оракул rust/dist/SHA256SUMS должен существовать");
    const oracleMap = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));

    const pkgbuild = readFileSync(PKGBUILD_PATH, "utf8");
    const srcinfo = readFileSync(SRCINFO_PATH, "utf8");
    const parsedSrc = parseSrcInfo(srcinfo);

    const sources = parsedSrc.get("source") ?? [];
    const sha256sums = parsedSrc.get("sha256sums") ?? [];

    assert.ok(sources.length >= 3, "В source должны быть перечислены релизный архив, бинарник и SBOM");
    assert.equal(
      sources.length,
      sha256sums.length,
      "Количество элементов в source и sha256sums должно совпадать 1-к-1",
    );

    const expectedArtifacts = [
      "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz",
      "discord-gpui",
      "sbom.spdx.json",
    ];

    for (const artifact of expectedArtifacts) {
      const idx = sources.findIndex((s) => s === artifact || s.endsWith(`/${artifact}`) || s.startsWith(`${artifact}::`));
      assert.ok(idx !== -1, `Артефакт ${artifact} должен присутствовать в массиве source`);

      const pkgSum = sha256sums[idx];
      assert.ok(pkgSum && /^[a-f0-9]{64}$/.test(pkgSum), `Хеш для ${artifact} должен быть 64-символьным SHA-256 (не SKIP)`);

      const oracleSum = oracleMap.get(artifact);
      assert.ok(oracleSum, `Артефакт ${artifact} должен присутствовать в rust/dist/SHA256SUMS`);
      assert.equal(
        pkgSum,
        oracleSum,
        `sha256sums для ${artifact} в PKGBUILD/.SRCINFO должен совпадать с rust/dist/SHA256SUMS`,
      );

      const diskPath = join(DIST_DIR, artifact);
      assert.ok(existsSync(diskPath), `Реальный файл артефакта должен существовать: ${diskPath}`);
      assert.equal(
        pkgSum,
        sha256File(diskPath),
        `sha256sums для ${artifact} должен совпадать с фактическим SHA-256 файла на диске`,
      );

      assert.ok(
        pkgbuild.includes(oracleSum),
        `PKGBUILD должен явно содержать SHA-256 хеш ${oracleSum} для ${artifact}`,
      );
    }
  });

  it("Шаг 4: .SRCINFO побайтово синхронизирован с выводом makepkg --printsrcinfo", () => {
    const srcinfoOnDisk = readFileSync(SRCINFO_PATH, "utf8").trim();
    const out = spawnSync("makepkg", ["--printsrcinfo"], {
      cwd: ARCH_DIR,
      encoding: "utf8",
    });
    assert.equal(out.status, 0, `makepkg --printsrcinfo завершился с ошибкой: ${out.stderr}`);
    assert.equal(
      srcinfoOnDisk,
      out.stdout.trim(),
      "Содержимое packaging/arch/.SRCINFO должно в точности совпадать с выводом makepkg --printsrcinfo",
    );
  });

  it("Шаг 5: PKGBUILD проходит верификацию makepkg --verifysource (позитивный сценарий) и отклоняет повреждённый артефакт / невалидный pkgver (негативные сценарии)", () => {
    // 1. Позитивный прогон makepkg --verifysource непосредственно в packaging/arch
    const tmpBuildDir = mkdtempSync(join(tmpdir(), "arch-verify-964-"));
    try {
      const verifyOut = spawnSync("makepkg", ["--verifysource"], {
        cwd: ARCH_DIR,
        env: {
          ...process.env,
          BUILDDIR: tmpBuildDir,
          SRCDEST: DIST_DIR,
          LC_ALL: "C",
        },
        encoding: "utf8",
      });
      assert.equal(
        verifyOut.status,
        0,
        `makepkg --verifysource должен успешно проходить (код 0). stderr: ${verifyOut.stderr}`,
      );
      assert.match(
        `${verifyOut.stdout}\n${verifyOut.stderr}`,
        /Validating source files with sha256sums\.\.\./,
      );
      assert.match(
        `${verifyOut.stdout}\n${verifyOut.stderr}`,
        /discord-gpui-v1\.0\.0-beta\.1-linux-x86_64\.tar\.gz \.\.\. Passed/,
      );
    } finally {
      rmSync(tmpBuildDir, { recursive: true, force: true });
    }

    // 2. Позитивный прогон makepkg --verifysource без передачи SRCDEST в env (авторазрешение rust/dist из PKGBUILD)
    const fallbackBuildDir = join(tmpdir(), `makepkg-discord-gpui-${process.getuid?.() ?? 1000}`);
    try {
      const verifyDefaultEnv = spawnSync("makepkg", ["--verifysource"], {
        cwd: ARCH_DIR,
        env: { ...process.env, LC_ALL: "C" },
        encoding: "utf8",
      });
      assert.equal(
        verifyDefaultEnv.status,
        0,
        `makepkg --verifysource без внешнего SRCDEST должен проходить: ${verifyDefaultEnv.stderr}`,
      );
      assert.equal(
        existsSync(join(ARCH_DIR, "src")),
        false,
        "makepkg --verifysource не должен оставлять каталог packaging/arch/src в репозитории",
      );
    } finally {
      rmSync(fallbackBuildDir, { recursive: true, force: true });
    }

    // 3. Негативный сценарий: повреждённый файл (несовпадение SHA-256) должен вызывать отказ makepkg --verifysource
    const negDir = mkdtempSync(join(tmpdir(), "arch-neg-checksum-964-"));
    try {
      const pkgbuildContent = readFileSync(PKGBUILD_PATH, "utf8");
      writeFileSync(join(negDir, "PKGBUILD"), pkgbuildContent, "utf8");
      // Подкладываем повреждённый файл discord-gpui в startdir (имеет приоритет над SRCDEST)
      writeFileSync(join(negDir, "discord-gpui"), "corrupted-binary-payload", "utf8");

      const failOut = spawnSync("makepkg", ["--verifysource"], {
        cwd: negDir,
        env: {
          ...process.env,
          BUILDDIR: negDir,
          SRCDEST: DIST_DIR,
          LC_ALL: "C",
        },
        encoding: "utf8",
      });
      assert.notEqual(
        failOut.status,
        0,
        "makepkg --verifysource обязан завершиться с ненулевым кодом при несовпадении SHA-256",
      );
      assert.match(
        `${failOut.stdout}\n${failOut.stderr}`,
        /FAILED|One or more files did not pass the validity check/i,
      );
    } finally {
      rmSync(negDir, { recursive: true, force: true });
    }

    // 4. Негативный сценарий: дефис в pkgver (1.0.0-beta.1) отклоняется линтером makepkg
    const badVerDir = mkdtempSync(join(tmpdir(), "arch-neg-pkgver-964-"));
    try {
      const badPkgbuild = readFileSync(PKGBUILD_PATH, "utf8").replace(
        /^pkgver=.*$/m,
        "pkgver=1.0.0-beta.1",
      );
      writeFileSync(join(badVerDir, "PKGBUILD"), badPkgbuild, "utf8");
      const badVerOut = spawnSync("makepkg", ["--printsrcinfo"], {
        cwd: badVerDir,
        env: { ...process.env, LC_ALL: "C" },
        encoding: "utf8",
      });
      assert.notEqual(badVerOut.status, 0, "makepkg обязан отклонять pkgver с дефисом");
      assert.match(badVerOut.stderr, /pkgver is not allowed to contain/i);
    } finally {
      rmSync(badVerDir, { recursive: true, force: true });
    }
  });

  it("Шаг 6: функции check() и package() в PKGBUILD корректно проверяют и устанавливают артефакты в $pkgdir", () => {
    const tmp = mkdtempSync(join(tmpdir(), "arch-pkg-964-"));
    try {
      const srcdir = join(tmp, "src");
      const pkgdir = join(tmp, "pkg");
      spawnSync("mkdir", ["-p", srcdir, pkgdir]);

      // Распаковываем релизный архив и копируем sbom.spdx.json так же, как это делает makepkg
      const tarPath = join(DIST_DIR, "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz");
      const extractOut = spawnSync("tar", ["-xzf", tarPath, "-C", srcdir], {
        encoding: "utf8",
      });
      assert.equal(extractOut.status, 0, `Ошибка распаковки архива: ${extractOut.stderr}`);
      writeFileSync(
        join(srcdir, "sbom.spdx.json"),
        readFileSync(join(DIST_DIR, "sbom.spdx.json")),
      );

      // Запускаем check() и package() из PKGBUILD в изолированном bash
      const runPkg = spawnSync(
        "bash",
        [
          "-e",
          "-c",
          `source "$1" && srcdir="$2" pkgdir="$3" check && srcdir="$2" pkgdir="$3" package`,
          "--",
          PKGBUILD_PATH,
          srcdir,
          pkgdir,
        ],
        {
          cwd: ARCH_DIR,
          env: {
            ...process.env,
            WAYLAND_DISPLAY: "wayland-1",
            DISPLAY: ":0",
          },
          encoding: "utf8",
        },
      );
      assert.equal(
        runPkg.status,
        0,
        `check() и package() должны выполняться с кодом 0: ${runPkg.stderr}`,
      );
      assert.match(runPkg.stdout, /GPUI_SMOKE_OK /);

      // Проверяем установленные в $pkgdir файлы
      const installedBin = join(pkgdir, "usr", "bin", "discord-gpui");
      const installedMit = join(pkgdir, "usr", "share", "licenses", "discord-gpui", "LICENSE-MIT");
      const installedApache = join(pkgdir, "usr", "share", "licenses", "discord-gpui", "LICENSE-APACHE");
      const installedSbom = join(pkgdir, "usr", "share", "doc", "discord-gpui", "sbom.spdx.json");
      const installedDesktop = join(pkgdir, "usr", "share", "applications", "discord-gpui.desktop");

      assert.ok(existsSync(installedBin), "Бинарник /usr/bin/discord-gpui должен быть установлен");
      const binStat = statSync(installedBin);
      assert.equal(binStat.mode & 0o111, 0o111, "Бинарник должен иметь права на исполнение (0755)");
      assert.equal(
        sha256File(installedBin),
        sha256File(join(DIST_DIR, "discord-gpui")),
        "SHA-256 установленного бинарника должен совпадать с rust/dist/discord-gpui",
      );

      assert.ok(existsSync(installedMit), "Файл лицензии LICENSE-MIT должен быть установлен");
      assert.ok(existsSync(installedApache), "Файл лицензии LICENSE-APACHE должен быть установлен");
      assert.ok(existsSync(installedSbom), "Файл SBOM sbom.spdx.json должен быть установлен");
      assert.equal(
        sha256File(installedSbom),
        sha256File(join(DIST_DIR, "sbom.spdx.json")),
        "SHA-256 установленного SBOM должен совпадать с rust/dist/sbom.spdx.json",
      );

      assert.ok(existsSync(installedDesktop), "Desktop-файл discord-gpui.desktop должен быть установлен");
      const desktopContent = readFileSync(installedDesktop, "utf8");
      assert.match(desktopContent, /^Exec=\/usr\/bin\/discord-gpui %u$/m);
      assert.match(
        desktopContent,
        /^MimeType=.*x-scheme-handler\/discord;.*x-scheme-handler\/disclip;/m,
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
