// @ts-check
/**
 * Контрактные и модульные тесты сборок и пакетов для Windows (MSI/Portable) и macOS (DMG/Vibrancy) (TASK-1016):
 * - SUB-1016-0: Статический, синтаксический и компиляционный контракт `packaging/windows/wix.xml`,
 *   `packaging/macos/build-dmg.sh` и генерируемого Rust-модуля окна Vibrancy/Portable
 *   (`#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в prod-коде, функции < 50 строк,
 *   файлы < 800 строк, чистый `rustfmt --check --edition 2024`, `rustc --test --edition 2024 -D warnings`,
 *   `bash -n`, 0 утечек в `secret-scan.mjs`).
 * - SUB-1016-1: Сверка карточек `TASK-1016`, `TASK-1016A`, `TASK-1016B` в `waves/kanban-board.json`,
 *   дизайн-токенов (`#1e1f22`, `#2b2d31`, `#ffffff`, `#5865f2`, `#3f4147`), геометрии и интерактивных
 *   состояний, оракулов `docs/feature-backlog.md` (`R6-03`, `R6-04`) и `/home/sanya/re-deep/`.
 * - SUB-1016-2: Валидация `packaging/windows/wix.xml` независимым XML-оракулом Python 3 (`xml.etree.ElementTree`):
 *   `ProductCode`, `UpgradeCode`, `InstallScope="perUser"`, `AppUserModelId="com.discord_lab.DiscordGpui"`,
 *   оверлей-иконки таскбара (`ITaskbarList3::SetOverlayIcon`, `DISCORD_APP_BADGE_SET`), хоткеи `Alt+F4`/`Ctrl+Q`,
 *   протоколы `discord://` и `disclip://`, изоляция Portable-режима (`data/` рядом с `.exe`, 0 записей в реестр и `AppData`),
 *   и сверка `SHA-256` с `rust/dist/SHA256SUMS`.
 * - SUB-1016-3: Реальное исполнение `packaging/macos/build-dmg.sh` во временном каталоге: сборка `DiscordGPUI.app`
 *   (`Contents/Info.plist`, `Contents/MacOS/discord-gpui`, `Contents/Resources/entitlements.plist`, `DiscordGPUI.icns`),
 *   валидация `Info.plist` и `entitlements.plist` независимым оракулом Python 3 (`plistlib`), проверка `NSVisualEffectView`
 *   (`NSVisualEffectMaterialHUDWindow` / Dark HUD, `NSVisualEffectBlendingModeBehindWindow`), нативных traffic lights,
 *   обработчика `open-url` (`discord://`, `disclip://`), UDIF `koly`-трейлера `.dmg` и режима `--verify`.
 * - SUB-1016-4: Прогон встроенных Rust-тестов Vibrancy/Portable и 9 обязательных измерений `TEST-POLICY.md`
 *   (Snowflake `0`/`u64::MAX`, 2000 символов, 10 файлов/25 МБ, `offset=9975`, громкость `0..200%`, бейдж `99+`,
 *   коды `400/401/403/404/413/429/5xx`, `empty`/`skeleton`/`HTTP 202`, `AGE_GATED_SPACES`, fallback и маскирование токенов).
 * - SUB-1016-5: Мутационный гейт `5/5` (`100%` kill rate) для Rust-контракта и спецификаций WiX/DMG.
 */
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, describe, it } from "node:test";
import { fileURLToPath } from "node:url";

import {
  checkRustSourceSizeLimits,
  evaluateMutationGate,
} from "../../scripts/quality-gate.mjs";
import { scanTextForSecrets } from "../../scripts/secret-scan.mjs";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
const WIX_REL_PATH = "packaging/windows/wix.xml";
const DMG_REL_PATH = "packaging/macos/build-dmg.sh";
const WIX_PATH = join(ROOT, WIX_REL_PATH);
const DMG_SCRIPT_PATH = join(ROOT, DMG_REL_PATH);
const KANBAN_BOARD_PATH = join(ROOT, "waves", "kanban-board.json");
const FEATURE_BACKLOG_PATH = join(ROOT, "docs", "feature-backlog.md");
const SHA256SUMS_PATH = join(ROOT, "rust", "dist", "SHA256SUMS");
const RE_MAIN_ARCH_PATH = "/home/sanya/re-deep/main-arch.md";
const RE_BACKEND_IPC_PATH = "/home/sanya/re-deep/backend-ipc.json";

const VERSION = "1.0.0-beta.1";
const UUID_REGEX =
  /^\{[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}\}$/i;

