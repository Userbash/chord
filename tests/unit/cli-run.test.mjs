// @ts-check
// Unit-тесты CLI (src/cli/run.mjs): поведение команд без live Discord/окна/сети.
// CDP эмулируется fake fetch/WebSocket (tests/helpers/fake-cdp.mjs),
// Frida — подменённым адаптером через resolve-хук (tests/helpers/fake-frida.mjs).
import { after, afterEach, beforeEach, describe, it } from "node:test";
import assert from "node:assert/strict";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { captureCli } from "../helpers/cli-capture.mjs";
import { installFakeCdp } from "../helpers/fake-cdp.mjs";
import { installFakeFrida } from "../helpers/fake-frida.mjs";
import { FakeWebSocket } from "../helpers/fake-websocket.mjs";
import { makeTmpDir } from "../helpers/tmp.mjs";

// Хук ставится до импорта run.mjs: динамический import("../adapters/frida.mjs") внутри CLI
// получает фейковый адаптер. Продуктовый код не меняется.
const fridaHook = installFakeFrida();
const { run } = await import("../../src/cli/run.mjs");

after(() => {
  fridaHook.restore();
});

/** @type {readonly Record<string, unknown>[]} */
const CALL_RECORDS = [
  { type: "call", module: "discord_utils.node", name: "Task::Run", address: "0x1" },
  { type: "call", module: "discord_utils.node", name: "Task::Run", address: "0x1" },
  { type: "call", module: "discord_app.node", name: "Init", address: "0x2" },
];

/**
 * Фейковый адаптер Frida: поведение задаётся тестом, живой Frida не вызывается.
 * @param {{
 *   processes?: Array<{ pid: number, name: string }>,
 *   deviceError?: Error,
 *   attach?: (device: any, pid: number) => Promise<any>,
 *   collectInventory?: (script: any, options: any) => Promise<any>,
 *   startHook?: (script: any, options: any) => Promise<any>,
 * }} [options]
 * @returns {any}
 */
function fakeFrida(options = {}) {
  const processes = options.processes ?? [];
  return {
    async loadDevice() {
      if (options.deviceError) throw options.deviceError;
      return { name: "fake-device" };
    },
    async listProcesses() {
      return processes;
    },
    attach: options.attach ?? (async () => {
      throw new Error("attach не настроен в тесте");
    }),
    collectInventory: options.collectInventory ?? (async () => []),
    startHook: options.startHook ?? (async (script, hookOptions) => script.exports.hook(hookOptions)),
    /** @param {any} script */
    async stopHook(script) {
      await script.exports.unhook();
      await script.unload();
    },
  };
}

/**
 * Фейковый агент hook: скрипт с message/exports, как у Frida.
 * @param {{ hookResult?: any, hookError?: Error, emitMessages?: boolean }} [options]
 * @returns {{ script: any, lifecycle: string[] }}
 */
function fakeHookScript(options = {}) {
  /** @type {((message: any) => void) | undefined} */
  let emit;
  /** @type {string[]} */
  const lifecycle = [];
  const script = {
    message: {
      /** @param {(message: any) => void} callback */
      connect(callback) {
        emit = callback;
      },
    },
    exports: {
      /** @param {any} hookOptions */
      async hook(hookOptions) {
        lifecycle.push("hook");
        if (options.hookError) throw options.hookError;
        if (options.emitMessages !== false) {
          emit?.({ type: "send", payload: CALL_RECORDS[0] });
          emit?.({ type: "send", payload: { type: "log", message: "не вызов" } });
          emit?.({ type: "error", description: "сбой агента" });
        }
        return options.hookResult ?? { hooked: 1, targets: 1, kind: "symbols" };
      },
      async unhook() {
        lifecycle.push("unhook");
      },
    },
    async unload() {
      lifecycle.push("unload");
    },
  };
  return { script, lifecycle };
}

