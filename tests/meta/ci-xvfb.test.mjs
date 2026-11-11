// @ts-check
/**
 * Мета-тест TASK-621 (DevOps + G4 QA): автоматическое управление виртуальным дисплеем Xvfb
 * и авто-обнаружение браузера в `scripts/ci.sh` и `scripts/gate.mjs` для headless CI.
 *
 * Проверяемые инварианты:
 * 1. `hasActiveDisplay`: пустой/пробельный DISPLAY и локальный `:N` без сокета в X11-каталоге
 *    (включая отсутствие самого `/tmp/.X11-unix`) дают `false`; существующий сокет `XN` или
 *    удалённый дисплей `host:D.S` дают `true`.
 * 2. `findXvfbRun`: находит исполняемый `xvfb-run` через `XVFB_RUN_BIN`, `PATH` или стандартные
 *    пути; при `XVFB_RUN_BIN=none` или отсутствии бинарника возвращает `null`.
 * 3. `resolveBrowserBin`: соблюдает приоритет `env` → `known-path` → `path` → `flatpak` → `none`;
 *    при невалидном явном `CHROME_BIN`/`CHROME_PATH` (несуществующий файл или директория)
 *    возвращает `env-missing` без тихого отката на другой браузер (fail-closed).
 * 4. `displayPreflightNote`, `e2ePhaseEnv`, `browserPreflight`, `e2eFixturePhase`: корректно
 *    формируют диагностику, окружение `CHROME_BIN` и аргументы `xvfb-run -a -s <XVFB_SERVER_ARGS>`.
 * 5. `scripts/ci.sh`: содержит функции жизненного цикла Xvfb (`start_xvfb_display`, `cleanup_xvfb`,
 *    `run_with_xvfb`, `trap cleanup_xvfb EXIT`), `detect_browser`, `has_active_display` и корректно
 *    отрабатывает `--dry-run --scope <worker|release|qualify>` с активным и отсутствующим DISPLAY.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  accessSync, chmodSync, constants, existsSync, mkdirSync, mkdtempSync,
  readFileSync, rmSync, statSync, writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const gateSource = readFileSync(join(root, "scripts", "gate.mjs"), "utf8");
const ciScriptPath = join(root, "scripts", "ci.sh");
const ciSource = readFileSync(ciScriptPath, "utf8");

/**
 * Извлекает тело функции `function <name>(...) { ... }` из `scripts/gate.mjs`.
 * @param {string} source
 * @param {string} name
 * @returns {string}
 */
function extractFunction(source, name) {
  const start = source.indexOf(`function ${name}(`);
  assert.notEqual(start, -1, `в scripts/gate.mjs нет функции ${name}`);
  let parenDepth = 0;
  let bodyStart = -1;
  for (let index = source.indexOf("(", start); index < source.length; index += 1) {
    if (source[index] === "(") parenDepth += 1;
    else if (source[index] === ")") {
      parenDepth -= 1;
      if (parenDepth === 0) {
        bodyStart = source.indexOf("{", index);
        break;
      }
    }
  }
  assert.notEqual(bodyStart, -1, `${name}: не найдено тело функции`);
  let depth = 0;
  for (let index = bodyStart; index < source.length; index += 1) {
    if (source[index] === "{") depth += 1;
    else if (source[index] === "}") {
      depth -= 1;
      if (depth === 0) return source.slice(start, index + 1);
    }
  }
  return assert.fail(`${name}: непарные скобки`);
}

/**
 * Собирает изолированный модуль функций Xvfb и обнаружения браузера из `scripts/gate.mjs`.
 * @returns {{
 *   isExecutableFile: (filePath: string) => boolean,
 *   hasActiveDisplay: (env?: Record<string, string | undefined>, x11SocketDir?: string) => boolean,
 *   findXvfbRun: (env?: Record<string, string | undefined>, options?: Record<string, any>) => string | null,
 *   resolveBrowserBin: (env?: Record<string, string | undefined>, options?: Record<string, any>) => { bin: string | null, explicitMissing: string | null, source: string },
 *   displayPreflightNote: (env?: Record<string, string | undefined>, options?: Record<string, any>) => string,
 *   e2ePhaseEnv: (env?: Record<string, string | undefined>, options?: Record<string, any>) => Record<string, string>,
 *   browserPreflight: (env?: Record<string, string | undefined>, options?: Record<string, any>) => string[],
 * }}
 */
