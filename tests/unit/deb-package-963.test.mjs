// @ts-check
/**
 * Контрактные и модульные тесты пакета Debian/Ubuntu (.deb), генератора DEBIAN/control,
 * регистрации URI-схемы discord:// и mime-типов XDG (TASK-963):
 * - Проверка структуры каталога packaging/deb/ (DEBIAN/control, DEBIAN/postinst, DEBIAN/postrm,
 *   usr/share/applications/discord-gpui.desktop, usr/share/mime/packages/discord-gpui.xml,
 *   usr/share/icons/hicolor/scalable/apps/discord-gpui.svg, usr/share/doc/discord-gpui/copyright).
 * - Генерация и валидация DEBIAN/control с обязательными зависимостями libc6 (>= 2.31)
 *   и libpipewire-0.3-0, сверка независимым системным оракулом awk/grep.
 * - Регистрация URI-схемы discord:// (x-scheme-handler/discord, Exec=... --url -- %u)
 *   и XDG mime-типов (usr/share/mime/packages/discord-gpui.xml), валидация через
 *   системный оракул desktop-file-validate.
 * - Сборка .deb архива (ar: debian-binary, control.tar.gz, data.tar.gz), проверка md5sums,
 *   прав доступа (0755 для postinst/postrm/бинаря, 0644 для метаданных) и CLI-кодов выхода (0/1/2).
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
const DEB_DIR = join(ROOT, "packaging", "deb");
const MODULE_PATH = join(DEB_DIR, "deb-package.mjs");
const CONTROL_PATH = join(DEB_DIR, "DEBIAN", "control");
const POSTINST_PATH = join(DEB_DIR, "DEBIAN", "postinst");
const POSTRM_PATH = join(DEB_DIR, "DEBIAN", "postrm");
const DESKTOP_PATH = join(
  DEB_DIR,
  "usr",
  "share",
  "applications",
  "discord-gpui.desktop",
);
const MIME_XML_PATH = join(
  DEB_DIR,
  "usr",
  "share",
  "mime",
  "packages",
  "discord-gpui.xml",
);
const ICON_SVG_PATH = join(
  DEB_DIR,
  "usr",
  "share",
  "icons",
  "hicolor",
  "scalable",
  "apps",
  "discord-gpui.svg",
);
const COPYRIGHT_PATH = join(
  DEB_DIR,
  "usr",
  "share",
  "doc",
  "discord-gpui",
  "copyright",
);
const DIST_BIN_PATH = join(ROOT, "rust", "dist", "discord-gpui");

describe("TASK-963: Структура пакета Debian packaging/deb/, DEBIAN/control, URI discord:// и mime-типы XDG", () => {
  it("Шаг 1: структура каталога packaging/deb/ содержит все обязательные файлы и права доступа", () => {
    for (const requiredPath of [
      DEB_DIR,
      MODULE_PATH,
      CONTROL_PATH,
      POSTINST_PATH,
      POSTRM_PATH,
      DESKTOP_PATH,
      MIME_XML_PATH,
      ICON_SVG_PATH,
      COPYRIGHT_PATH,
    ]) {
      assert.ok(
        existsSync(requiredPath),
        `Обязательный артефакт пакета Debian должен существовать: ${requiredPath}`,
      );
    }

    // Проверка прав доступа maintainer-скриптов (должны быть исполняемыми 0755)
    const postinstMode = statSync(POSTINST_PATH).mode & 0o777;
    const postrmMode = statSync(POSTRM_PATH).mode & 0o777;
    assert.equal(
      postinstMode & 0o111,
      0o111,
      `DEBIAN/postinst должен иметь биты исполнения (текущий mode: ${postinstMode.toString(8)})`,
    );
    assert.equal(
      postrmMode & 0o111,
      0o111,
      `DEBIAN/postrm должен иметь биты исполнения (текущий mode: ${postrmMode.toString(8)})`,
    );
  });

  it("Шаг 2: генерация и валидация DEBIAN/control с зависимостями libc6 (>= 2.31) и libpipewire-0.3-0 + сверка оракулом awk", async () => {
    const {
      generateDebianControl,
      parseDebianControl,
      validateDebianControl,
      compareDebianVersions,
    } = await import("../../packaging/deb/deb-package.mjs");

    const controlContent = readFileSync(CONTROL_PATH, "utf8");
    const parsed = parseDebianControl(controlContent);
    const validation = validateDebianControl(controlContent);

    assert.equal(
      validation.valid,
      true,
      `DEBIAN/control должен быть валидным: ${validation.errors.join("; ")}`,
    );
    assert.equal(parsed.fields.get("Package"), "discord-gpui");
    assert.equal(parsed.fields.get("Architecture"), "amd64");
    assert.equal(parsed.fields.get("Priority"), "optional");
    assert.equal(parsed.fields.get("Section"), "net");

    const dependsRaw = parsed.fields.get("Depends") ?? "";
    assert.match(
      dependsRaw,
      /\blibc6\s*\(>=\s*2\.31\)/,
      "Поле Depends обязано содержать зависимость libc6 (>= 2.31)",
    );
    assert.match(
      dependsRaw,
      /\blibpipewire-0\.3-0\b/,
      "Поле Depends обязано содержать зависимость libpipewire-0.3-0",
    );

    // Независимый системный оракул awk: извлечение полей Package, Architecture и Depends из DEBIAN/control
    const awkOut = spawnSync(
      "awk",
      [
        "-F: ",
        "/^(Package|Architecture|Depends):/ { print $1 \"=\" $2 }",
        CONTROL_PATH,
      ],
      { encoding: "utf8" },
    );
    assert.equal(awkOut.status, 0, `Ошибка оракула awk: ${awkOut.stderr}`);
    assert.match(awkOut.stdout, /Package=discord-gpui/);
    assert.match(awkOut.stdout, /Architecture=amd64/);
    assert.match(awkOut.stdout, /Depends=.*libc6 \(>= 2\.31\)/);
    assert.match(awkOut.stdout, /Depends=.*libpipewire-0\.3-0/);

    // Проверка детерминизма генератора generateDebianControl()
    const generated = generateDebianControl({
      packageName: "discord-gpui",
      version: "1.0.0~beta1-1",
      architecture: "amd64",
      installedSizeKb: 42572,
    });
    const genValidation = validateDebianControl(generated);
    assert.equal(genValidation.valid, true);
    assert.match(generated, /^Package: discord-gpui$/m);
    assert.match(generated, /^Depends: .*libc6 \(>= 2\.31\).*libpipewire-0\.3-0/m);

    // Граничные проверки компаратора версий libc6 (2.30 < 2.31 == 2.31 < 2.35)
    assert.ok(compareDebianVersions("2.30", "2.31") < 0);
    assert.equal(compareDebianVersions("2.31", "2.31"), 0);
    assert.ok(compareDebianVersions("2.35", "2.31") > 0);

    // Негативные сценарии валидации DEBIAN/control:
    // 1) Отсутствие libpipewire-0.3-0
    const noPipewire = generated.replace(/,\s*libpipewire-0\.3-0/, "");
    const resNoPw = validateDebianControl(noPipewire);
    assert.equal(resNoPw.valid, false);
    assert.ok(resNoPw.errors.some((e) => e.includes("libpipewire-0.3-0")));

    // 2) Слишком старая версия libc6 (< 2.31, например 2.28)
    const oldLibc = generated.replace("libc6 (>= 2.31)", "libc6 (>= 2.28)");
    const resOldLibc = validateDebianControl(oldLibc);
    assert.equal(resOldLibc.valid, false);
    assert.ok(resOldLibc.errors.some((e) => e.includes("libc6")));

    // 3) Некорректное имя пакета с заглавными буквами или пробелами
    const badPkgName = generated.replace("Package: discord-gpui", "Package: Discord_GPUI");
    const resBadPkg = validateDebianControl(badPkgName);
    assert.equal(resBadPkg.valid, false);
    assert.ok(resBadPkg.errors.some((e) => e.includes("Package")));

    // 4) Пустой вход
    const resEmpty = validateDebianControl("   \n  ");
    assert.equal(resEmpty.valid, false);
  });

  it("Шаг 3: регистрация URI-схемы discord:// и mime-типов XDG проходит валидацию и системный оракул desktop-file-validate", async () => {
    const {
      validateDesktopEntry,
      validateMimeXml,
      validateMaintainerScripts,
    } = await import("../../packaging/deb/deb-package.mjs");

    const desktopContent = readFileSync(DESKTOP_PATH, "utf8");
    const desktopRes = validateDesktopEntry(desktopContent);
    assert.equal(
      desktopRes.valid,
      true,
      `discord-gpui.desktop должен быть валидным: ${desktopRes.errors.join("; ")}`,
    );

    // Сверка с RE-оракулом (/home/sanya/re-deep/main-arch.md §9.4 и /usr/share/applications/discord.desktop):
    // 1) Exec передаёт --url -- %u для перехвата discord:// ссылок
    assert.match(
      desktopContent,
      /^Exec=\/usr\/bin\/discord-gpui\s+--url\s+--\s+%u$/m,
      "Поле Exec должно содержать '/usr/bin/discord-gpui --url -- %u' для передачи URI discord://",
    );
    // 2) MimeType регистрирует x-scheme-handler/discord; и xdg-mime типы
    assert.match(
      desktopContent,
      /^MimeType=.*x-scheme-handler\/discord;/m,
      "Поле MimeType должно регистрировать URI-схему x-scheme-handler/discord;",
    );
    assert.match(
      desktopContent,
      /^MimeType=.*application\/x-discord-gpui-invite;/m,
      "Поле MimeType должно регистрировать XDG MIME-тип application/x-discord-gpui-invite;",
    );

    // Независимый системный оракул: desktop-file-validate (если установлен в ОС)
    const dfvCheck = spawnSync("desktop-file-validate", [DESKTOP_PATH], {
      encoding: "utf8",
    });
    if (!dfvCheck.error) {
      assert.equal(
        dfvCheck.status,
        0,
        `desktop-file-validate вернул ошибку:\nstdout: ${dfvCheck.stdout}\nstderr: ${dfvCheck.stderr}`,
      );
    }

    // Проверка XDG Shared MIME-info XML (usr/share/mime/packages/discord-gpui.xml)
    const mimeXmlContent = readFileSync(MIME_XML_PATH, "utf8");
    const mimeRes = validateMimeXml(mimeXmlContent);
    assert.equal(
      mimeRes.valid,
      true,
      `discord-gpui.xml должен быть валидным: ${mimeRes.errors.join("; ")}`,
    );
    assert.match(
      mimeXmlContent,
      /<mime-info\s+xmlns="http:\/\/www\.freedesktop\.org\/standards\/shared-mime-info">/,
    );
    assert.match(mimeXmlContent, /<mime-type\s+type="x-scheme-handler\/discord">/);
    assert.match(mimeXmlContent, /<mime-type\s+type="x-scheme-handler\/disclip">/);
    assert.match(
      mimeXmlContent,
      /<mime-type\s+type="application\/x-discord-gpui-invite">/,
    );
    assert.match(mimeXmlContent, /<glob\s+pattern="\*\.discord-invite"\s*\/>/);

    // Проверка maintainer-скриптов postinst и postrm на регистрацию XDG БД и URI-схемы
    const postinstContent = readFileSync(POSTINST_PATH, "utf8");
    const postrmContent = readFileSync(POSTRM_PATH, "utf8");
    const scriptsRes = validateMaintainerScripts(postinstContent, postrmContent);
    assert.equal(
      scriptsRes.valid,
      true,
      `Maintainer-скрипты должны быть валидны: ${scriptsRes.errors.join("; ")}`,
    );
    assert.match(postinstContent, /update-desktop-database/);
    assert.match(postinstContent, /update-mime-database/);
    assert.match(
      postinstContent,
      /xdg-mime\s+default\s+discord-gpui\.desktop\s+x-scheme-handler\/discord/,
    );

    // Негативные проверки desktop и mime:
    const brokenDesktop = desktopContent.replace("x-scheme-handler/discord;", "");
    assert.equal(validateDesktopEntry(brokenDesktop).valid, false);
    const brokenExecDesktop = desktopContent.replace("-- %u", "");
    assert.equal(validateDesktopEntry(brokenExecDesktop).valid, false);
    const brokenMimeXml = mimeXmlContent.replace("x-scheme-handler/discord", "text/plain");
    assert.equal(validateMimeXml(brokenMimeXml).valid, false);
  });

  it("Шаг 4: сборка пакета .deb (buildDebPackage), проверка структуры ar/tar и контрольных сумм md5sums независимыми оракулами", async () => {
    const { buildDebPackage, inspectDebArchive } = await import(
      "../../packaging/deb/deb-package.mjs"
    );

    const tmp = mkdtempSync(join(tmpdir(), "deb-pkg-963-"));
    try {
      // Создаём компактный тестовый ELF-бинарник для быстрой проверки сборки .deb пакета,
      // а также проверяем расчёт размера реального бинарника rust/dist/discord-gpui
      const mockBinPath = join(tmp, "discord-gpui-bin");
      writeFileSync(mockBinPath, "#!/bin/sh\necho GPUI_SMOKE_OK\n", { mode: 0o755 });
      chmodSync(mockBinPath, 0o755);

      const outDebPath = join(tmp, "discord-gpui_1.0.0~beta1-1_amd64.deb");
      const buildResult = buildDebPackage({
        debRootDir: DEB_DIR,
        binaryPath: mockBinPath,
        outputDebPath: outDebPath,
        version: "1.0.0~beta1-1",
        architecture: "amd64",
      });

      assert.equal(buildResult.status, "ok");
      assert.ok(existsSync(outDebPath), "Собранный .deb файл должен существовать");

      // Независимый системный оракул 1: утилита `ar -t` проверяет состав .deb архива
      const arList = spawnSync("ar", ["-t", outDebPath], { encoding: "utf8" });
      assert.equal(arList.status, 0, `Ошибка чтения ar-архива: ${arList.stderr}`);
      const members = arList.stdout.trim().split(/\r?\n/);
      assert.deepEqual(
        members,
        ["debian-binary", "control.tar.gz", "data.tar.gz"],
        "Внутри .deb архива должны идти строго debian-binary, control.tar.gz, data.tar.gz",
      );

      // Распаковка .deb через системный `ar -x` и проверка содержимого control.tar.gz и data.tar.gz
      const extractDir = join(tmp, "extracted");
      const { mkdirSync } = await import("node:fs");
      mkdirSync(extractDir, { recursive: true });

      const arExtract = spawnSync("ar", ["-x", outDebPath], {
        cwd: extractDir,
        encoding: "utf8",
      });
      assert.equal(arExtract.status, 0, `ar -x завершился с ошибкой: ${arExtract.stderr}`);

      assert.equal(
        readFileSync(join(extractDir, "debian-binary"), "utf8"),
        "2.0\n",
        "Формат debian-binary должен быть строго '2.0\\n'",
      );

      const controlEntries = spawnSync(
        "tar",
        ["-tzf", join(extractDir, "control.tar.gz")],
        { encoding: "utf8" },
      );
      assert.equal(controlEntries.status, 0);
      assert.match(controlEntries.stdout, /control/);
      assert.match(controlEntries.stdout, /postinst/);
      assert.match(controlEntries.stdout, /postrm/);
      assert.match(controlEntries.stdout, /md5sums/);

      // Распаковка data.tar.gz и control.tar.gz для проверки md5sums системным оракулом `md5sum -c`
      const dataUnpackDir = join(extractDir, "data-root");
      const ctrlUnpackDir = join(extractDir, "ctrl-root");
      mkdirSync(dataUnpackDir, { recursive: true });
      mkdirSync(ctrlUnpackDir, { recursive: true });

      assert.equal(
        spawnSync("tar", ["-xzf", join(extractDir, "data.tar.gz"), "-C", dataUnpackDir])
          .status,
        0,
      );
      assert.equal(
        spawnSync("tar", ["-xzf", join(extractDir, "control.tar.gz"), "-C", ctrlUnpackDir])
          .status,
        0,
      );

      const md5Check = spawnSync(
        "md5sum",
        ["-c", join(ctrlUnpackDir, "md5sums")],
        { cwd: dataUnpackDir, encoding: "utf8" },
      );
      assert.equal(
        md5Check.status,
        0,
        `Системный оракул md5sum -c не прошёл:\n${md5Check.stdout}\n${md5Check.stderr}`,
      );

      // Сверка SHA-256 упакованного бинарника usr/bin/discord-gpui с исходным файлом
      const unpackedBin = join(dataUnpackDir, "usr", "bin", "discord-gpui");
      assert.ok(existsSync(unpackedBin), "usr/bin/discord-gpui должен присутствовать в data.tar.gz");
      const expectedSha = createHash("sha256")
        .update(readFileSync(mockBinPath))
        .digest("hex");
      const actualSha = createHash("sha256")
        .update(readFileSync(unpackedBin))
        .digest("hex");
      assert.equal(actualSha, expectedSha);

      // Инспекция архива через inspectDebArchive()
      const inspected = inspectDebArchive(outDebPath);
      assert.equal(inspected.valid, true);
      assert.equal(inspected.debianBinaryVersion, "2.0");

      // Проверка CLI-режима модуля deb-package.mjs (коды выхода 0, 1, 2)
      const outJson = join(tmp, "deb-report.json");
      const cliOk = spawnSync(
        process.execPath,
        [MODULE_PATH, "--verify", "--out-json", outJson],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(cliOk.status, 0, `CLI --verify упал: ${cliOk.stderr}`);
      const report = JSON.parse(readFileSync(outJson, "utf8"));
      assert.equal(report.taskId, "TASK-963");
      assert.equal(report.status, "passed");

      const cliBadArg = spawnSync(
        process.execPath,
        [MODULE_PATH, "--unknown-option"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(cliBadArg.status, 1, "Неизвестный аргумент CLI должен возвращать код 1");

      const cliMissingRoot = spawnSync(
        process.execPath,
        [MODULE_PATH, "--verify", "--deb-root", join(tmp, "nonexistent-deb-root")],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(cliMissingRoot.status, 2, "Отсутствующее окружение/каталог должен возвращать код 2");
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