/** @type {string} */
let workDir = "";
/** @type {string} */
let rustContractPath = "";
/** @type {string} */
let testBinPath = "";

/**
 * Парсит `rust/dist/SHA256SUMS` в `Map<filename, sha256>`.
 * @param {string} content
 * @returns {Map<string, string>}
 */
function parseSha256Sums(content) {
  /** @type {Map<string, string>} */
  const map = new Map();
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line) continue;
    const m = /^([a-f0-9]{64})\s+\*?(.+)$/.exec(line);
    assert.ok(m?.[1] && m?.[2], `Некорректная строка SHA256SUMS: ${line}`);
    map.set(m[2].trim(), m[1]);
  }
  return map;
}

/**
 * Парсит WiX XML через независимый системный оракул Python 3 (`xml.etree.ElementTree`).
 * @param {string} xmlText
 * @returns {Record<string, any>}
 */
function parseWixXmlWithPythonOracle(xmlText) {
  const py = [
    "import json, sys, xml.etree.ElementTree as ET",
    "root = ET.fromstring(sys.stdin.read())",
    "ns = {'w': 'http://schemas.microsoft.com/wix/2006/wi'}",
    "prod = root.find('w:Product', ns)",
    "pkg = prod.find('w:Package', ns) if prod is not None else None",
    "props = {p.attrib.get('Id'): p.attrib.get('Value') for p in (prod.findall('.//w:Property', ns) if prod is not None else [])}",
    "comps = {}",
    "for c in (prod.findall('.//w:Component', ns) if prod is not None else []):",
    "    cid = c.attrib.get('Id', '')",
    "    cond = c.find('w:Condition', ns)",
    "    regs = [r.attrib for r in c.findall('.//w:RegistryKey', ns)] + [r.attrib for r in c.findall('.//w:RegistryValue', ns)]",
    "    sc_props = {sp.attrib.get('Key'): sp.attrib.get('Value') for sp in c.findall('.//w:ShortcutProperty', ns)}",
    "    folders = [cf.attrib.get('Directory', '') for cf in c.findall('.//w:CreateFolder', ns)]",
    "    comps[cid] = {'guid': c.attrib.get('Guid', ''), 'condition': (cond.text.strip() if cond is not None and cond.text else ''), 'reg_count': len(regs), 'shortcut_props': sc_props, 'folders': folders}",
    "print(json.dumps({'root_tag': root.tag, 'product': prod.attrib if prod is not None else {}, 'package': pkg.attrib if pkg is not None else {}, 'properties': props, 'components': comps}))",
  ].join("\n");
  const res = spawnSync("python3", ["-c", py], {
    input: xmlText,
    encoding: "utf8",
  });
  if (res.status !== 0) {
    return { parseError: res.stderr || "XML parse error" };
  }
  return JSON.parse(res.stdout);
}

/**
 * Валидирует контракт `packaging/windows/wix.xml` против оракулов R6-03 и SHA256SUMS.
 * @param {string} xmlText
 * @param {Map<string, string>} sums
 * @returns {{ passed: boolean, errors: string[] }}
 */
