// @ts-check
// Live smoke запускается явно: GUI требует видимой сессии Wayland/X11.
// GUI_SPIKE_E2E=1 — принудительно; GUI_SPIKE_E2E=0 — отключить; иначе авто по DISPLAY.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { spawn } from "node:child_process";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const binary = process.env.GUI_SPIKE_BIN
  ?? fileURLToPath(new URL("../../rust/gui-spike/target/debug/discord-gui-spike", import.meta.url));
const uiDir = fileURLToPath(new URL("../../rust/gui-spike/ui/", import.meta.url));

/** Новейший mtime в ui/** (рекурсивно) — Tauri встраивает frontendDist при сборке. @returns {number} */
function newestUiMtime() {
  const entries = readdirSync(uiDir, { recursive: true });
  return Math.max(...entries.map((name) => statSync(join(uiDir, String(name))).mtimeMs));
}

describe("Tauri GUI live smoke", () => {
  it("открывает окно и публикует проверяемый DOM-снимок", { timeout: 20_000 }, async (t) => {
    // В гейте на машине с дисплеем smoke обязателен: GBM/panic → failed gate.
    // Без дисплея (CI) — skip с причиной; GUI_SPIKE_E2E=1 форсирует запуск.
    const forced = process.env.GUI_SPIKE_E2E;
    const hasDisplay = Boolean(process.env.DISPLAY || process.env.WAYLAND_DISPLAY);
    if (!hasDisplay && forced !== "1") return t.skip("нет дисплея (DISPLAY/WAYLAND_DISPLAY)");
    // P2-1: на дисплее нет бинарника — это fail, а не тихий skip.
    if (!existsSync(binary)) {
      assert.fail(`нет бинарника ${binary}: на дисплее smoke обязателен (cargo build)`);
    }
    // Иначе smoke проверяет встроенную в старый бинарь копию ui/*, а не сдаваемые файлы.
    assert.ok(
      statSync(binary).mtimeMs >= newestUiMtime(),
      "бинарь старше ui/*: пересоберите `cargo build` перед live smoke",
    );

    // P2-3: приложение обязано дожить до SPIKE_EXIT_AFTER_MS; ранний exit 0 — fail.
    const exitAfterMs = Number(process.env.SPIKE_EXIT_AFTER_MS ?? 10_000);
    const startedAt = Date.now();
    const child = spawn(binary, [], {
      cwd: fileURLToPath(new URL("../../", import.meta.url)),
      env: {
        ...process.env,
        GDK_BACKEND: process.env.GDK_BACKEND ?? "x11",
        WEBKIT_DISABLE_COMPOSITING_MODE: "1",
        WEBKIT_DISABLE_DMABUF_RENDERER: "1",
        LIBGL_ALWAYS_SOFTWARE: "1",
        SPIKE_EXIT_AFTER_MS: String(exitAfterMs),
        SPIKE_AUTOBENCH: "0",
        SPIKE_AUTH_DISABLED: "1",
      },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk) => { stdout += chunk; });
    child.stderr.setEncoding("utf8").on("data", (chunk) => { stderr += chunk; });
    // Не оставляем сиротское окно, если приложение зависло/не закрылось.
    const killer = setTimeout(() => child.kill("SIGKILL"), 18_000);
    try {
      const exitCode = await new Promise((resolve) => child.once("close", resolve));

      assert.equal(exitCode, 0, `Tauri exit=${exitCode}; stderr: ${stderr}`);
      const livedMs = Date.now() - startedAt;
      const minAliveMs = exitAfterMs - 1_000;
      assert.ok(
        livedMs >= minAliveMs,
        `окно прожило ${livedMs} мс < ${minAliveMs} мс (SPIKE_EXIT_AFTER_MS=${exitAfterMs}): `
          + `преждевременный exit ${exitCode}; stderr: ${stderr}`,
      );
      assert.match(stdout, /discord-gui-spike: window-ready/);
      const line = stdout.split("\n").find((item) => item.startsWith("UI_SNAPSHOT "));
      assert.ok(line, `UI_SNAPSHOT отсутствует; stderr: ${stderr}`);
      const snapshot = JSON.parse(line.slice("UI_SNAPSHOT ".length));
      assert.equal(snapshot.screenCount, 4);
      assert.equal(snapshot.chatRows, 100_000);
      assert.equal(snapshot.memberRows, 24_229);
      assert.equal(snapshot.emojiCategories, 10);
      assert.equal(snapshot.settingsToggles, 38);
      assert.ok(snapshot.chatRenderedRows > 0 && snapshot.chatRenderedRows < snapshot.chatRows);
      assert.ok(snapshot.memberRenderedRows > 0 && snapshot.memberRenderedRows < snapshot.memberRows);
      // P1-1 регрессия: при скрытом экране рисуется только overscan (~8). Порог 10 ловит это.
      assert.ok(snapshot.chatRenderedRows >= 10, `chatRenderedRows=${snapshot.chatRenderedRows}`);
      assert.ok(snapshot.memberRenderedRows >= 10, `memberRenderedRows=${snapshot.memberRenderedRows}`);
      // P2 (REV-1): окно данных не должно снова материализовать весь список.
      assert.ok(
        Number.isInteger(snapshot.chatLoadedRows) && snapshot.chatLoadedRows <= 500,
        `chatLoadedRows=${snapshot.chatLoadedRows} > 500`,
      );
      assert.ok(
        Number.isInteger(snapshot.memberLoadedRows) && snapshot.memberLoadedRows <= 500,
        `memberLoadedRows=${snapshot.memberLoadedRows} > 500`,
      );
      assert.ok(snapshot.chatLoadedRows < snapshot.chatRows);
      assert.ok(snapshot.memberLoadedRows < snapshot.memberRows);
      assert.deepEqual(snapshot.actions, {
        screenClicks: 4,
        categoryClicks: true,
        toggleClick: true,
        listScroll: true,
      });
      assert.doesNotMatch(stderr, /Failed to create GBM buffer|Gdk-Message:|panic|segmentation fault/i);
    } finally {
      clearTimeout(killer);
      if (child.exitCode === null) child.kill("SIGKILL");
    }
  });
});