function loadGateXvfbHelpers() {
  const fnNames = [
    "runCommand",
    "isExecutableFile",
    "hasActiveDisplay",
    "findXvfbRun",
    "resolveBrowserBin",
    "displayPreflightNote",
    "e2ePhaseEnv",
    "browserPreflight",
  ];
  const bodies = fnNames.map((name) => extractFunction(gateSource, name)).join("\n\n");
  const factory = new Function(
    "deps",
    `
    const {
      spawnSync, accessSync, chmodSync, constants, existsSync,
      mkdirSync, statSync, writeFileSync, delimiter, join, root, gateDir,
    } = deps;
    const KNOWN_BROWSER_PATHS = [
      "/home/sanya/.local/bin/chromium",
      "/usr/bin/chromium",
      "/usr/bin/chromium-browser",
      "/usr/bin/google-chrome",
      "/usr/bin/google-chrome-stable",
      "/usr/bin/brave-browser",
      "/usr/bin/brave",
      "/usr/bin/microsoft-edge",
      "/usr/bin/microsoft-edge-stable",
      "/snap/bin/chromium",
      "/opt/google/chrome/chrome",
      "/opt/brave.com/brave/brave-browser",
    ];
    const BROWSER_BIN_NAMES = [
      "chromium",
      "chromium-browser",
      "google-chrome",
      "google-chrome-stable",
      "chrome",
      "brave-browser",
      "brave",
      "microsoft-edge",
      "microsoft-edge-stable",
    ];
    const FLATPAK_BROWSERS = [
      "com.brave.Browser",
      "org.chromium.Chromium",
      "com.google.Chrome",
    ];
    const KNOWN_XVFB_PATHS = [
      "/usr/bin/xvfb-run",
      "/usr/local/bin/xvfb-run",
      "/home/sanya/.local/bin/xvfb-run",
    ];
    const XVFB_SERVER_ARGS = "-screen 0 1280x720x24 -ac +extension GLX +render -noreset";
    ${bodies}
    return {
      isExecutableFile,
      hasActiveDisplay,
      findXvfbRun,
      resolveBrowserBin,
      displayPreflightNote,
      e2ePhaseEnv,
      browserPreflight,
    };
    `,
  );
  return factory({
    spawnSync,
    accessSync,
    chmodSync,
    constants,
    existsSync,
    mkdirSync,
    statSync,
    writeFileSync,
    delimiter,
    join,
    root,
    gateDir: join(root, ".gate"),
  });
}

/**
 * Создаёт временный исполняемый скрипт в указанной директории.
 * @param {string} dir
 * @param {string} name
 * @param {string} [body]
 * @returns {string}
 */
function makeFakeExecutable(dir, name, body = "#!/usr/bin/env bash\nexit 0\n") {
  const filePath = join(dir, name);
  writeFileSync(filePath, body, "utf8");
  chmodSync(filePath, 0o755);
  return filePath;
}