export function validateWixManifestContract(xmlText, sums) {
  /** @type {string[]} */
  const errors = [];
  if (!xmlText || !xmlText.trim()) return { passed: false, errors: ["Пустой WiX XML манифест"] };
  const parsed = parseWixXmlWithPythonOracle(xmlText);
  if (parsed.parseError) return { passed: false, errors: [`XML syntax error: ${parsed.parseError}`] };
  const prod = parsed.product ?? {};
  const pkg = parsed.package ?? {};
  const props = parsed.properties ?? {};
  const comps = parsed.components ?? {};

  if (!UUID_REGEX.test(prod.Id ?? "")) errors.push("Невалидный ProductCode (Product@Id)");
  if (!UUID_REGEX.test(prod.UpgradeCode ?? "")) errors.push("Невалидный UpgradeCode");
  if (prod.Name !== "Discord GPUI") errors.push("Product@Name должен быть Discord GPUI");
  if (pkg.InstallScope !== "perUser" || pkg.InstallPrivileges !== "limited") {
    errors.push("Package должен задавать InstallScope=perUser и InstallPrivileges=limited");
  }
  if (props.APP_USER_MODEL_ID !== "com.discord_lab.DiscordGpui") {
    errors.push("APP_USER_MODEL_ID должен быть com.discord_lab.DiscordGpui");
  }
  if (props.PORTABLE_DATA_DIR !== "data") errors.push("PORTABLE_DATA_DIR должен быть равен data");
  if (!String(props.TASKBAR_OVERLAY_BADGE ?? "").includes("ITaskbarList3::SetOverlayIcon")) {
    errors.push("TASKBAR_OVERLAY_BADGE должен указывать ITaskbarList3::SetOverlayIcon");
  }
  if (props.WINDOWS_EXIT_HOTKEYS !== "Alt+F4;Ctrl+Q") errors.push("WINDOWS_EXIT_HOTKEYS должен содержать Alt+F4;Ctrl+Q");
  if (props.BINARY_SHA256 !== sums.get("discord-gpui")) errors.push("BINARY_SHA256 не совпадает с rust/dist/SHA256SUMS");
  const portableComp = comps.PortableModeComponent;
  if (!portableComp || portableComp.condition !== 'PORTABLE_MODE = "1"') {
    errors.push('PortableModeComponent обязан иметь условие PORTABLE_MODE = "1"');
  } else if (portableComp.reg_count !== 0 || !portableComp.folders.includes("PortableDataDir")) {
    errors.push("PortableModeComponent не должен писать в реестр и обязан создавать PortableDataDir (data/)");
  }
  const protoComp = comps.ProtocolHandlerComponent;
  if (!protoComp || protoComp.condition !== 'PORTABLE_MODE = "0"' || protoComp.reg_count < 2) {
    errors.push('ProtocolHandlerComponent обязан быть ограничен PORTABLE_MODE = "0" и регистрировать схемы');
  }
  return { passed: errors.length === 0, errors };
}

/**
 * Парсит Apple `.plist` файл через системный оракул Python 3 (`plistlib`).
 * @param {string} plistPath
 * @returns {Record<string, any>}
 */
function parsePlistWithPythonOracle(plistPath) {
  const py = [
    "import json, plistlib, sys",
    "with open(sys.argv[1], 'rb') as f:",
    "    data = plistlib.load(f)",
    "print(json.dumps(data))",
  ].join("\n");
  const res = spawnSync("python3", ["-c", py, plistPath], { encoding: "utf8" });
  assert.equal(res.status, 0, `Ошибка plistlib (${plistPath}): ${res.stderr}`);
  return JSON.parse(res.stdout);
}

/**
 * Компилирует Rust-модуль через `rustc --test --edition 2024 -D warnings`.
 * @param {string} srcPath
 * @param {string} outBin
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function compileRustContract(srcPath, outBin) {
  const res = spawnSync(
    "rustc",
    ["--test", "--edition", "2024", "-D", "warnings", srcPath, "-o", outBin],
    { cwd: ROOT, encoding: "utf8", timeout: 30_000 },
  );
  return { status: res.status, stdout: res.stdout ?? "", stderr: res.stderr ?? "" };
}

/**
 * Запускает скомпилированный тестовый бинарник Rust.
 * @param {string} binPath
 * @param {string} [filter]
 * @returns {{ status: number | null, stdout: string, stderr: string }}
 */
function runRustContractTests(binPath, filter) {
  const args = filter ? [filter, "--nocapture"] : ["--nocapture"];
  const res = spawnSync(binPath, args, { cwd: ROOT, encoding: "utf8", timeout: 15_000 });
  return { status: res.status, stdout: res.stdout ?? "", stderr: res.stderr ?? "" };
}

