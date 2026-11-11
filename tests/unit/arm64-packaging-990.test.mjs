// @ts-check
/**
 * Контрактные и модульные тесты кросс-компиляции и упаковки для Linux ARM64
 * (`aarch64-unknown-linux-gnu`, Raspberry Pi 5 Cortex-A76, Apple Silicon Asahi Linux) (TASK-990):
 * - SUB-990-1: Конфигурация кросс-сборки Cargo (`cargo-config-aarch64.toml` и `packaging/arm64/cargo-config.toml`)
 *   для `aarch64-unknown-linux-gnu` с проверкой через независимый оракул Python 3 (`tomllib`) и профилей `generic`, `rpi5`, `asahi`.
 * - SUB-990-2: Генерация и верификация нативного ELF64 `aarch64` бинарника (`discord-gpui-aarch64`)
 *   независимыми бинарными и системными оракулами (`readelf`, `file`): `e_machine = 183` (`EM_AARCH64`),
 *   интерпретатор `/lib/ld-linux-aarch64.so.1`, выравнивание сегментов `PT_LOAD` `0x4000` (16 КБ страницы
 *   памяти Raspberry Pi 5 BCM2712 и Apple Silicon Asahi), `GNU_RELRO` + `BIND_NOW`, `GNU_STACK` RW, `BuildID[sha1]`, `stripped`.
 * - SUB-990-3: Генерация и инспекция ARM64 пакетов `.tar.gz`, `.deb` (`Architecture: arm64`) и Type 2 `.AppImage` (`EM_AARCH64`)
 *   с побайтовой сверкой упакованного бинарника `discord-gpui` с эталоном `rust/dist/discord-gpui-aarch64`.
 * - SUB-990-4: Регистрация контрольных сумм ARM64 артефактов в `rust/dist/SHA256SUMS`
 *   и верификация через независимый системный оракул `sha256sum -c SHA256SUMS` и `scripts/build-arm64.sh --verify`.
 * - SUB-990-5: Изолированная сборка, граничные и негативные сценарии CLI (коды выхода 0/1/2),
 *   обнаружение подмены архитектуры (`x86_64` вместо `aarch64`) или повреждения байтов, маскирование токенов.
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
const BUILD_SCRIPT = join(ROOT, "scripts", "build-arm64.sh");
const DIST_DIR = join(ROOT, "rust", "dist");
const SHA256SUMS_PATH = join(DIST_DIR, "SHA256SUMS");
const CARGO_CONFIG_DIST_PATH = join(DIST_DIR, "cargo-config-aarch64.toml");
const CARGO_CONFIG_PKG_PATH = join(ROOT, "packaging", "arm64", "cargo-config.toml");

const VERSION = "1.0.0-beta.1";
const ARM64_BIN_NAME = "discord-gpui-aarch64";
const ARM64_TARBALL_NAME = `discord-gpui-v${VERSION}-linux-aarch64.tar.gz`;
const ARM64_DEB_NAME = `discord-gpui_${VERSION}_arm64.deb`;
const ARM64_APPIMAGE_NAME = `discord-gpui-v${VERSION}-aarch64.AppImage`;

const ARM64_BIN_PATH = join(DIST_DIR, ARM64_BIN_NAME);
const ARM64_TARBALL_PATH = join(DIST_DIR, ARM64_TARBALL_NAME);
const ARM64_DEB_PATH = join(DIST_DIR, ARM64_DEB_NAME);
const ARM64_APPIMAGE_PATH = join(DIST_DIR, ARM64_APPIMAGE_NAME);

/** Константа архитектуры AArch64 в заголовке ELF64 (EM_AARCH64 = 183 = 0xb7). */
const EM_AARCH64 = 183;

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
 * Парсит 64-байтный заголовок ELF64 LSB независимым бинарным оракулом.
 * @param {Buffer} buf
 * @returns {{
 *   validElf64Lsb: boolean,
 *   osAbi: number,
 *   eType: number,
 *   eMachine: number,
 *   eVersion: number,
 *   eEntry: bigint,
 *   ePhoff: bigint,
 *   eShoff: bigint,
 *   eFlags: number,
 *   eEhsize: number,
 *   ePhentsize: number,
 *   ePhnum: number,
 *   eShentsize: number,
 *   eShnum: number,
 *   appImageType2Magic: boolean,
 * }}
 */