describe("TASK-621: scripts/gate.mjs — управление Xvfb и авто-обнаружение браузера", () => {
  it("hasActiveDisplay корректно проверяет пустой DISPLAY, локальный сокет X11 и удалённый дисплей", () => {
    const { hasActiveDisplay } = loadGateXvfbHelpers();
    const tmp = mkdtempSync(join(tmpdir(), "discord-lab-xvfb-disp-"));
    try {
      const x11Dir = join(tmp, ".X11-unix");
      const missingDir = join(tmp, "no-such-x11-dir");

      assert.equal(hasActiveDisplay({ DISPLAY: "" }, x11Dir), false, "пустой DISPLAY не активен");
      assert.equal(hasActiveDisplay({ DISPLAY: "   " }, x11Dir), false, "пробельный DISPLAY не активен");
      assert.equal(hasActiveDisplay({}, x11Dir), false, "незаданный DISPLAY не активен");

      // Даже если каталог /tmp/.X11-unix отсутствует, локальный :99 без сокета — не активен.
      assert.equal(
        hasActiveDisplay({ DISPLAY: ":99" }, missingDir),
        false,
        "локальный :99 при отсутствии каталога .X11-unix обязан возвращать false",
      );

      mkdirSync(x11Dir, { recursive: true });
      assert.equal(
        hasActiveDisplay({ DISPLAY: ":42" }, x11Dir),
        false,
        "локальный :42 без файла X42 не активен",
      );

      writeFileSync(join(x11Dir, "X42"), "");
      assert.equal(
        hasActiveDisplay({ DISPLAY: ":42" }, x11Dir),
        true,
        "локальный :42 при наличии X42 активен",
      );
      assert.equal(
        hasActiveDisplay({ DISPLAY: ":42.0" }, x11Dir),
        true,
        "локальный :42.0 при наличии X42 активен",
      );

      // Удалённый TCP-дисплей (host:D.S) не требует локального unix-сокета.
      assert.equal(
        hasActiveDisplay({ DISPLAY: "localhost:10.0" }, missingDir),
        true,
        "удалённый дисплей localhost:10.0 считается активным",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("findXvfbRun находит xvfb-run через XVFB_RUN_BIN, PATH и knownPaths, отвергая неисполняемые пути", () => {
    const { findXvfbRun } = loadGateXvfbHelpers();
    const tmp = mkdtempSync(join(tmpdir(), "discord-lab-xvfb-bin-"));
    try {
      const pathDir = join(tmp, "path-bin");
      const knownDir = join(tmp, "known-bin");
      mkdirSync(pathDir, { recursive: true });
      mkdirSync(knownDir, { recursive: true });

      assert.equal(
        findXvfbRun({ PATH: "", XVFB_RUN_BIN: "" }, { knownPaths: [] }),
        null,
        "при пустых PATH и knownPaths xvfb-run не должен находиться",
      );

      const knownXvfb = makeFakeExecutable(knownDir, "xvfb-run");
      assert.equal(
        findXvfbRun({ PATH: "" }, { knownPaths: [knownXvfb] }),
        knownXvfb,
        "xvfb-run должен находиться по knownPaths",
      );

      const pathXvfb = makeFakeExecutable(pathDir, "xvfb-run");
      assert.equal(
        findXvfbRun({ PATH: pathDir }, { knownPaths: [] }),
        pathXvfb,
        "xvfb-run должен находиться в PATH",
      );

      assert.equal(
        findXvfbRun({ PATH: pathDir, XVFB_RUN_BIN: "none" }, { knownPaths: [knownXvfb] }),
        null,
        "XVFB_RUN_BIN=none явно отключает xvfb-run",
      );

      assert.equal(
        findXvfbRun({ PATH: pathDir, XVFB_RUN_BIN: join(tmp, "missing-xvfb") }, { knownPaths: [knownXvfb] }),
        null,
        "несуществующий XVFB_RUN_BIN возвращает null",
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("resolveBrowserBin соблюдает приоритеты env -> known-path -> path -> flatpak -> none и fail-closed на битом CHROME_BIN", () => {
    const { resolveBrowserBin } = loadGateXvfbHelpers();
    const tmp = mkdtempSync(join(tmpdir(), "discord-lab-browser-"));
    try {
      const envDir = join(tmp, "env-bin");
      const knownDir = join(tmp, "known-bin");
      const pathDir = join(tmp, "path-bin");
      const flatpakDir = join(tmp, "flatpak-bin");
      const gateOut = join(tmp, "gate-out");
      mkdirSync(envDir, { recursive: true });
      mkdirSync(knownDir, { recursive: true });
      mkdirSync(pathDir, { recursive: true });
      mkdirSync(flatpakDir, { recursive: true });

      const envBrowser = makeFakeExecutable(envDir, "custom-chromium");
      const knownBrowser = makeFakeExecutable(knownDir, "chromium");
      const pathBrowser = makeFakeExecutable(pathDir, "brave-browser");

      // 1. Явный валидный CHROME_BIN побеждает все остальные источники.
      assert.deepEqual(
        resolveBrowserBin(
          { CHROME_BIN: envBrowser, PATH: pathDir },
          { knownPaths: [knownBrowser], allowFlatpak: false },
        ),
        { bin: envBrowser, explicitMissing: null, source: "env" },
      );

      // 2. Явный битый CHROME_BIN (или директория) возвращает env-missing и НЕ откатывается на knownPaths/PATH.
      const missingExplicit = join(tmp, "does-not-exist-chrome");
      assert.deepEqual(
        resolveBrowserBin(
          { CHROME_BIN: missingExplicit, PATH: pathDir },
          { knownPaths: [knownBrowser], allowFlatpak: false },
        ),
        { bin: null, explicitMissing: missingExplicit, source: "env-missing" },
      );
      assert.deepEqual(
        resolveBrowserBin(
          { CHROME_PATH: envDir, PATH: pathDir },
          { knownPaths: [knownBrowser], allowFlatpak: false },
        ),
        { bin: null, explicitMissing: envDir, source: "env-missing" },
      );

      // 3. При отсутствии CHROME_BIN проверяются knownPaths.
      assert.deepEqual(
        resolveBrowserBin(
          { CHROME_BIN: "", PATH: pathDir },
          { knownPaths: [join(tmp, "none"), knownBrowser], allowFlatpak: false },
        ),
        { bin: knownBrowser, explicitMissing: null, source: "known-path" },
      );

      // 4. При отсутствии в knownPaths ищется в PATH.
      assert.deepEqual(
        resolveBrowserBin(
          { CHROME_BIN: "", PATH: pathDir },
          { knownPaths: [join(tmp, "none")], allowFlatpak: false },
        ),
        { bin: pathBrowser, explicitMissing: null, source: "path" },
      );

      // 5. Резервное обнаружение через Flatpak генерирует исполняемую обёртку в gateDir.
      makeFakeExecutable(
        flatpakDir,
        "flatpak",
        '#!/usr/bin/env bash\nif [[ "$1" == "info" && "$2" == "org.chromium.Chromium" ]]; then exit 0; fi\nexit 1\n',
      );
      const flatpakResolved = resolveBrowserBin(
        { CHROME_BIN: "", PATH: flatpakDir },
        { knownPaths: [], allowFlatpak: true, gateDir: gateOut },
      );
      assert.equal(flatpakResolved.source, "flatpak:org.chromium.Chromium");
      assert.equal(flatpakResolved.explicitMissing, null);
      assert.equal(flatpakResolved.bin, join(gateOut, "flatpak-browser-wrapper.sh"));
      const wrapperText = readFileSync(String(flatpakResolved.bin), "utf8");
      assert.match(wrapperText, /exec flatpak run --filesystem=host org\.chromium\.Chromium "\$@"/);

      // 6. Полное отсутствие браузера.
      assert.deepEqual(
        resolveBrowserBin(
          { CHROME_BIN: "", PATH: "" },
          { knownPaths: [], allowFlatpak: false },
        ),
        { bin: null, explicitMissing: null, source: "none" },
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("displayPreflightNote, e2ePhaseEnv и browserPreflight формируют полную диагностику дисплея и браузера", () => {
    const { displayPreflightNote, e2ePhaseEnv, browserPreflight } = loadGateXvfbHelpers();
    const tmp = mkdtempSync(join(tmpdir(), "discord-lab-preflight-"));
    try {
      const x11Dir = join(tmp, ".X11-unix");
      mkdirSync(x11Dir, { recursive: true });
      writeFileSync(join(x11Dir, "X7"), "");
      const fakeBrowser = makeFakeExecutable(tmp, "chromium");
      const fakeXvfb = makeFakeExecutable(tmp, "xvfb-run");

      assert.equal(
        displayPreflightNote({ DISPLAY: ":7" }, { x11SocketDir: x11Dir, knownXvfbPaths: [] }),
        "дисплей: DISPLAY=:7",
      );
      assert.equal(
        displayPreflightNote(
          { DISPLAY: "", PATH: "" },
          { x11SocketDir: x11Dir, knownXvfbPaths: [fakeXvfb] },
        ),
        `дисплей: xvfb-run -a (${fakeXvfb}, DISPLAY не активен)`,
      );
      assert.equal(
        displayPreflightNote(
          { DISPLAY: "", PATH: "" },
          { x11SocketDir: x11Dir, knownXvfbPaths: [] },
        ),
        "дисплей: headless без DISPLAY (xvfb-run не найден, используется --headless=new)",
      );

      assert.deepEqual(
        e2ePhaseEnv(
          { CHROME_BIN: "", PATH: "" },
          { knownPaths: [fakeBrowser], allowFlatpak: false },
        ),
        { CHROME_BIN: fakeBrowser },
      );
      assert.deepEqual(
        e2ePhaseEnv(
          { CHROME_BIN: fakeBrowser, PATH: "" },
          { knownPaths: [fakeBrowser], allowFlatpak: false },
        ),
        {},
      );

      const okNotes = browserPreflight(
        { CHROME_BIN: "", DISPLAY: "", PATH: "" },
        {
          knownPaths: [fakeBrowser],
          allowFlatpak: false,
          x11SocketDir: x11Dir,
          knownXvfbPaths: [fakeXvfb],
        },
      );
      assert.equal(okNotes.length, 2);
      assert.match(okNotes[0] ?? "", /браузер: .*chromium \(known-path\)/);
      assert.match(okNotes[1] ?? "", /дисплей: xvfb-run -a/);

      const missingNotes = browserPreflight(
        { CHROME_BIN: "", DISPLAY: "", PATH: "" },
        {
          knownPaths: [],
          allowFlatpak: false,
          x11SocketDir: x11Dir,
          knownXvfbPaths: [],
        },
      );
      assert.match(missingNotes[0] ?? "", /^e2e_unmet: браузера нет/);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("e2eFixturePhase передаёт XVFB_SERVER_ARGS (-screen 0 1280x720x24) в xvfb-run -a -s", () => {
    assert.match(
      gateSource,
      /const XVFB_SERVER_ARGS = "-screen 0 1280x720x24 -ac \+extension GLX \+render -noreset";/,
      "в scripts/gate.mjs не объявлена константа XVFB_SERVER_ARGS для 1280x720x24",
    );
    const phaseFn = extractFunction(gateSource, "e2eFixturePhase");
    assert.match(phaseFn, /"-a",\s*"-s",\s*XVFB_SERVER_ARGS/, "e2eFixturePhase должна передавать -a -s XVFB_SERVER_ARGS в xvfb-run");
    assert.match(phaseFn, /failOnSkips:\s*true/, "e2eFixturePhase обязана включать failOnSkips");
    assert.match(phaseFn, /minPass:\s*1/, "e2eFixturePhase обязана требовать minPass: 1");
    assert.match(phaseFn, /--test-concurrency=1/, "e2eFixturePhase обязана запускать тесты последовательно");
  });
});

describe("TASK-621: scripts/ci.sh — управление Xvfb и авто-обнаружение браузера в CI", () => {
  it("scripts/ci.sh синтаксически корректен и содержит функции жизненного цикла Xvfb и браузера", () => {
    const syntax = spawnSync("bash", ["-n", ciScriptPath], { encoding: "utf8" });
    assert.equal(syntax.status, 0, `bash -n scripts/ci.sh: ${syntax.stderr}`);

    for (const token of [
      "XVFB_SERVER_ARGS=",
      "detect_browser()",
      "has_active_display()",
      "start_xvfb_display()",
      "cleanup_xvfb()",
      "run_with_xvfb()",
      "trap cleanup_xvfb EXIT",
    ]) {
      assert.ok(ciSource.includes(token), `в scripts/ci.sh отсутствует обязательный элемент: ${token}`);
    }
  });

  it("scripts/ci.sh --dry-run --scope release выбирает прямой запуск при активном DISPLAY и xvfb-run при отсутствии DISPLAY", () => {
    const tmp = mkdtempSync(join(tmpdir(), "discord-lab-ci-sh-"));
    try {
      const x11Dir = join(tmp, ".X11-unix");
      mkdirSync(x11Dir, { recursive: true });
      writeFileSync(join(x11Dir, "X42"), "");
      const fakeBrowser = makeFakeExecutable(tmp, "chromium");

      // 1. Активный DISPLAY (:42 при наличии X42 в DISCORD_LAB_X11_UNIX_DIR).
      const activeRun = spawnSync(
        "bash",
        [ciScriptPath, "--dry-run", "--scope", "release"],
        {
          cwd: root,
          encoding: "utf8",
          env: {
            ...process.env,
            DISPLAY: ":42",
            DISCORD_LAB_X11_UNIX_DIR: x11Dir,
            CHROME_BIN: fakeBrowser,
          },
        },
      );
      assert.equal(activeRun.status, 0, `stderr=${activeRun.stderr}`);
      assert.match(activeRun.stdout, /ci: browser detected: CHROME_BIN=.*chromium/);
      assert.match(activeRun.stdout, /ci: display: активен DISPLAY=:42/);
      assert.match(activeRun.stdout, /\+ node scripts\/gate\.mjs --scope release/);
      assert.doesNotMatch(activeRun.stdout, /\+ xvfb-run/);

      // 2. Неактивный DISPLAY (:99 без сокета X99 в DISCORD_LAB_X11_UNIX_DIR).
      const headlessRun = spawnSync(
        "bash",
        [ciScriptPath, "--dry-run", "--scope=release"],
        {
          cwd: root,
          encoding: "utf8",
          env: {
            ...process.env,
            DISPLAY: ":99",
            DISCORD_LAB_X11_UNIX_DIR: x11Dir,
            CHROME_BIN: fakeBrowser,
          },
        },
      );
      assert.equal(headlessRun.status, 0, `stderr=${headlessRun.stderr}`);
      assert.match(headlessRun.stdout, /ci: display: активный DISPLAY отсутствует/);
      assert.match(
        headlessRun.stdout,
        /\+ xvfb-run -a -s "-screen 0 1280x720x24 -ac \+extension GLX \+render -noreset" node scripts\/gate\.mjs --scope release/,
      );
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("scripts/ci.sh не подменяет явно заданный невалидный CHROME_BIN другим браузером и отвергает неизвестный --scope", () => {
    const tmp = mkdtempSync(join(tmpdir(), "discord-lab-ci-neg-"));
    try {
      const brokenBin = join(tmp, "missing-chrome-binary");
      const brokenRun = spawnSync(
        "bash",
        [ciScriptPath, "--dry-run", "--scope", "worker"],
        {
          cwd: root,
          encoding: "utf8",
          env: {
            ...process.env,
            DISPLAY: "",
            CHROME_BIN: brokenBin,
          },
        },
      );
      assert.equal(brokenRun.status, 0);
      assert.match(
        brokenRun.stderr,
        /ci: browser: явный CHROME_BIN=.*missing-chrome-binary не существует или не исполняется/,
      );
      assert.match(
        brokenRun.stdout,
        new RegExp(`CHROME_BIN=${brokenBin.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`),
        "явный невалидный CHROME_BIN должен сохраняться для fail-closed проверки в gate.mjs",
      );

      const badScope = spawnSync(
        "bash",
        [ciScriptPath, "--dry-run", "--scope", "invalid-scope"],
        { cwd: root, encoding: "utf8" },
      );
      assert.equal(badScope.status, 1, "неизвестный --scope обязан завершаться с кодом 1");
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("run_with_xvfb и start_xvfb_display/cleanup_xvfb управляют виртуальным Xvfb-сервером и очищают процесс", () => {
    const tmp = mkdtempSync(join(tmpdir(), "discord-lab-ci-xvfb-daemon-"));
    try {
      const binDir = join(tmp, "bin");
      const x11Dir = join(tmp, ".X11-unix");
      const argsLog = join(tmp, "xvfb-args.log");
      mkdirSync(binDir, { recursive: true });
      mkdirSync(x11Dir, { recursive: true });

      // Фейковый демон Xvfb: записывает свои аргументы, создаёт файл сокета X<num> и ждёт сигнала.
      makeFakeExecutable(
        binDir,
        "Xvfb",
        `#!/usr/bin/env bash
echo "$*" > "${argsLog}"
disp="\${1#:}"
touch "${x11Dir}/X\${disp}"
trap 'rm -f "${x11Dir}/X\${disp}"; exit 0' TERM INT
while true; do sleep 0.1; done
`,
      );

      // Извлекаем блок функций из scripts/ci.sh (от XVFB_SERVER_ARGS до run() {).
      const fnStart = ciSource.indexOf("XVFB_SERVER_ARGS=");
      const fnEnd = ciSource.indexOf("\nrun() {");
      assert.ok(fnStart >= 0 && fnEnd > fnStart, "не найден блок функций Xvfb в scripts/ci.sh");
      const functionsBlock = ciSource.slice(fnStart, fnEnd);

      const testScript = join(tmp, "test-xvfb-lifecycle.sh");
      writeFileSync(
        testScript,
        `#!/usr/bin/env bash
set -euo pipefail
ROOT="${root}"
DRY_RUN=0
${functionsBlock}
start_xvfb_display
echo "STARTED_DISPLAY=$DISPLAY"
echo "STARTED_PID=$XVFB_PID"
kill -0 "$XVFB_PID"
cleanup_xvfb
echo "CLEANED_PID=\${XVFB_PID:-none}"
`,
        "utf8",
      );
      chmodSync(testScript, 0o755);

      const res = spawnSync("bash", [testScript], {
        encoding: "utf8",
        env: {
          ...process.env,
          DISPLAY: "",
          DISCORD_LAB_X11_UNIX_DIR: x11Dir,
          PATH: `${binDir}${delimiter}${process.env.PATH ?? ""}`,
        },
      });
      assert.equal(res.status, 0, `stderr=${res.stderr}`);
      assert.match(res.stdout, /STARTED_DISPLAY=:99/);
      assert.match(res.stdout, /STARTED_PID=\d+/);
      assert.match(res.stdout, /CLEANED_PID=none/);

      const loggedArgs = readFileSync(argsLog, "utf8").trim();
      assert.match(loggedArgs, /^:99 -screen 0 1280x720x24 -ac \+extension GLX \+render -noreset$/);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });
});