describe("CLI: справка, версия, неизвестная команда", () => {
  it("без аргументов печатает справку и выходит с 0", async () => {
    const result = await captureCli(() => run([]));
    assert.equal(result.code, 0);
    assert.match(result.stdout, /Команды:/u);
    assert.match(result.stdout, /cdp targets/u);
  });

  it("-h и --help печатают справку", async () => {
    for (const flag of ["-h", "--help"]) {
      const result = await captureCli(() => run([flag]));
      assert.equal(result.code, 0, flag);
      assert.match(result.stdout, /Использование: discord-lab/u);
    }
  });

  it("--version печатает версию из package.json", async () => {
    const packageJson = JSON.parse(await readFile(new URL("../../package.json", import.meta.url), "utf8"));
    const result = await captureCli(() => run(["--version"]));
    assert.equal(result.code, 0);
    assert.equal(result.stdout.trim(), String(packageJson.version));
  });

  it("флаги без команды (например --verbose) печатают справку", async () => {
    const result = await captureCli(() => run(["--verbose"]));
    assert.equal(result.code, 0);
    assert.match(result.stdout, /Команды:/u);
  });

  it("неизвестная команда → код 1", async () => {
    const result = await captureCli(() => run(["nope"]));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /Неизвестная команда: nope/u);
  });

  it("domains/static — заглушки с кодом 2", async () => {
    /** @type {[string, RegExp][]} */
    const cases = [
      ["domains", /domains появится в M4/u],
      ["static", /static появится в M4/u],
    ];
    for (const [command, pattern] of cases) {
      const result = await captureCli(() => run([command]));
      assert.equal(result.code, 2, command);
      assert.match(result.stderr, pattern);
    }
  });
});