function parseElf64Header(buf) {
  if (buf.length < 64) {
    return {
      validElf64Lsb: false,
      osAbi: -1,
      eType: 0,
      eMachine: 0,
      eVersion: 0,
      eEntry: 0n,
      ePhoff: 0n,
      eShoff: 0n,
      eFlags: 0,
      eEhsize: 0,
      ePhentsize: 0,
      ePhnum: 0,
      eShentsize: 0,
      eShnum: 0,
      appImageType2Magic: false,
    };
  }
  const validElf64Lsb =
    buf[0] === 0x7f &&
    buf[1] === 0x45 &&
    buf[2] === 0x4c &&
    buf[3] === 0x46 &&
    buf[4] === 0x02 &&
    buf[5] === 0x01 &&
    buf[6] === 0x01;
  const appImageType2Magic =
    buf[8] === 0x41 && buf[9] === 0x49 && buf[10] === 0x02;

  return {
    validElf64Lsb,
    osAbi: buf[7] ?? 0,
    eType: buf.readUInt16LE(16),
    eMachine: buf.readUInt16LE(18),
    eVersion: buf.readUInt32LE(20),
    eEntry: buf.readBigUInt64LE(24),
    ePhoff: buf.readBigUInt64LE(32),
    eShoff: buf.readBigUInt64LE(40),
    eFlags: buf.readUInt32LE(48),
    eEhsize: buf.readUInt16LE(52),
    ePhentsize: buf.readUInt16LE(54),
    ePhnum: buf.readUInt16LE(56),
    eShentsize: buf.readUInt16LE(58),
    eShnum: buf.readUInt16LE(60),
    appImageType2Magic,
  };
}

/**
 * Парсит TOML-файл через независимый системный оракул Python 3 (`tomllib`).
 * @param {string} tomlPath
 * @returns {Record<string, any>}
 */
function parseTomlWithPythonOracle(tomlPath) {
  const pyCode = [
    "import json, sys, tomllib",
    "with open(sys.argv[1], 'rb') as f:",
    "    data = tomllib.load(f)",
    "print(json.dumps(data))",
  ].join("\n");
  const res = spawnSync("python3", ["-c", pyCode, tomlPath], {
    encoding: "utf8",
  });
  assert.equal(
    res.status,
    0,
    `Ошибка парсинга TOML оракулом tomllib (${tomlPath}):\n${res.stderr}`,
  );
  return JSON.parse(res.stdout);
}

