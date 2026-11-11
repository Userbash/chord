// @ts-check
/**
 * Контрактный тест TASK-901 (SUB-901-1): изолированный headless-композитор Wayland
 * `scripts/run-headless-compositor.sh` (cage/wlroots, llvmpipe, виртуальный сокет wayland-99).
 *
 * Проверяемые инварианты (9 измерений TEST-POLICY.md):
 * 1. Positive: запуск скрипта создаёт изолированный XDG_RUNTIME_DIR в оперативной памяти (/dev/shm
 *    или TMPDIR), поднимает UNIX-сокет `wayland-99` (или заданный через `--socket`), экспортирует
 *    `LIBGL_ALWAYS_SOFTWARE=1`, `GALLIUM_DRIVER=llvmpipe`, `WLR_BACKENDS=headless`,
 *    `WLR_LIBINPUT_NO_DEVICES=1`, `WAYLAND_DISPLAY=wayland-99` и завершается за < 300 мс.
 * 2. Positive (cage backend): при наличии исполняемого `cage` в PATH или `CAGE_BIN` запускает
 *    `cage -d -s -- ...` с окружением headless wlroots + llvmpipe.
 * 3. Boundary: если сокет `wayland-99` уже занят в `XDG_RUNTIME_DIR`, автоматически выделяет
 *    следующий свободный сокет (`wayland-100`..`wayland-199`) без затрагивания сессии пользователя.
 * 4. Negative & Error: неизвестные флаги и невалидный `--resolution` возвращают код выхода `1`;
 *    несуществующий явный `CAGE_BIN` возвращает код выхода `2` (fail-closed, среда недоступна).
 * 5. Recovery & Resource Hygiene: после завершения дочерней команды (включая ненулевой код выхода)
 *    фоновый процесс композитора корректно останавливается (0 зомби-процессов), а временный сокет
 *    удаляется.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const scriptPath = join(root, "scripts", "run-headless-compositor.sh");

/**
 * Создаёт временный исполняемый скрипт.
 * @param {string} dir
 * @param {string} name
 * @param {string} body
 * @returns {string}
 */
function makeExecutable(dir, name, body) {
  const filePath = join(dir, name);
  writeFileSync(filePath, body, "utf8");
  chmodSync(filePath, 0o755);
  return filePath;
}