describe("TASK-1016: Сборки и пакеты для Windows (MSI/Portable) и macOS (DMG/Vibrancy)", () => {
  before(() => {
    workDir = mkdtempSync(join(tmpdir(), "packaging-1016-"));
    rustContractPath = join(workDir, "vibrancy_window_contract.rs");
    testBinPath = join(workDir, "vibrancy_window_contract_test");
  });

  after(() => {
    if (workDir && existsSync(workDir)) {
      rmSync(workDir, { recursive: true, force: true });
    }
  });

  it("SUB-1016-0: файлы packaging/windows/wix.xml и packaging/macos/build-dmg.sh существуют, соблюдают лимиты (<50 строк/fn, <800 строк/файл), #![forbid(unsafe_code)], rustfmt 2024 и bash -n", () => {
    assert.ok(existsSync(WIX_PATH), `Обязан существовать ${WIX_REL_PATH}`);
    assert.ok(existsSync(DMG_SCRIPT_PATH), `Обязан существовать ${DMG_REL_PATH}`);
    assert.ok((statSync(DMG_SCRIPT_PATH).mode & 0o111) !== 0, "build-dmg.sh обязан быть исполняемым (+x)");

    const wixText = readFileSync(WIX_PATH, "utf8");
    const dmgText = readFileSync(DMG_SCRIPT_PATH, "utf8");
    assert.ok(wixText.split("\n").length < 800, "wix.xml обязан быть < 800 строк");
    assert.ok(dmgText.split("\n").length < 800, "build-dmg.sh обязан быть < 800 строк");
    assert.equal(spawnSync("bash", ["-n", DMG_SCRIPT_PATH], { encoding: "utf8" }).status, 0);
    assert.equal(scanTextForSecrets(wixText, WIX_REL_PATH).length, 0);
    assert.equal(scanTextForSecrets(dmgText, DMG_REL_PATH).length, 0);

    const emitRes = spawnSync("bash", [DMG_SCRIPT_PATH, "--emit-rust-contract", rustContractPath], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(emitRes.status, 0, `Ошибка --emit-rust-contract: ${emitRes.stderr}`);
    const rustSource = readFileSync(rustContractPath, "utf8");
    assert.match(rustSource, /^#!\[forbid\(unsafe_code\)\]/m);
    assert.doesNotMatch(rustSource.split("#[cfg(test)]")[0] ?? rustSource, /\.unwrap\(\)|\.expect\(/);

    const sizeCheck = checkRustSourceSizeLimits(DMG_REL_PATH, rustSource);
    assert.equal(sizeCheck.passed, true, `Лимит размера Rust-кода: ${sizeCheck.reason}`);
    const fmtRes = spawnSync("rustfmt", ["--check", "--edition", "2024", rustContractPath], {
      cwd: ROOT,
      encoding: "utf8",
    });
    assert.equal(fmtRes.status, 0, `Ошибка rustfmt: ${fmtRes.stderr || fmtRes.stdout}`);
    const compiled = compileRustContract(rustContractPath, testBinPath);
    assert.equal(compiled.status, 0, `Ошибка компиляции rustc:\n${compiled.stderr}`);
  });

  it("SUB-1016-1: сверяет карточки TASK-1016, TASK-1016A, TASK-1016B в waves/kanban-board.json, дизайн-токены и оракулы docs/feature-backlog.md (R6-03, R6-04) + RE-дамп", () => {
    const kanban = JSON.parse(readFileSync(KANBAN_BOARD_PATH, "utf8"));
    const cards = kanban.cards ?? [];
    const c1016 = cards.find((/** @type {any} */ c) => c.id === "TASK-1016");
    const c1016a = cards.find((/** @type {any} */ c) => c.id === "TASK-1016A");
    const c1016b = cards.find((/** @type {any} */ c) => c.id === "TASK-1016B");

    assert.ok(c1016 && c1016a && c1016b, "Карточки TASK-1016/1016A/1016B обязаны существовать");
    assert.equal(c1016.layout.container, "Cross-OS Packaging");
    assert.equal(c1016.layout.dimensions, "multi-platform");
    assert.equal(c1016a.layout.container, "Windows Package");
    assert.equal(c1016a.layout.dimensions, "MSI / EXE");
    assert.equal(c1016b.layout.container, "macOS DMG");
    assert.equal(c1016b.layout.dimensions, "App Bundle");

    for (const card of [c1016, c1016a, c1016b]) {
      assert.equal(card.design_tokens.background, "#1e1f22");
      assert.equal(card.design_tokens.surface, "#2b2d31");
      assert.equal(card.design_tokens.text_primary, "#ffffff");
      assert.equal(card.design_tokens.accent, "#5865f2");
      assert.equal(card.design_tokens.border, "#3f4147");
    }

    const backlog = readFileSync(FEATURE_BACKLOG_PATH, "utf8");
    assert.match(backlog, /R6-03.*Windows-specific.*portable mode.*appBadge\.ts/);
    assert.match(backlog, /R6-04.*macOS-specific.*Vibrancy.*traffic lights.*open-url.*entitlements/);

    if (existsSync(RE_MAIN_ARCH_PATH)) {
      const mainArch = readFileSync(RE_MAIN_ARCH_PATH, "utf8");
      assert.match(mainArch, /setAppUserModelId/);
      assert.match(mainArch, /open-url/);
      assert.match(mainArch, /disclip/);
    }
    if (existsSync(RE_BACKEND_IPC_PATH)) {
      const ipcRaw = readFileSync(RE_BACKEND_IPC_PATH, "utf8");
      assert.ok(ipcRaw.includes("DISCORD_APP_DOCK_SET_BADGE"));
      assert.ok(ipcRaw.includes("DISCORD_THUMBAR_BUTTONS_UPDATE"));
    }
  });

  it("SUB-1016-2: валидирует packaging/windows/wix.xml (ProductCode, UpgradeCode, оверлей-иконки таскбара, хоткеи Alt+F4/Ctrl+Q) и изолированную Portable-сборку в data/ без записи в реестр и AppData", () => {
    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const wixText = readFileSync(WIX_PATH, "utf8");
    const validation = validateWixManifestContract(wixText, sums);
    assert.equal(validation.passed, true, `Ошибки валидации wix.xml:\n${validation.errors.join("\n")}`);

    const portableDir = join(workDir, "win-portable");
    const stageRes = spawnSync(
      "bash",
      [DMG_SCRIPT_PATH, "--stage-portable", portableDir, "--version", VERSION],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(stageRes.status, 0, `Ошибка --stage-portable: ${stageRes.stderr}`);
    assert.ok(existsSync(join(portableDir, "discord-gpui.exe")));
    assert.ok(existsSync(join(portableDir, "data")));
    assert.ok(existsSync(join(portableDir, "portable.json")));

    const portableManifest = JSON.parse(readFileSync(join(portableDir, "portable.json"), "utf8"));
    assert.equal(portableManifest.portable, true);
    assert.equal(portableManifest.data_dir, "data");
    assert.equal(portableManifest.writes_appdata, false);
    assert.equal(portableManifest.writes_registry, false);
    assert.equal(portableManifest.app_user_model_id, "com.discord_lab.DiscordGpui");
  });

  it("SUB-1016-3: исполняет packaging/macos/build-dmg.sh, проверяет структуру DiscordGPUI.app (Info.plist, Contents/MacOS/, entitlements, Vibrancy Dark HUD, traffic lights, open-url) и DMG (UDIF koly + --verify)", () => {
    const macOutDir = join(workDir, "macos-dist");
    const buildRes = spawnSync(
      "bash",
      [DMG_SCRIPT_PATH, "--out-dir", macOutDir, "--version", VERSION, "--arch", "universal"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(buildRes.status, 0, `Ошибка сборки DMG:\n${buildRes.stderr}\n${buildRes.stdout}`);

    const appDir = join(macOutDir, "DiscordGPUI.app");
    const infoPlistPath = join(appDir, "Contents", "Info.plist");
    const macBinPath = join(appDir, "Contents", "MacOS", "discord-gpui");
    const entitlementsPath = join(appDir, "Contents", "Resources", "entitlements.plist");
    const icnsPath = join(appDir, "Contents", "Resources", "DiscordGPUI.icns");
    const dmgPath = join(macOutDir, `discord-gpui-v${VERSION}-macos-universal.dmg`);
    assert.ok(existsSync(infoPlistPath) && existsSync(macBinPath) && existsSync(entitlementsPath));
    assert.ok(existsSync(icnsPath) && existsSync(dmgPath));
    assert.equal(statSync(macBinPath).mode & 0o777, 0o755);

    const plist = parsePlistWithPythonOracle(infoPlistPath);
    assert.equal(plist.CFBundleName, "DiscordGPUI");
    assert.equal(plist.CFBundleIdentifier, "com.discord_lab.DiscordGpui");
    assert.equal(plist.CFBundleExecutable, "discord-gpui");
    assert.equal(plist.CFBundlePackageType, "APPL");
    assert.equal(plist.NSHighResolutionCapable, true);
    assert.equal(plist.NSRequiresAquaSystemAppearance, false);
    assert.equal(plist.DiscordVibrancyMaterial, "NSVisualEffectMaterialHUDWindow");
    assert.equal(plist.DiscordVibrancyBlendingMode, "NSVisualEffectBlendingModeBehindWindow");
    assert.deepEqual(plist.DiscordTrafficLightPosition, { x: 14, y: 14 });
    const schemes = (plist.CFBundleURLTypes ?? []).flatMap((/** @type {any} */ u) => u.CFBundleURLSchemes ?? []);
    assert.ok(schemes.includes("discord") && schemes.includes("disclip"));

    const ent = parsePlistWithPythonOracle(entitlementsPath);
    assert.equal(ent["com.apple.security.device.audio-input"], true);
    assert.equal(ent["com.apple.security.device.camera"], true);
    assert.equal(ent["com.apple.security.network.client"], true);
    assert.equal(ent["com.apple.security.cs.allow-jit"], false);
    const dmgBuf = readFileSync(dmgPath);
    const trailer = dmgBuf.subarray(dmgBuf.length - 512);
    assert.equal(trailer.subarray(0, 4).toString("ascii"), "koly");
    assert.equal(trailer.readUInt32BE(4), 4);
    assert.equal(trailer.readUInt32BE(8), 512);
    const verifyRes = spawnSync(
      "bash",
      [DMG_SCRIPT_PATH, "--out-dir", macOutDir, "--version", VERSION, "--arch", "universal", "--verify"],
      { cwd: ROOT, encoding: "utf8" },
    );
    assert.equal(verifyRes.status, 0, `Ошибка --verify: ${verifyRes.stderr}`);
  });

  it("SUB-1016-4: проходит встроенные Rust-тесты Vibrancy NSVisualEffectView, Windows MSI/Portable и всех 9 измерений TEST-POLICY.md", () => {
    const run = runRustContractTests(testBinPath);
    assert.equal(run.status, 0, `Сбой встроенных тестов Rust:\n${run.stderr}\n${run.stdout}`);
    assert.match(run.stdout, /test tests::test_windows_portable_and_taskbar_badge .* ok/);
    assert.match(run.stdout, /test tests::test_macos_vibrancy_and_open_url .* ok/);
    assert.match(run.stdout, /test tests::test_nine_mandatory_dimensions .* ok/);
  });

  it("SUB-1016-5: убивает 5/5 мутаций Rust-контракта и 5/5 мутаций WiX XML манифеста (100% mutation kill rate)", () => {
    const sums = parseSha256Sums(readFileSync(SHA256SUMS_PATH, "utf8"));
    const wixText = readFileSync(WIX_PATH, "utf8");
    const wixMutations = [
      (/** @type {string} */ s) => s.replace('InstallScope="perUser"', 'InstallScope="perMachine"'),
      (/** @type {string} */ s) => s.replace("com.discord_lab.DiscordGpui", "com.broken.AppId"),
      (/** @type {string} */ s) => s.replace('PORTABLE_MODE = "1"', 'PORTABLE_MODE = "0"'),
      (/** @type {string} */ s) => s.replace("ITaskbarList3::SetOverlayIcon", "NoOverlayBadge"),
      (/** @type {string} */ s) => s.replace(sums.get("discord-gpui") ?? "", "0".repeat(64)),
    ];
    for (const mutateWix of wixMutations) {
      assert.equal(validateWixManifestContract(mutateWix(wixText), sums).passed, false);
    }

    const rustSource = readFileSync(rustContractPath, "utf8");
    const mutationSpecs = [
      ["MUT-pkg-01", "PortableWritesAppData", "writes_appdata: false,", "writes_appdata: true,"],
      ["MUT-pkg-02", "Badge99PlusCapBroken", "if count > 99 {", "if count > 999 {"],
      ["MUT-pkg-03", "VibrancyDarkHudDisabled", "material: NsVisualEffectMaterial::DarkHud,", "material: NsVisualEffectMaterial::Titlebar,"],
      ["MUT-pkg-04", "OpenUrlSchemeBypassed", 'trimmed.starts_with("discord://") || trimmed.starts_with("disclip://")', 'trimmed.starts_with("http://")'],
      ["MUT-pkg-05", "SecretRedactionDisabled", "redact_secrets(raw_log)", "raw_log.to_string()"],
    ];
    const mutants = mutationSpecs.map(([id, operator, from, to]) => ({
      id: id ?? "",
      moduleName: "vibrancy_window_contract.rs",
      operator: operator ?? "",
      mutatedSource: rustSource.replace(from ?? "", to ?? ""),
    }));
    const gate = evaluateMutationGate(mutants, (mutant, idx) => {
      const mSrc = join(workDir, `mutant_${idx + 1}.rs`);
      const mBin = join(workDir, `mutant_${idx + 1}_bin`);
      writeFileSync(mSrc, mutant.mutatedSource, "utf8");
      if (compileRustContract(mSrc, mBin).status !== 0) return { killed: true, reason: "compilation_error" };
      if (runRustContractTests(mBin).status !== 0) return { killed: true, reason: "test_assertion_failed" };
      return { killed: false, reason: "mutant_survived" };
    });
    assert.equal(gate.passed, true, `Выжили мутанты: ${gate.survivors.join(", ")}`);
    assert.equal(gate.killed, 5);
    assert.equal(gate.killRate, 1.0);
  });
});
