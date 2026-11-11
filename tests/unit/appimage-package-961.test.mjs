// @ts-check
/**
 * Контрактные и модульные тесты сборки автономного AppImage пакета и headless-верификации (TASK-961):
 * - SUB-961-1: Спецификация `packaging/appimage/AppRun`, `packaging/appimage/discord-gpui.desktop`
 *   и иконки `packaging/appimage/discord-gpui.svg` (валидация через системный оракул `desktop-file-validate`,
 *   проверка настройки `APPDIR`, `LD_LIBRARY_PATH`, `XDG_DATA_DIRS`, sandbox-эвристики и `exec` без лишнего процесса).
 * - SUB-961-2: Скрипт сборки `scripts/build-appimage.sh`, бандлинг разделяемых библиотек в `usr/lib/`,
 *   формирование валидного ELF-заголовка Type 2 AppImage (`\x7fELF\x02\x01\x01` + `AI\x02` на смещении 8..11),
 *   распаковка `--appimage-extract` и побайтовая сверка `usr/bin/discord-gpui` с эталоном `rust/dist/discord-gpui`.
 * - SUB-961-3: Headless-верификация `--headless-smoke` непосредственно внутри собранного `.AppImage`
 *   (включая fallback `--appimage-extract-and-run` и `llvmpipe`), проверка бюджета памяти `VmRSS < 60 MB`,
 *   граничных/негативных сценариев CLI и наблюдаемости (`--report-json`).
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
const BUILD_SCRIPT = join(ROOT, "scripts", "build-appimage.sh");
const APPRUN_PATH = join(ROOT, "packaging", "appimage", "AppRun");
const DESKTOP_PATH = join(ROOT, "packaging", "appimage", "discord-gpui.desktop");
const ICON_SVG_PATH = join(ROOT, "packaging", "appimage", "discord-gpui.svg");
const DIST_BIN_PATH = join(ROOT, "rust", "dist", "discord-gpui");
const DEFAULT_APPIMAGE_PATH = join(
  ROOT,
  "rust",
  "dist",
  "discord-gpui-v1.0.0-beta.1-x86_64.AppImage",
);

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
 * Проверяет 20-байтный ELF-заголовок Type 2 AppImage для архитектуры x86_64.
 * Независимый бинарный оракул по спецификации AppImage Type 2:
 * - bytes 0..7:  `\x7fELF\x02\x01\x01` (ELF64, LSB, version 1)
 * - bytes 8..11: `AI\x02` (`0x41, 0x49, 0x02`)
 * - bytes 16..18: `2` (ET_EXEC) или `3` (ET_DYN)
 * - bytes 18..20: `62` (`0x3e, 0x00` — EM_X86_64)
 * @param {Buffer} buf
 * @returns {boolean}
 */
function isValidType2AppImageHeader(buf) {
  if (buf.length < 20) return false;
  const elfMagicOk =
    buf[0] === 0x7f &&
    buf[1] === 0x45 &&
    buf[2] === 0x4c &&
    buf[3] === 0x46 &&
    buf[4] === 0x02 &&
    buf[5] === 0x01 &&
    buf[6] === 0x01;
  const appImageMagicOk =
    buf[8] === 0x41 && buf[9] === 0x49 && buf[10] === 0x02;
  const eType = buf.readUInt16LE(16);
  const eMachine = buf.readUInt16LE(18);
  return (
    elfMagicOk &&
    appImageMagicOk &&
    (eType === 2 || eType === 3) &&
    eMachine === 62
  );
}