describe("CLI summarize", () => {
  it("суммирует указанный лог и пишет CSV рядом", async (t) => {
    const dir = await makeTmpDir(t);
    const logPath = join(dir, "calls-7.jsonl");
    const lines = CALL_RECORDS.map((record) => JSON.stringify(record));
    lines.push("{битая строка");
    await writeFile(logPath, `${lines.join("\n")}\n`);

    const result = await captureCli(() => run(["summarize", logPath]));
    assert.equal(result.code, 0);
    assert.match(result.stdout, /calls-7\.jsonl: вызовов 3, уникальных функций 2 -> calls-7\.jsonl\.by-function\.csv/u);
    const csv = await readFile(`${logPath}.by-function.csv`, "utf8");
    assert.equal(csv, [
      "calls,module,name,address",
      "1,discord_app.node,Init,0x2",
      "2,discord_utils.node,Task::Run,0x1",
      "",
    ].join("\n"));
  });

  it("без аргумента берёт первый calls-*.jsonl из cwd", async (t) => {
    const dir = await makeTmpDir(t);
    await writeFile(join(dir, "calls-b.jsonl"), `${JSON.stringify(CALL_RECORDS[2])}\n`);
    await writeFile(join(dir, "calls-a.jsonl"), `${JSON.stringify(CALL_RECORDS[0])}\n`);
    const previous = process.cwd();
    try {
      process.chdir(dir);
      const result = await captureCli(() => run(["summarize"]));
      assert.equal(result.code, 0);
      assert.match(result.stdout, /calls-a\.jsonl: вызовов 1, уникальных функций 1/u);
      assert.equal(await readFile(join(dir, "calls-a.jsonl.by-function.csv"), "utf8"), [
        "calls,module,name,address",
        "1,discord_utils.node,Task::Run,0x1",
        "",
      ].join("\n"));
    } finally {
      process.chdir(previous);
    }
  });

  it("нет логов в cwd → код 1", async (t) => {
    const dir = await makeTmpDir(t);
    const previous = process.cwd();
    try {
      process.chdir(dir);
      const result = await captureCli(() => run(["summarize"]));
      assert.equal(result.code, 1);
      assert.match(result.stderr, /Логов не найдено/u);
    } finally {
      process.chdir(previous);
    }
  });

  it("cwd недоступен для чтения → код 2", async () => {
    const directory = /** @type {any} */ (process);
    const originalCwd = directory.cwd;
    directory.cwd = () => "/nonexistent-qa4-03-cwd";
    try {
      const result = await captureCli(() => run(["summarize"]));
      assert.equal(result.code, 2);
      assert.match(result.stderr, /Не удалось найти логи в \/nonexistent-qa4-03-cwd/u);
    } finally {
      directory.cwd = originalCwd;
    }
  });

  it("нечитаемый лог → код 2", async (t) => {
    const dir = await makeTmpDir(t);
    const result = await captureCli(() => run(["summarize", join(dir, "missing.jsonl")]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Не удалось обработать лог/u);
  });
});

describe("CLI cdp: fake fetch/WebSocket", () => {
  beforeEach(() => {
    FakeWebSocket.instances.length = 0;
    FakeWebSocket.autoOpen = true;
  });

  afterEach(() => {
    FakeWebSocket.instances.length = 0;
    FakeWebSocket.autoOpen = true;
  });

  it("cdp targets печатает список целей из /json/list", async (t) => {
    const targets = [
      { type: "page", url: "file:///app/index.html", webSocketDebuggerUrl: "ws://fixture" },
      { type: "service_worker", url: "file:///app/sw.js" },
    ];
    const cdp = installFakeCdp({ targets });
    t.after(() => cdp.restore());

    const result = await captureCli(() => run(["cdp", "targets"]));
    assert.equal(result.code, 0);
    assert.deepEqual(JSON.parse(result.stdout), targets);
    assert.deepEqual(cdp.urls, ["http://127.0.0.1:9222/json/list"]);
  });

  it("cdp targets: ошибка HTTP → код 2", async (t) => {
    const cdp = installFakeCdp({ listStatus: 500 });
    t.after(() => cdp.restore());

    const result = await captureCli(() => run(["cdp", "targets"]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Ошибка CDP endpoint http:\/\/127\.0\.0\.1:9222: CDP list failed: 500/u);
  });

  it("cdp без подкоманды → код 1", async () => {
    const result = await captureCli(() => run(["cdp"]));
    assert.equal(result.code, 1);
    assert.match(result.stderr, /Неизвестная команда cdp: \(не указана\)/u);
  });

  it("cdp eval печатает JSON-значение и закрывает сокет", async (t) => {
    const cdp = installFakeCdp({ respond: () => ({ result: { value: { answer: 42 } } }) });
    t.after(() => cdp.restore());

    const result = await captureCli(() => run(["cdp", "eval", "6", "*", "7"]));
    assert.equal(result.code, 0);
    assert.deepEqual(JSON.parse(result.stdout), { answer: 42 });
    assert.equal(cdp.calls[0]?.method, "Runtime.evaluate");
    assert.equal(cdp.calls[0]?.params.expression, "6 * 7");
    assert.equal(cdp.calls[0]?.params.returnByValue, true);
    assert.equal(cdp.calls[0]?.params.awaitPromise, true);
    assert.equal(cdp.sockets().at(-1)?.readyState, 3);
  });

  it("cdp eval: исключение в странице → код 2", async (t) => {
    const cdp = installFakeCdp({
      respond: () => ({ exceptionDetails: { exception: { description: "ReferenceError: x is not defined" } } }),
    });
    t.after(() => cdp.restore());

    const result = await captureCli(() => run(["cdp", "eval", "x"]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /ReferenceError: x is not defined/u);
  });

  it("cdp list печатает элементы", async (t) => {
    const items = [{ tag: "BUTTON", role: null, aria: "Мой аккаунт", text: "Мой аккаунт", visible: true, rect: [0, 0, 10, 10] }];
    const cdp = installFakeCdp({ respond: () => ({ result: { value: items } }) });
    t.after(() => cdp.restore());

    const result = await captureCli(() => run(["cdp", "list", 'button[data-nav="1"]']));
    assert.equal(result.code, 0);
    assert.deepEqual(JSON.parse(result.stdout), items);
    assert.ok(String(cdp.calls[0]?.params.expression).includes(JSON.stringify('button[data-nav="1"]')));
  });

  it("cdp clicktext печатает результат клика", async (t) => {
    const outcome = { clicked: true, tag: "BUTTON", role: null, aria: null, text: "Мой аккаунт" };
    const cdp = installFakeCdp({ respond: () => ({ result: { value: outcome } }) });
    t.after(() => cdp.restore());

    const result = await captureCli(() => run(["cdp", "clicktext", "Мой аккаунт"]));
    assert.equal(result.code, 0);
    assert.deepEqual(JSON.parse(result.stdout), outcome);
  });

  it("cdp shot сохраняет PNG в новый каталог", async (t) => {
    const png = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a]);
    const cdp = installFakeCdp({ respond: () => ({ data: png.toString("base64") }) });
    t.after(() => cdp.restore());
    const dir = await makeTmpDir(t);
    const path = join(dir, "nested", "shot.png");

    const result = await captureCli(() => run(["cdp", "shot", path]));
    assert.equal(result.code, 0);
    assert.deepEqual(JSON.parse(result.stdout), { saved: path, bytes: png.length });
    assert.deepEqual([...(await readFile(path))], [...png]);
  });

  it("cdp shot: каталог не создаётся → код 2", async (t) => {
    const cdp = installFakeCdp({ respond: () => ({ data: Buffer.from([1, 2]).toString("base64") }) });
    t.after(() => cdp.restore());
    const dir = await makeTmpDir(t);
    const blocker = join(dir, "blocker");
    await writeFile(blocker, "not a directory");

    const result = await captureCli(() => run(["cdp", "shot", join(blocker, "shot.png")]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Не удалось записать скриншот/u);
  });

  it("cdp eval без подходящей страницы → код 2", async (t) => {
    const cdp = installFakeCdp({ targets: [] });
    t.after(() => cdp.restore());

    const result = await captureCli(() => run(["cdp", "eval", "1"]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /нет подходящей страницы/u);
  });
});

describe("CLI pids: подменённый адаптер Frida", () => {
  it("без --all печатает только процессы Discord", async () => {
    fridaHook.configure(fakeFrida({
      processes: [
        { pid: 42, name: "Discord" },
        { pid: 43, name: "bash" },
        { pid: 44, name: "Vesktop" },
      ],
    }));

    const result = await captureCli(() => run(["pids"]));
    assert.equal(result.code, 0);
    assert.deepEqual(result.stdout.trim().split("\n"), ["42\tDiscord"]);
  });

  it("--all печатает все процессы", async () => {
    fridaHook.configure(fakeFrida({
      processes: [
        { pid: 42, name: "Discord" },
        { pid: 43, name: "bash" },
      ],
    }));

    const result = await captureCli(() => run(["pids", "--all"]));
    assert.equal(result.code, 0);
    assert.deepEqual(result.stdout.trim().split("\n"), ["42\tDiscord", "43\tbash"]);
  });

  it("пустой список → код 0 и подсказка", async () => {
    fridaHook.configure(fakeFrida({ processes: [] }));

    const result = await captureCli(() => run(["pids"]));
    assert.equal(result.code, 0);
    assert.match(result.stderr, /Процессы Discord не найдены/u);
  });

  it("Frida недоступна → код 2", async () => {
    fridaHook.configure(fakeFrida({ deviceError: new Error("local device unavailable") }));

    const result = await captureCli(() => run(["pids"]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Frida недоступна: local device unavailable/u);
  });
});

describe("CLI inventory: подменённый адаптер Frida", () => {
  it("неверные числовые аргументы → код 1", async () => {
    /** @type {[string[], RegExp][]} */
    const cases = [
      [["inventory", "--pid", "abc"], /Неверное значение --pid/u],
      [["inventory", "--max-symbols", "0"], /Неверное значение --max-symbols/u],
    ];
    for (const [args, pattern] of cases) {
      const result = await captureCli(() => run(args));
      assert.equal(result.code, 1, args.join(" "));
      assert.match(result.stderr, pattern);
    }
  });

  it("Frida недоступна → код 2", async () => {
    fridaHook.configure(fakeFrida({ deviceError: new Error("local device unavailable") }));

    const result = await captureCli(() => run(["inventory"]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Frida недоступна: local device unavailable/u);
  });

  it("процессов Discord нет → код 0 и подсказка", async () => {
    fridaHook.configure(fakeFrida({ processes: [] }));

    const result = await captureCli(() => run(["inventory"]));
    assert.equal(result.code, 0);
    assert.match(result.stderr, /Процессы Discord не найдены/u);
  });

  it("ошибка attach по --pid → код 2 и счётчик пропусков", async () => {
    fridaHook.configure(fakeFrida({
      attach: async () => {
        throw new Error("нет доступа к процессу");
      },
    }));

    const result = await captureCli(() => run(["inventory", "--pid", "4242"]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /\[inventory\] pid=4242 pid-4242/u);
    assert.match(result.stderr, /\[inventory\] пропущено \(нет доступа к процессу\)/u);
    assert.match(result.stderr, /\[inventory\] процессов пропущено: 1/u);
  });
});

describe("CLI hook: подменённый адаптер Frida", () => {
  it("неверные аргументы → код 1", async () => {
    /** @type {[string[], RegExp][]} */
    const cases = [
      [["hook", "--pid", "abc"], /Неверное значение --pid/u],
      [["hook", "--limit", "0"], /Неверное значение --limit/u],
      [["hook", "--arg-count", "7"], /Неверное значение --arg-count/u],
      [["hook", "--duration-ms", "-1"], /Неверное значение --duration-ms/u],
      [["hook", "--pattern", "["], /Неверный regex в --pattern/u],
    ];
    for (const [args, pattern] of cases) {
      const result = await captureCli(() => run(args));
      assert.equal(result.code, 1, args.join(" "));
      assert.match(result.stderr, pattern);
    }
  });

  it("пишет записи вызовов в --out и завершает подсчёт", async (t) => {
    const dir = await makeTmpDir(t);
    const out = join(dir, "logs", "calls-7.jsonl");
    const target = fakeHookScript();
    fridaHook.configure(fakeFrida({
      processes: [{ pid: 7, name: "Discord" }],
      attach: async () => ({ session: { detach: async () => {} }, script: target.script }),
    }));

    const result = await captureCli(() => run(["hook", "--pid", "7", "--out", out, "--duration-ms", "1"]));
    assert.equal(result.code, 0);
    assert.match(result.stderr, /\[hook\] pid=7 источник=symbols целей=1 установлено=1/u);
    assert.match(result.stderr, /\[hook\] завершено: записано вызовов: 1, ошибок записи: 0/u);
    const lines = (await readFile(out, "utf8")).trim().split("\n");
    assert.equal(lines.length, 1);
    assert.deepEqual(JSON.parse(lines[0] ?? ""), CALL_RECORDS[0]);
    assert.deepEqual(target.lifecycle, ["hook", "unhook", "unload"]);
  });

  it("hooked=0 → подсказка про фильтр", async (t) => {
    const dir = await makeTmpDir(t);
    const out = join(dir, "calls.jsonl");
    const target = fakeHookScript({ hookResult: { hooked: 0, targets: 0, kind: "exports" }, emitMessages: false });
    fridaHook.configure(fakeFrida({
      attach: async () => ({ session: { detach: async () => {} }, script: target.script }),
    }));

    const result = await captureCli(() => run(["hook", "--pid", "7", "--out", out, "--duration-ms", "1", "--kind", "exports"]));
    assert.equal(result.code, 0);
    assert.match(result.stderr, /целей=0 установлено=0/u);
    assert.match(result.stderr, /Под фильтр не попали функции/u);
    assert.match(result.stderr, /записано вызовов: 0/u);
  });

  it("процессов Discord нет и --pid не задан → код 0", async (t) => {
    const dir = await makeTmpDir(t);
    fridaHook.configure(fakeFrida({ processes: [] }));

    const result = await captureCli(() => run(["hook", "--out", join(dir, "calls.jsonl")]));
    assert.equal(result.code, 0);
    assert.match(result.stderr, /Процессы Discord не найдены\. Запустите Discord или используйте --spawn/u);
  });

  it("Frida недоступна → код 2", async (t) => {
    const dir = await makeTmpDir(t);
    fridaHook.configure(fakeFrida({ deviceError: new Error("local device unavailable") }));

    const result = await captureCli(() => run(["hook", "--pid", "7", "--out", join(dir, "calls.jsonl")]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Frida недоступна: local device unavailable/u);
  });

  it("attach не удался → код 2", async (t) => {
    const dir = await makeTmpDir(t);
    fridaHook.configure(fakeFrida({
      attach: async () => {
        throw new Error("нет доступа");
      },
    }));

    const result = await captureCli(() => run(["hook", "--pid", "7", "--out", join(dir, "calls.jsonl")]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Не удалось подключиться \(pid 7\): нет доступа/u);
  });

  it("каталог лога не создаётся → код 2", async (t) => {
    const dir = await makeTmpDir(t);
    const blocker = join(dir, "blocker");
    await writeFile(blocker, "not a directory");
    fridaHook.configure(fakeFrida({}));

    const result = await captureCli(() => run(["hook", "--pid", "7", "--out", join(blocker, "calls.jsonl")]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Не удалось подготовить каталог лога/u);
  });

  it("лог не открывается (путь — каталог) → код 2 и остановка агента", async (t) => {
    const dir = await makeTmpDir(t);
    const out = join(dir, "log-as-dir");
    await mkdir(out, { recursive: true });
    const target = fakeHookScript();
    fridaHook.configure(fakeFrida({
      attach: async () => ({ session: { detach: async () => {} }, script: target.script }),
    }));

    const result = await captureCli(() => run(["hook", "--pid", "7", "--out", out]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Не удалось открыть лог/u);
    assert.deepEqual(target.lifecycle, ["unhook", "unload"]);
  });

  it("ошибка startHook → код 2, но агент и сессия освобождаются", async (t) => {
    const dir = await makeTmpDir(t);
    const out = join(dir, "calls.jsonl");
    const target = fakeHookScript({ hookError: new Error("агент упал") });
    fridaHook.configure(fakeFrida({
      attach: async () => ({ session: { detach: async () => {} }, script: target.script }),
    }));

    const result = await captureCli(() => run(["hook", "--pid", "7", "--out", out, "--duration-ms", "1"]));
    assert.equal(result.code, 2);
    assert.match(result.stderr, /Ошибка hook \(pid 7\): агент упал/u);
    assert.match(result.stderr, /\[hook\] завершено: записано вызовов: 0/u);
    assert.deepEqual(target.lifecycle, ["hook", "unhook", "unload"]);
  });
});