/**
 * Парсит файл формата `SHA256SUMS` в `Map<filename, sha256>`.
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

describe("TASK-990: Кросс-компиляция и пакеты для Linux ARM64 (aarch64, Raspberry Pi 5, Asahi)", () => {
  it("SUB-990-1: скрипт scripts/build-arm64.sh и конфигурация Cargo для aarch64-unknown-linux-gnu валидны и поддерживают профили generic, rpi5, asahi", () => {
    assert.ok(
      existsSync(BUILD_SCRIPT),
      `Скрипт сборки ARM64 должен существовать: ${BUILD_SCRIPT}`,
    );
    const scriptStat = statSync(BUILD_SCRIPT);
    assert.ok(
      (scriptStat.mode & 0o111) !== 0,
      "scripts/build-arm64.sh должен иметь права на исполнение (chmod +x)",
    );

    // Проверка синтаксиса bash -n
    const syntaxCheck = spawnSync("bash", ["-n", BUILD_SCRIPT], {
      encoding: "utf8",
    });
    assert.equal(
      syntaxCheck.status,
      0,
      `Синтаксическая ошибка в scripts/build-arm64.sh: ${syntaxCheck.stderr}`,
    );

    // Проверка сгенерированных файлов конфигурации Cargo для aarch64-unknown-linux-gnu
    for (const cfgPath of [CARGO_CONFIG_DIST_PATH, CARGO_CONFIG_PKG_PATH]) {
      assert.ok(
        existsSync(cfgPath),
        `Конфигурация кросс-компиляции Cargo должна существовать: ${cfgPath}`,
      );
      const parsed = parseTomlWithPythonOracle(cfgPath);
      assert.equal(
        parsed.build?.target,
        "aarch64-unknown-linux-gnu",
        "Секция [build] должна задавать target = 'aarch64-unknown-linux-gnu'",
      );

      const targetCfg = parsed.target?.["aarch64-unknown-linux-gnu"];
      assert.ok(
        targetCfg && typeof targetCfg === "object",
        "Должна присутствовать секция [target.aarch64-unknown-linux-gnu]",
      );
      assert.equal(targetCfg.linker, "clang");
      assert.equal(targetCfg.ar, "llvm-ar");
      assert.ok(
        Array.isArray(targetCfg.rustflags),
        "rustflags должен быть массивом строк",
      );
      const flagsJoined = targetCfg.rustflags.join(" ");
      assert.match(flagsJoined, /--target=aarch64-unknown-linux-gnu/);
      assert.match(flagsJoined, /-fuse-ld=lld/);
      assert.match(flagsJoined, /max-page-size=16384/);
      assert.match(flagsJoined, /\+neon/);
      assert.match(flagsJoined, /\+crc/);
      assert.match(flagsJoined, /\+aes/);
      assert.match(flagsJoined, /\+sha2/);

      const envCfg = parsed.env;
      assert.ok(envCfg && typeof envCfg === "object", "Должна присутствовать секция [env]");
      assert.equal(envCfg.CC_aarch64_unknown_linux_gnu, "clang");
      assert.equal(envCfg.CXX_aarch64_unknown_linux_gnu, "clang++");
      assert.equal(envCfg.AR_aarch64_unknown_linux_gnu, "llvm-ar");
      assert.equal(envCfg.PKG_CONFIG_ALLOW_CROSS, "1");
      assert.equal(envCfg.PKG_CONFIG_SYSROOT_DIR, "/usr/aarch64-linux-gnu");
    }
  });

  it("SUB-990-2: генерирует валидный 64-битный ELF aarch64 (EM_AARCH64=183, 16K page alignment, Full RELRO, stripped) по оракулам readelf и file", () => {
    assert.ok(
      existsSync(ARM64_BIN_PATH),
      `Бинарник ARM64 должен существовать: ${ARM64_BIN_PATH}`,
    );
    const binStat = statSync(ARM64_BIN_PATH);
    assert.ok(
      (binStat.mode & 0o111) !== 0,
      "rust/dist/discord-gpui-aarch64 должен иметь права на исполнение (0755)",
    );
    assert.ok(
      binStat.size >= 4096,
      `Размер ELF64 aarch64 (${binStat.size} байт) должен быть не менее 4096 байт`,
    );

    // 1. Независимый бинарный оракул 64-байтного заголовка ELF64
    const binBuf = readFileSync(ARM64_BIN_PATH);
    const hdr = parseElf64Header(binBuf);
    assert.equal(hdr.validElf64Lsb, true, "Должен быть валидный заголовок ELF64 LSB (\\x7fELF\\x02\\x01\\x01)");
    assert.ok(
      hdr.eType === 2 || hdr.eType === 3,
      `Тип ELF e_type должен быть ET_EXEC (2) или ET_DYN PIE (3), получено: ${hdr.eType}`,
    );
    assert.equal(
      hdr.eMachine,
      EM_AARCH64,
      `Архитектура ELF e_machine должна быть EM_AARCH64 (183 / 0xb7), получено: ${hdr.eMachine}`,
    );
    assert.equal(hdr.eEhsize, 64, "Размер заголовка ELF64 должен быть 64 байта");
    assert.equal(hdr.ePhentsize, 56, "Размер записи Program Header в ELF64 должен быть 56 байт");
    assert.ok(hdr.ePhnum >= 6, "ELF64 должен содержать программные сегменты (PT_PHDR, PT_INTERP, PT_LOAD, PT_DYNAMIC, PT_GNU_RELRO, PT_GNU_STACK)");

    // 2. Независимый системный оракул file
    const fileRes = spawnSync("file", [ARM64_BIN_PATH], {
      env: { ...process.env, LC_ALL: "C" },
      encoding: "utf8",
    });
    assert.equal(fileRes.status, 0);
    assert.match(fileRes.stdout, /ELF 64-bit LSB (pie )?executable, ARM aarch64/);
    assert.match(fileRes.stdout, /interpreter \/lib\/ld-linux-aarch64\.so\.1/);
    assert.match(fileRes.stdout, /BuildID\[sha1\]=[0-9a-f]{40}/);
    assert.match(fileRes.stdout, /\bstripped\b/);
    assert.doesNotMatch(fileRes.stdout, /not stripped/);

    // 3. Независимый системный оракул readelf (-h -l -d -n)
    const readelfRes = spawnSync(
      "readelf",
      ["-h", "-l", "-d", "-n", ARM64_BIN_PATH],
      {
        env: { ...process.env, LC_ALL: "C" },
        encoding: "utf8",
      },
    );
    assert.equal(readelfRes.status, 0, `readelf завершился с ошибкой: ${readelfRes.stderr}`);
    assert.match(readelfRes.stdout, /Class:\s+ELF64/);
    assert.match(readelfRes.stdout, /Machine:\s+AArch64/);
    assert.match(
      readelfRes.stdout,
      /Requesting program interpreter:\s+\/lib\/ld-linux-aarch64\.so\.1/,
    );
    // Проверка выравнивания сегментов PT_LOAD 0x4000 (16 КБ страницы памяти для Raspberry Pi 5 и Apple Silicon Asahi Linux)
    assert.match(
      readelfRes.stdout,
      /LOAD\s+0x[0-9a-f]+\s+0x[0-9a-f]+\s+0x[0-9a-f]+\s+0x[0-9a-f]+\s+0x[0-9a-f]+\s+[RWE ]+\s+0x4000/,
      "Сегменты PT_LOAD должны иметь выравнивание 0x4000 (16 КБ) для совместимости с Raspberry Pi 5 и Asahi Linux",
    );
    assert.match(readelfRes.stdout, /GNU_RELRO/, "Должен присутствовать сегмент защиты памяти GNU_RELRO");
    assert.match(readelfRes.stdout, /BIND_NOW/, "Должен быть активен флаг Full RELRO (BIND_NOW)");
    assert.match(readelfRes.stdout, /GNU_STACK/, "Должен присутствовать сегмент GNU_STACK");
    assert.match(readelfRes.stdout, /NT_GNU_BUILD_ID/, "Должна присутствовать секция NT_GNU_BUILD_ID");

    // 4. Проверка встроенного контракта метаданных и маркера GPUI_SMOKE_OK внутри бинарника
    const binText = binBuf.toString("utf8");
    assert.match(binText, /GPUI_SMOKE_OK \{/);
    assert.match(binText, /aarch64-unknown-linux-gnu/);
  });

  it("SUB-990-3: собирает и верифицирует ARM64 пакеты .tar.gz, .deb (Architecture: arm64) и Type 2 .AppImage (EM_AARCH64)", async () => {
    const {
      inspectDebArchive,
      parseDebianControl,
      validateDebianControl,
    } = await import("../../packaging/deb/deb-package.mjs");

    assert.ok(existsSync(ARM64_TARBALL_PATH), `Архив .tar.gz должен существовать: ${ARM64_TARBALL_PATH}`);
    assert.ok(existsSync(ARM64_DEB_PATH), `Пакет .deb должен существовать: ${ARM64_DEB_PATH}`);
    assert.ok(existsSync(ARM64_APPIMAGE_PATH), `Пакет .AppImage должен существовать: ${ARM64_APPIMAGE_PATH}`);

    const expectedBinSha = sha256File(ARM64_BIN_PATH);

    // 1. Инспекция и распаковка ARM64 .tar.gz
    const tmp = mkdtempSync(join(tmpdir(), "arm64-pkg-990-"));
    try {
      const tarDir = join(tmp, "tar-extract");
      spawnSync("mkdir", ["-p", tarDir]);
      const tarRes = spawnSync("tar", ["-xzf", ARM64_TARBALL_PATH, "-C", tarDir], {
        encoding: "utf8",
      });
      assert.equal(tarRes.status, 0, `Ошибка распаковки ${ARM64_TARBALL_NAME}: ${tarRes.stderr}`);

      const listRes = spawnSync("tar", ["-tzf", ARM64_TARBALL_PATH], {
        encoding: "utf8",
      });
      assert.equal(listRes.status, 0);
      const entries = listRes.stdout
        .split(/\r?\n/)
        .map((s) => s.trim())
        .filter(Boolean);
      const hasEntry = (/** @type {string} */ suffix) =>
        entries.some((e) => e === suffix || e.endsWith(`/${suffix}`));

      for (const reqFile of [
        "discord-gpui",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "sbom.spdx.json",
        "BETA-TESTING-GUIDE.md",
        "RELEASE-NOTES-v1.0.0-beta.1.md",
        "cargo-config-aarch64.toml",
      ]) {
        assert.ok(
          hasEntry(reqFile),
          `В архиве ${ARM64_TARBALL_NAME} должен присутствовать файл ${reqFile}`,
        );
      }

      const binEntry = entries.find((e) => e === "discord-gpui" || e.endsWith("/discord-gpui"));
      assert.ok(binEntry);
      const extractedTarBin = join(tarDir, binEntry);
      assert.equal(
        sha256File(extractedTarBin),
        expectedBinSha,
        "SHA-256 бинарника внутри ARM64 .tar.gz должен побайтово совпадать с rust/dist/discord-gpui-aarch64",
      );
      assert.equal(parseElf64Header(readFileSync(extractedTarBin)).eMachine, EM_AARCH64);

      // 2. Инспекция и распаковка ARM64 .deb пакета (discord-gpui_1.0.0-beta.1_arm64.deb)
      const debInspection = inspectDebArchive(ARM64_DEB_PATH);
      assert.equal(
        debInspection.valid,
        true,
        `Структура .deb пакета должна быть валидной: ${debInspection.errors.join("; ")}`,
      );
      assert.equal(debInspection.debianBinaryVersion, "2.0");

      const debExtractDir = join(tmp, "deb-extract");
      const debCtrlDir = join(debExtractDir, "control");
      const debDataDir = join(debExtractDir, "data");
      spawnSync("mkdir", ["-p", debCtrlDir, debDataDir]);

      assert.equal(
        spawnSync("ar", ["x", ARM64_DEB_PATH], { cwd: debExtractDir, encoding: "utf8" }).status,
        0,
      );
      assert.equal(
        spawnSync("tar", ["-xzf", join(debExtractDir, "control.tar.gz"), "-C", debCtrlDir], {
          encoding: "utf8",
        }).status,
        0,
      );
      assert.equal(
        spawnSync("tar", ["-xzf", join(debExtractDir, "data.tar.gz"), "-C", debDataDir], {
          encoding: "utf8",
        }).status,
        0,
      );

      const controlText = readFileSync(join(debCtrlDir, "control"), "utf8");
      const controlValidation = validateDebianControl(controlText);
      assert.equal(
        controlValidation.valid,
        true,
        `DEBIAN/control в arm64.deb должен быть валидным: ${controlValidation.errors.join("; ")}`,
      );
      const parsedControl = parseDebianControl(controlText);
      assert.equal(parsedControl.fields.get("Package"), "discord-gpui");
      assert.equal(
        parsedControl.fields.get("Architecture"),
        "arm64",
        "В DEBIAN/control пакета ARM64 должно быть поле Architecture: arm64",
      );
      assert.match(parsedControl.fields.get("Depends") ?? "", /libc6 \(>= 2\.31\)/);
      assert.match(parsedControl.fields.get("Depends") ?? "", /libpipewire-0\.3-0/);

      // Независимый системный оракул awk для проверки Architecture: arm64 в control
      const awkCheck = spawnSync(
        "awk",
        ["-F: ", "/^Architecture:/ { print $2 }", join(debCtrlDir, "control")],
        { encoding: "utf8" },
      );
      assert.equal(awkCheck.stdout.trim(), "arm64");

      const debBinPath = join(debDataDir, "usr", "bin", "discord-gpui");
      assert.ok(existsSync(debBinPath), "В data.tar.gz должен присутствовать usr/bin/discord-gpui");
      assert.equal(
        sha256File(debBinPath),
        expectedBinSha,
        "SHA-256 бинарника usr/bin/discord-gpui внутри arm64.deb должен совпадать с discord-gpui-aarch64",
      );
      const md5sumsText = readFileSync(join(debCtrlDir, "md5sums"), "utf8");
      const expectedDebBinMd5 = createHash("md5").update(readFileSync(debBinPath)).digest("hex");
      assert.ok(
        md5sumsText.includes(`${expectedDebBinMd5}  usr/bin/discord-gpui`),
        "DEBIAN/md5sums должен содержать точный MD5 бинарника usr/bin/discord-gpui",
      );

      // 3. Инспекция и распаковка Type 2 ARM64 .AppImage
      const appImageStat = statSync(ARM64_APPIMAGE_PATH);
      assert.ok(
        (appImageStat.mode & 0o111) !== 0,
        "Собранный ARM64 .AppImage должен иметь права на исполнение (0755)",
      );
      const appImageBuf = readFileSync(ARM64_APPIMAGE_PATH);
      const appHdr = parseElf64Header(appImageBuf);
      assert.equal(appHdr.validElf64Lsb, true, "Заголовок ARM64 AppImage должен быть ELF64 LSB");
      assert.equal(
        appHdr.appImageType2Magic,
        true,
        "Заголовок ARM64 AppImage должен содержать сигнатуру Type 2 'AI\\x02' в байтах 8..11",
      );
      assert.equal(
        appHdr.eMachine,
        EM_AARCH64,
        "Рантайм ARM64 AppImage должен быть скомпилирован под EM_AARCH64 (183)",
      );

      // Проверка 64-байтного трейлера AI2PAYLOAD и распаковка встроенного AppDir
      assert.ok(appImageBuf.length > 4096 + 64);
      const trailerBuf = appImageBuf.subarray(appImageBuf.length - 64);
      assert.equal(
        trailerBuf.subarray(0, 12).toString("ascii"),
        "AI2PAYLOAD\0\0",
        "Трейлер ARM64 AppImage должен содержать сигнатуру AI2PAYLOAD",
      );
      const payloadOffset = Number(trailerBuf.readBigUInt64LE(12));
      const payloadSize = Number(trailerBuf.readBigUInt64LE(20));
      assert.ok(
        payloadOffset >= 4096 && payloadOffset % 4096 === 0,
        `Смещение образа AppDir (${payloadOffset}) должно быть кратно 4096`,
      );
      assert.ok(payloadSize > 0 && payloadOffset + payloadSize + 64 === appImageBuf.length);

      const payloadSlice = appImageBuf.subarray(payloadOffset, payloadOffset + payloadSize);
      const payloadTarPath = join(tmp, "appimage-payload.tar.gz");
      const appDirExtract = join(tmp, "appdir-extract");
      writeFileSync(payloadTarPath, payloadSlice);
      spawnSync("mkdir", ["-p", appDirExtract]);
      assert.equal(
        spawnSync("tar", ["-xzf", payloadTarPath, "-C", appDirExtract], { encoding: "utf8" }).status,
        0,
      );
      const appDirBin = join(appDirExtract, "usr", "bin", "discord-gpui");
      assert.ok(existsSync(appDirBin), "Внутри AppDir должен присутствовать usr/bin/discord-gpui");
      assert.equal(
        sha256File(appDirBin),
        expectedBinSha,
        "Бинарник внутри ARM64 AppImage должен побайтово совпадать с discord-gpui-aarch64",
      );
      assert.ok(existsSync(join(appDirExtract, "AppRun")));
      assert.ok(existsSync(join(appDirExtract, "discord-gpui.desktop")));
      assert.ok(existsSync(join(appDirExtract, "usr", "lib", "bundled-deps.json")));
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-990-4: контрольные суммы всех ARM64 артефактов записаны в rust/dist/SHA256SUMS и проходят sha256sum -c и --verify", () => {
    assert.ok(existsSync(SHA256SUMS_PATH), "Файл rust/dist/SHA256SUMS должен существовать");
    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));

    /** @type {Array<[string, string]>} */
    const expectedEntries = [
      [ARM64_BIN_NAME, ARM64_BIN_PATH],
      [ARM64_TARBALL_NAME, ARM64_TARBALL_PATH],
      [ARM64_DEB_NAME, ARM64_DEB_PATH],
      [ARM64_APPIMAGE_NAME, ARM64_APPIMAGE_PATH],
      ["discord-gpui", join(DIST_DIR, "discord-gpui")],
      ["discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz", join(DIST_DIR, "discord-gpui-v1.0.0-beta.1-linux-x86_64.tar.gz")],
      ["sbom.spdx.json", join(DIST_DIR, "sbom.spdx.json")],
    ];
    for (const [name, fullPath] of expectedEntries) {
      assert.ok(
        sums.has(name),
        `В rust/dist/SHA256SUMS должна присутствовать запись для ${name}`,
      );
      assert.equal(
        sums.get(name),
        sha256File(fullPath),
        `Хеш ${name} в rust/dist/SHA256SUMS должен совпадать с фактическим SHA-256 файла`,
      );
    }

    // Независимый системный оракул: sha256sum -c SHA256SUMS в каталоге rust/dist/
    const shaCheck = spawnSync("sha256sum", ["-c", "SHA256SUMS"], {
      cwd: DIST_DIR,
      encoding: "utf8",
    });
    assert.equal(
      shaCheck.status,
      0,
      `sha256sum -c SHA256SUMS завершился с ошибкой:\nstdout: ${shaCheck.stdout}\nstderr: ${shaCheck.stderr}`,
    );

    // Проверка режима --verify скрипта scripts/build-arm64.sh с генерацией JSON-отчёта
    const tmp = mkdtempSync(join(tmpdir(), "arm64-verify-990-"));
    try {
      const reportPath = join(tmp, "verify-report.json");
      const verifyRun = spawnSync(
        "bash",
        [
          BUILD_SCRIPT,
          "--verify",
          "--output",
          DIST_DIR,
          "--report-json",
          reportPath,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        verifyRun.status,
        0,
        `scripts/build-arm64.sh --verify завершился с ошибкой:\nstdout: ${verifyRun.stdout}\nstderr: ${verifyRun.stderr}`,
      );
      assert.ok(existsSync(reportPath), "Файл --report-json должен быть создан");
      const report = JSON.parse(readFileSync(reportPath, "utf8"));
      assert.equal(report.task_id, "TASK-990");
      assert.equal(report.status, "passed");
      assert.equal(report.target, "aarch64-unknown-linux-gnu");
      assert.equal(report.elf_machine, "EM_AARCH64");
      assert.equal(report.elf_machine_id, EM_AARCH64);
      assert.equal(report.page_size_bytes, 16384);
      assert.equal(report.artifacts?.binary?.sha256, sha256File(ARM64_BIN_PATH));
      assert.equal(report.artifacts?.tarball?.sha256, sha256File(ARM64_TARBALL_PATH));
      assert.equal(report.artifacts?.deb?.sha256, sha256File(ARM64_DEB_PATH));
      assert.equal(report.artifacts?.appimage?.sha256, sha256File(ARM64_APPIMAGE_PATH));
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("SUB-990-5: проверяет изолированную сборку профилей rpi5/asahi, негативные/граничные сценарии CLI (коды 0/1/2), повреждение артефактов и маскирование секретов", () => {
    const tmp = mkdtempSync(join(tmpdir(), "arm64-cli-990-"));
    try {
      // 1. Справка --help -> код 0
      const helpRun = spawnSync("bash", [BUILD_SCRIPT, "--help"], {
        cwd: ROOT,
        encoding: "utf8",
      });
      assert.equal(helpRun.status, 0);
      assert.match(helpRun.stdout, /Использование:\s*scripts\/build-arm64\.sh/);

      // 2. Режим --dry-run не создаёт файлов на диске -> код 0
      const dryDir = join(tmp, "dry-dist");
      const dryRun = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--dry-run", "--output", dryDir, "--profile", "asahi"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(dryRun.status, 0);
      assert.match(dryRun.stdout, /aarch64-unknown-linux-gnu/);
      assert.equal(
        existsSync(dryDir),
        false,
        "В режиме --dry-run выходной каталог и файлы не должны создаваться",
      );

      // 3. Изолированная сборка с профилем rpi5 (Raspberry Pi 5 Cortex-A76) и проверка маскирования токенов
      const rpiDist = join(tmp, "rpi5-dist");
      const rpiReportPath = join(tmp, "rpi5-report.json");
      const fakeDiscordToken = "mfa.SecretTokenValueForArm64Test123456789";
      const fakeGhToken = "ghp_1234567890abcdefghijklmnopqrstuvwxyzABCD";

      const rpiBuild = spawnSync(
        "bash",
        [
          BUILD_SCRIPT,
          "--output",
          rpiDist,
          "--version",
          "1.0.0-rc.1",
          "--profile",
          "rpi5",
          "--report-json",
          rpiReportPath,
        ],
        {
          cwd: ROOT,
          env: {
            ...process.env,
            DISCORD_TOKEN: fakeDiscordToken,
            GITHUB_TOKEN: fakeGhToken,
          },
          encoding: "utf8",
        },
      );
      assert.equal(
        rpiBuild.status,
        0,
        `Сборка профиля rpi5 упала:\nstdout: ${rpiBuild.stdout}\nstderr: ${rpiBuild.stderr}`,
      );

      // Проверка отсутствия утечки секретов в логах и отчёте
      const combinedOutput = `${rpiBuild.stdout}\n${rpiBuild.stderr}\n${readFileSync(rpiReportPath, "utf8")}`;
      assert.ok(
        !combinedOutput.includes(fakeDiscordToken) && !combinedOutput.includes(fakeGhToken),
        "Секретные токены из окружения должны быть замаскированы и не попадать в логи или JSON-отчёт",
      );

      const rpiCargoToml = parseTomlWithPythonOracle(join(rpiDist, "cargo-config-aarch64.toml"));
      const rpiFlags = rpiCargoToml.target["aarch64-unknown-linux-gnu"].rustflags.join(" ");
      assert.match(
        rpiFlags,
        /target-cpu=cortex-a76/,
        "Профиль rpi5 должен настраивать target-cpu=cortex-a76 для Raspberry Pi 5",
      );
      assert.match(
        rpiFlags,
        /max-page-size=16384/,
        "Профиль rpi5 должен сохранять выравнивание страниц max-page-size=16384",
      );

      // Проверка профиля asahi (Apple Silicon M1/M2)
      const asahiCargoPath = join(tmp, "asahi-cargo.toml");
      const asahiDry = spawnSync(
        "bash",
        [
          BUILD_SCRIPT,
          "--output",
          rpiDist,
          "--version",
          "1.0.0-rc.1",
          "--profile",
          "asahi",
          "--cargo-config",
          asahiCargoPath,
        ],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(asahiDry.status, 0);
      const asahiCargoToml = parseTomlWithPythonOracle(asahiCargoPath);
      const asahiFlags = asahiCargoToml.target["aarch64-unknown-linux-gnu"].rustflags.join(" ");
      assert.match(
        asahiFlags,
        /target-cpu=apple-m1/,
        "Профиль asahi должен настраивать target-cpu=apple-m1 для Apple Silicon Asahi Linux",
      );

      // 4. Негативный сценарий: повреждение байтов в архиве роняет --verify и sha256sum -c с кодом 1
      const rpiTarPath = join(rpiDist, "discord-gpui-v1.0.0-rc.1-linux-aarch64.tar.gz");
      const origTarBytes = readFileSync(rpiTarPath);
      const corruptedTarBytes = Buffer.from(origTarBytes);
      const corruptIdx = corruptedTarBytes.length - 10;
      corruptedTarBytes.writeUInt8((corruptedTarBytes.readUInt8(corruptIdx) ^ 0xff) & 0xff, corruptIdx);
      writeFileSync(rpiTarPath, corruptedTarBytes);

      const badVerifySha = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--verify", "--output", rpiDist, "--version", "1.0.0-rc.1"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        badVerifySha.status,
        1,
        "При несовпадении SHA256SUMS режим --verify должен завершаться с кодом 1",
      );
      writeFileSync(rpiTarPath, origTarBytes);

      // 5. Негативный сценарий: подмена бинарника на x86_64 ELF (e_machine = 62) должна отвергаться с кодом 1
      const rpiBinPath = join(rpiDist, "discord-gpui-aarch64");
      const origBinBytes = readFileSync(rpiBinPath);
      const x86FakeBytes = Buffer.from(origBinBytes);
      x86FakeBytes.writeUInt16LE(62, 18); // EM_X86_64 = 62 вместо EM_AARCH64 = 183
      writeFileSync(rpiBinPath, x86FakeBytes);

      const badArchVerify = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--verify", "--output", rpiDist, "--version", "1.0.0-rc.1"],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        badArchVerify.status,
        1,
        "При подмене e_machine на x86_64 (62) режим --verify обязан завершаться с кодом 1",
      );
      writeFileSync(rpiBinPath, origBinBytes);

      // 6. Негативные аргументы CLI -> код 1
      for (const badArgs of [
        ["--unknown-flag"],
        ["--version", "not_a_semver!"],
        ["--target", "x86_64-unknown-linux-gnu"],
        ["--profile", "unknown_cpu_profile"],
        ["--output"],
      ]) {
        const res = spawnSync("bash", [BUILD_SCRIPT, ...badArgs], {
          cwd: ROOT,
          encoding: "utf8",
        });
        assert.equal(
          res.status,
          1,
          `Ожидался код выхода 1 для аргументов ${JSON.stringify(badArgs)}, получено: ${res.status}`,
        );
      }

      // 7. Отсутствие артефактов при --verify -> код 2 (среда/файлы недоступны)
      const emptyVerifyDir = join(tmp, "empty-verify");
      spawnSync("mkdir", ["-p", emptyVerifyDir]);
      const missingRes = spawnSync(
        "bash",
        [BUILD_SCRIPT, "--verify", "--output", emptyVerifyDir],
        { cwd: ROOT, encoding: "utf8" },
      );
      assert.equal(
        missingRes.status,
        2,
        "При отсутствии артефактов в каталоге --verify должен возвращать код 2",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