describe("TASK-961: Сборка автономного AppImage пакета и headless-верификация", () => {
  it("SUB-961-1: спецификация packaging/appimage/AppRun, desktop-файл и иконка валидны и поддерживают sandbox/env контракты", () => {
    assert.ok(
      existsSync(APPRUN_PATH),
      `Файл AppRun должен существовать: ${APPRUN_PATH}`,
    );
    assert.ok(
      existsSync(DESKTOP_PATH),
      `Desktop-файл должен существовать: ${DESKTOP_PATH}`,
    );
    assert.ok(
      existsSync(ICON_SVG_PATH),
      `Векторная иконка должна существовать: ${ICON_SVG_PATH}`,
    );

    const appRunStat = statSync(APPRUN_PATH);
    assert.ok(
      (appRunStat.mode & 0o111) !== 0,
      "packaging/appimage/AppRun должен иметь права на исполнение (chmod +x)",
    );

    const appRunText = readFileSync(APPRUN_PATH, "utf8");
    assert.match(appRunText, /^#!\/usr\/bin\/env bash|^#!\/bin\/sh/);
    assert.match(appRunText, /APPDIR/);
    assert.match(appRunText, /LD_LIBRARY_PATH/);
    assert.match(appRunText, /XDG_DATA_DIRS/);
    assert.match(appRunText, /exec\s+"\$APPDIR\/usr\/bin\/discord-gpui"/);

    // Проверка desktop-файла системным оракулом desktop-file-validate (если установлен)
    const desktopText = readFileSync(DESKTOP_PATH, "utf8");
    assert.match(desktopText, /^\[Desktop Entry\]/m);
    assert.match(desktopText, /^Type=Application$/m);
    assert.match(desktopText, /^Name=Discord GPUI$/m);
    assert.match(desktopText, /^Exec=discord-gpui %U$/m);
    assert.match(desktopText, /^Icon=discord-gpui$/m);
    assert.match(desktopText, /^Categories=.*Network.*InstantMessaging/m);
    assert.match(desktopText, /^MimeType=.*x-scheme-handler\/discord;/m);
    assert.match(desktopText, /^StartupWMClass=discord-gpui$/m);

    const validateCheck = spawnSync("desktop-file-validate", [DESKTOP_PATH], {
      encoding: "utf8",
    });
    if (validateCheck.error === undefined) {
      assert.equal(
        validateCheck.status,
        0,
        `desktop-file-validate вернул ошибку: ${validateCheck.stderr || validateCheck.stdout}`,
      );
    }

    // Проверка SVG-иконки
    const svgText = readFileSync(ICON_SVG_PATH, "utf8");
    assert.match(svgText, /<svg[^>]*viewBox=/);
    assert.match(svgText, /#5865f2/i);

    // Поведенческая проверка AppRun на изолированном макете AppDir (sandbox check, env, ошибка отсутствия бинарника)
    const tmp = mkdtempSync(join(tmpdir(), "apprun-961-"));
    try {
      const fakeAppRun = join(tmp, "AppRun");
      writeFileSync(fakeAppRun, appRunText);
      chmodSync(fakeAppRun, 0o755);

      // 1. Негативный сценарий: в AppDir отсутствует usr/bin/discord-gpui -> код выхода 2
      const missingBinRun = spawnSync(fakeAppRun, ["--headless-smoke"], {
        encoding: "utf8",
      });
      assert.equal(
        missingBinRun.status,
        2,
        `AppRun без бинарника должен завершаться с кодом 2, получено: ${missingBinRun.status}`,
      );

      // 2. Проверка проброса окружения, LD_LIBRARY_PATH и sandbox-эвристики через тестовый бинарник-оракул
      const usrBinDir = join(tmp, "usr", "bin");
      const usrLibDir = join(tmp, "usr", "lib");
      spawnSync("mkdir", ["-p", usrBinDir, usrLibDir]);
      const probeBin = join(usrBinDir, "discord-gpui");
      writeFileSync(
        probeBin,
        [
          "#!/usr/bin/env bash",
          'printf \'{"appdir":"%s","ld_library_path":"%s","sandbox":"%s","gl_software":"%s","args":"%s"}\\n\' "$APPDIR" "$LD_LIBRARY_PATH" "$DISCORD_GPUI_SANDBOX" "$LIBGL_ALWAYS_SOFTWARE" "$*"',
        ].join("\n"),
      );
      chmodSync(probeBin, 0o755);

      const probeRun = spawnSync(fakeAppRun, ["--no-sandbox", "--headless-smoke"], {
        env: {
          ...process.env,
          DISCORD_APPIMAGE_SOFTWARE_RENDER: "1",
        },
        encoding: "utf8",
      });
      assert.equal(probeRun.status, 0, `stderr: ${probeRun.stderr}`);
      const probeJson = JSON.parse(probeRun.stdout.trim());
      assert.equal(probeJson.appdir, tmp);
      assert.ok(
        probeJson.ld_library_path.startsWith(`${usrLibDir}`),
        `LD_LIBRARY_PATH должен начинаться с ${usrLibDir}, получено: ${probeJson.ld_library_path}`,
      );
      assert.equal(probeJson.sandbox, "fallback-no-sandbox");
      assert.equal(probeJson.gl_software, "1");
      assert.equal(probeJson.args, "--no-sandbox --headless-smoke");
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-961-2: scripts/build-appimage.sh собирает валидный Type 2 AppImage с бандлингом зависимостей, иконкой и .desktop файлом", () => {
    assert.ok(
      existsSync(BUILD_SCRIPT),
      `Скрипт сборки должен существовать: ${BUILD_SCRIPT}`,
    );
    const scriptStat = statSync(BUILD_SCRIPT);
    assert.ok(
      (scriptStat.mode & 0o111) !== 0,
      "scripts/build-appimage.sh должен быть исполняемым (chmod +x)",
    );

    const tmp = mkdtempSync(join(tmpdir(), "appimage-build-961-"));
    try {
      const outAppImage = join(
        tmp,
        "discord-gpui-v1.0.0-beta.1-x86_64.AppImage",
      );
      const reportJsonPath = join(tmp, "appimage-report.json");

      // Сборка AppImage во временном каталоге с генерацией JSON-отчёта
      const buildRes = spawnSync(
        "bash",
        [
          BUILD_SCRIPT,
          "--output",
          outAppImage,
          "--report-json",
          reportJsonPath,
        ],
        {
          cwd: ROOT,
          encoding: "utf8",
        },
      );
      assert.equal(
        buildRes.status,
        0,
        `Сборка AppImage завершилась с ошибкой:\nstdout: ${buildRes.stdout}\nstderr: ${buildRes.stderr}`,
      );
      assert.ok(existsSync(outAppImage), "Выходной файл .AppImage должен быть создан");
      assert.ok(existsSync(reportJsonPath), "JSON-отчёт сборки должен быть создан");

      // 1. Проверка размера и прав доступа .AppImage
      const appImageStat = statSync(outAppImage);
      assert.ok(
        (appImageStat.mode & 0o111) !== 0,
        "Собранный .AppImage должен иметь права 0755",
      );
      assert.ok(
        appImageStat.size > 5 * 1024 * 1024 &&
          appImageStat.size < 256 * 1024 * 1024,
        `Размер .AppImage (${appImageStat.size} байт) должен быть в пределах 5 МБ .. 256 МБ`,
      );

      // 2. Независимый бинарный оракул: проверка 20-байтного заголовка Type 2 AppImage (ELF64 + AI\x02)
      const headerBuf = readFileSync(outAppImage).subarray(0, 64);
      assert.equal(
        isValidType2AppImageHeader(headerBuf),
        true,
        "Заголовок .AppImage должен строго соответствовать спецификации Type 2 x86_64 (\\x7fELF\\x02\\x01\\x01 + AI\\x02)",
      );

      // 3. Проверка сервисных флагов рантайма AppImage (--appimage-offset, --appimage-updateinformation)
      const offsetRes = spawnSync(outAppImage, ["--appimage-offset"], {
        encoding: "utf8",
      });
      assert.equal(offsetRes.status, 0);
      const offsetVal = Number.parseInt(offsetRes.stdout.trim(), 10);
      assert.ok(
        Number.isInteger(offsetVal) && offsetVal >= 4096 && offsetVal % 4096 === 0,
        `Смещение архива AppImage (${offsetRes.stdout.trim()}) должно быть кратно 4096`,
      );

      const updateInfoRes = spawnSync(
        outAppImage,
        ["--appimage-updateinformation"],
        { encoding: "utf8" },
      );
      assert.equal(updateInfoRes.status, 0);
      assert.match(
        updateInfoRes.stdout.trim(),
        /^gh-releases-zsync\|discord-lab\|discord-gpui\|/,
      );

      // 4. Распаковка через --appimage-extract и независимая проверка содержимого squashfs-root
      const extractDir = join(tmp, "extract-work");
      spawnSync("mkdir", ["-p", extractDir]);
      const extractRes = spawnSync(outAppImage, ["--appimage-extract"], {
        cwd: extractDir,
        encoding: "utf8",
      });
      assert.equal(
        extractRes.status,
        0,
        `--appimage-extract завершился с ошибкой: ${extractRes.stderr}`,
      );

      const squashRoot = join(extractDir, "squashfs-root");
      assert.ok(existsSync(squashRoot), "Директория squashfs-root должна быть создана");
      assert.ok(existsSync(join(squashRoot, "AppRun")), "В корне AppDir должен быть AppRun");
      assert.ok(
        existsSync(join(squashRoot, "discord-gpui.desktop")),
        "В корне AppDir должен быть discord-gpui.desktop",
      );
      assert.ok(
        existsSync(join(squashRoot, "discord-gpui.png")),
        "В корне AppDir должна быть иконка discord-gpui.png",
      );
      assert.ok(
        existsSync(join(squashRoot, ".DirIcon")),
        "В корне AppDir должен присутствовать .DirIcon",
      );
      assert.ok(
        existsSync(
          join(squashRoot, "usr", "share", "applications", "discord-gpui.desktop"),
        ),
        "Должен присутствовать usr/share/applications/discord-gpui.desktop",
      );
      assert.ok(
        existsSync(
          join(
            squashRoot,
            "usr",
            "share",
            "icons",
            "hicolor",
            "128x128",
            "apps",
            "discord-gpui.png",
          ),
        ),
        "Должна присутствовать иконка hicolor/128x128/apps/discord-gpui.png",
      );
      assert.ok(
        existsSync(
          join(
            squashRoot,
            "usr",
            "share",
            "icons",
            "hicolor",
            "scalable",
            "apps",
            "discord-gpui.svg",
          ),
        ),
        "Должна присутствовать векторная иконка hicolor/scalable/apps/discord-gpui.svg",
      );

      // 5. Побайтовая сверка упакованного usr/bin/discord-gpui с эталоном rust/dist/discord-gpui
      const extractedBin = join(squashRoot, "usr", "bin", "discord-gpui");
      assert.ok(existsSync(extractedBin), "Должен присутствовать usr/bin/discord-gpui");
      assert.equal(
        sha256File(extractedBin),
        sha256File(DIST_BIN_PATH),
        "SHA-256 бинарника внутри AppImage должен побайтово совпадать с rust/dist/discord-gpui",
      );

      // 6. Проверка бандлинга зависимостей в usr/lib/ через независимый системный оракул ldd
      const bundledLibDir = join(squashRoot, "usr", "lib");
      assert.ok(
        existsSync(join(bundledLibDir, "libxkbcommon.so.0")),
        "В usr/lib должна быть сбандлена библиотека libxkbcommon.so.0",
      );
      assert.ok(
        existsSync(join(bundledLibDir, "libxkbcommon-x11.so.0")),
        "В usr/lib должна быть сбандлена библиотека libxkbcommon-x11.so.0",
      );
      assert.ok(
        existsSync(join(bundledLibDir, "bundled-deps.json")),
        "В usr/lib должен присутствовать манифест сбандленных библиотек bundled-deps.json",
      );

      const lddWithBundled = spawnSync("ldd", [extractedBin], {
        env: {
          ...process.env,
          LC_ALL: "C",
          LD_LIBRARY_PATH: bundledLibDir,
        },
        encoding: "utf8",
      });
      assert.equal(lddWithBundled.status, 0);
      assert.doesNotMatch(
        lddWithBundled.stdout,
        /not found/i,
        "Все динамические зависимости внутри AppDir должны разрешаться без 'not found'",
      );
      assert.ok(
        lddWithBundled.stdout.includes(
          join(bundledLibDir, "libxkbcommon.so.0"),
        ),
        `ldd должен разрешать libxkbcommon.so.0 из бандла ${bundledLibDir}`,
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-961-3: выполняет --headless-smoke внутри релизного AppImage (VmRSS < 60 MB) и проверяет fallback/негативные режимы", () => {
    assert.ok(
      existsSync(DEFAULT_APPIMAGE_PATH),
      `Релизный пакет AppImage должен существовать: ${DEFAULT_APPIMAGE_PATH}`,
    );

    // 1. Прямой запуск собранного AppImage с флагом --headless-smoke
    const smokeRun = spawnSync(DEFAULT_APPIMAGE_PATH, ["--headless-smoke"], {
      env: { ...process.env, WAYLAND_DISPLAY: "wayland-1", DISPLAY: ":0" },
      encoding: "utf8",
    });
    assert.equal(
      smokeRun.status,
      0,
      `Запуск --headless-smoke внутри AppImage упал:\nstdout: ${smokeRun.stdout}\nstderr: ${smokeRun.stderr}`,
    );
    assert.match(smokeRun.stdout, /^GPUI_SMOKE_OK \{/m);

    const jsonLine = smokeRun.stdout
      .split(/\r?\n/)
      .find((line) => line.startsWith("GPUI_SMOKE_OK "));
    assert.ok(jsonLine, "Вывод должен содержать строку GPUI_SMOKE_OK {...}");
    const smokePayload = JSON.parse(jsonLine.slice("GPUI_SMOKE_OK ".length));

    assert.equal(smokePayload.status, "ok");
    assert.equal(smokePayload.sqlite_wal_ok, true);
    assert.equal(smokePayload.fps_budget_ok, true);
    assert.ok(
      typeof smokePayload.vmrss_mb === "number" &&
        smokePayload.vmrss_mb > 0 &&
        smokePayload.vmrss_mb < 60.0,
      `Потребление памяти VmRSS внутри AppImage (${smokePayload.vmrss_mb} МБ) должно быть строго < 60 МБ`,
    );

    // 2. Fallback-режим без FUSE: --appimage-extract-and-run --headless-smoke под софтверным рендером llvmpipe
    const extractAndRun = spawnSync(
      DEFAULT_APPIMAGE_PATH,
      ["--appimage-extract-and-run", "--headless-smoke"],
      {
        env: {
          ...process.env,
          DISCORD_APPIMAGE_SOFTWARE_RENDER: "1",
          WAYLAND_DISPLAY: "wayland-1",
          DISPLAY: ":0",
        },
        encoding: "utf8",
      },
    );
    assert.equal(
      extractAndRun.status,
      0,
      `--appimage-extract-and-run --headless-smoke упал: ${extractAndRun.stderr}`,
    );
    assert.match(extractAndRun.stdout, /GPUI_SMOKE_OK /);

    // 3. Проверка --dry-run, негативных и граничных сценариев CLI scripts/build-appimage.sh
    const tmp = mkdtempSync(join(tmpdir(), "appimage-cli-961-"));
    try {
      // Dry-run не создаёт выходной файл и возвращает 0
      const dryRunTarget = join(tmp, "dry-run.AppImage");
      const dryRun = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--dry-run", "--output", dryRunTarget],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(dryRun.status, 0);
      assert.equal(
        existsSync(dryRunTarget),
        false,
        "В режиме --dry-run файл .AppImage не должен создаваться",
      );

      // Неизвестный флаг -> код 1
      const badFlag = spawnSync("bash", [BUILD_SCRIPT, "--unknown-flag"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(badFlag.status, 1);

      // Некорректная semver-версия -> код 1
      const badVer = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--version", "invalid_version!"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(badVer.status, 1);

      // Отсутствующий входной бинарник -> код 2
      const missingBin = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--binary", join(tmp, "missing-discord-gpui")],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(missingBin.status, 2);

      // Не-ELF файл вместо бинарника -> код 1
      const fakeElf = join(tmp, "fake-bin");
      writeFileSync(fakeElf, "not an elf binary\n");
      chmodSync(fakeElf, 0o755);
      const invalidElf = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--binary", fakeElf, "--output", join(tmp, "out.AppImage")],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(invalidElf.status, 1);

      // Граничная проверка превышения лимита памяти (--max-rss-mb 1 -> код 1, т.к. реальный VmRSS ~11.3 МБ)
      const rssFail = spawnSync(
        "bash",
        [
          BUILD_SCRIPT,
          "--output",
          join(tmp, "rss-fail.AppImage"),
          "--max-rss-mb",
          "1",
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        rssFail.status,
        1,
        "При превышении порога --max-rss-mb скрипт должен завершаться с кодом 1",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