describe("TASK-901.1: scripts/run-headless-compositor.sh — изолированный композитор Wayland в ОЗУ", () => {
  it("скрипт существует, исполняем и проходит проверку синтаксиса bash -n", () => {
    assert.equal(existsSync(scriptPath), true, "scripts/run-headless-compositor.sh должен существовать");
    const syntax = spawnSync("bash", ["-n", scriptPath], { encoding: "utf8" });
    assert.equal(syntax.status, 0, `Ошибка синтаксиса bash -n: ${syntax.stderr}`);
  });

  it("разворачивает изолированный сокет wayland-99 и окружение llvmpipe менее чем за 300 мс", () => {
    const startMs = performance.now();
    const res = spawnSync(
      "bash",
      [
        scriptPath,
        "--",
        "bash",
        "-c",
        'echo "WL=$WAYLAND_DISPLAY"; echo "SW=$LIBGL_ALWAYS_SOFTWARE"; echo "DRV=$GALLIUM_DRIVER"; echo "WLR=$WLR_BACKENDS"; test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" && echo "SOCKET_OK=1"',
      ],
      {
        cwd: root,
        encoding: "utf8",
        env: {
          ...process.env,
          WAYLAND_DISPLAY: "wayland-0",
          DISPLAY: ":0",
        },
      },
    );
    const elapsedMs = performance.now() - startMs;

    assert.equal(res.status, 0, `stderr=${res.stderr}`);
    assert.match(res.stdout, /WL=wayland-99/);
    assert.match(res.stdout, /SW=1/);
    assert.match(res.stdout, /DRV=llvmpipe/);
    assert.match(res.stdout, /WLR=headless/);
    assert.match(res.stdout, /SOCKET_OK=1/);
    assert.ok(
      elapsedMs < 300,
      `Запуск и остановка headless-композитора заняли ${elapsedMs.toFixed(1)} мс (бюджет < 300 мс)`,
    );
  });

  it("автоматически выбирает следующий свободный сокет wayland-100, если wayland-99 занят", () => {
    const tmpRuntime = mkdtempSync(join(tmpdir(), "discord-wl-collision-"));
    try {
      // Имитируем занятый сокет wayland-99 и lock-файл в заданном XDG_RUNTIME_DIR
      writeFileSync(join(tmpRuntime, "wayland-99"), "");
      writeFileSync(join(tmpRuntime, "wayland-99.lock"), "99999\n");

      const res = spawnSync(
        "bash",
        [
          scriptPath,
          "--runtime-dir",
          tmpRuntime,
          "--",
          "bash",
          "-c",
          'echo "ALLOCATED=$WAYLAND_DISPLAY"; test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" && echo "SOCK_READY=1"',
        ],
        { cwd: root, encoding: "utf8" },
      );

      assert.equal(res.status, 0, `stderr=${res.stderr}`);
      assert.match(res.stdout, /ALLOCATED=wayland-100/);
      assert.match(res.stdout, /SOCK_READY=1/);
      // После завершения скрипта временный сокет wayland-100 должен быть удалён, а чужой wayland-99 не тронут
      assert.equal(existsSync(join(tmpRuntime, "wayland-100")), false, "wayland-100 должен быть очищен после выхода");
      assert.equal(existsSync(join(tmpRuntime, "wayland-99")), true, "существующий wayland-99 не должен удаляться");
    } finally {
      rmSync(tmpRuntime, { recursive: true, force: true });
    }
  });

  it("использует cage -d при наличии бинарника cage (CAGE_BIN) и передаёт переменные wlroots/llvmpipe", () => {
    const tmp = mkdtempSync(join(tmpdir(), "discord-cage-bin-"));
    try {
      const envLog = join(tmp, "cage-env.log");
      const fakeCage = makeExecutable(
        tmp,
        "cage",
        `#!/usr/bin/env bash
echo "ARGS=$*" > "${envLog}"
echo "WLR_BACKENDS=$WLR_BACKENDS" >> "${envLog}"
echo "LIBGL_ALWAYS_SOFTWARE=$LIBGL_ALWAYS_SOFTWARE" >> "${envLog}"
echo "GALLIUM_DRIVER=$GALLIUM_DRIVER" >> "${envLog}"
echo "WAYLAND_DISPLAY=$WAYLAND_DISPLAY" >> "${envLog}"
while [[ $# -gt 0 ]]; do
  if [[ "$1" == "--" ]]; then
    shift
    exec "$@"
  fi
  shift
done
`,
      );

      const res = spawnSync(
        "bash",
        [scriptPath, "--socket", "wayland-99", "--resolution", "1920x1080", "--", "echo", "INSIDE_CAGE_OK"],
        {
          cwd: root,
          encoding: "utf8",
          env: {
            ...process.env,
            CAGE_BIN: fakeCage,
          },
        },
      );

      assert.equal(res.status, 0, `stderr=${res.stderr}`);
      assert.match(res.stdout, /INSIDE_CAGE_OK/);
      const logged = readFileSync(envLog, "utf8");
      assert.match(logged, /WLR_BACKENDS=headless/);
      assert.match(logged, /LIBGL_ALWAYS_SOFTWARE=1/);
      assert.match(logged, /GALLIUM_DRIVER=llvmpipe/);
      assert.match(logged, /WAYLAND_DISPLAY=wayland-99/);
    } finally {
      rmSync(tmp, { recursive: true, force: true });
    }
  });

  it("обрабатывает ошибки аргументов (код 1) и недоступный CAGE_BIN (код 2 fail-closed)", () => {
    const badArg = spawnSync("bash", [scriptPath, "--unknown-flag"], {
      cwd: root,
      encoding: "utf8",
    });
    assert.equal(badArg.status, 1, "неизвестный аргумент обязан возвращать код 1");

    const badRes = spawnSync("bash", [scriptPath, "--resolution", "invalid_res", "--", "true"], {
      cwd: root,
      encoding: "utf8",
    });
    assert.equal(badRes.status, 1, "некорректный формат разрешения обязан возвращать код 1");

    const missingCage = spawnSync("bash", [scriptPath, "--", "true"], {
      cwd: root,
      encoding: "utf8",
      env: {
        ...process.env,
        CAGE_BIN: "/nonexistent/path/to/cage-binary",
      },
    });
    assert.equal(missingCage.status, 2, "явный несуществующий CAGE_BIN обязан возвращать код 2");
  });
});
