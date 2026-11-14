// @ts-check
// RW-38: живая AT-SPI/X11-проба окна. Запускает scripts/gui-window-probe.py,
// который проверяет элементы внутри реального окна и снимает PNG.
// В гейте на машине с дисплеем проба обязательна; без дисплея — skip с причиной.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const probe = fileURLToPath(new URL("../../scripts/gui-window-probe.py", import.meta.url));

describe("GUI window probe (AT-SPI + X11)", () => {
  it("находит окно, элементы и снимает PNG", { timeout: 90_000 }, (t) => {
    const forced = process.env.GUI_SPIKE_E2E;
    const hasDisplay = Boolean(process.env.DISPLAY || process.env.WAYLAND_DISPLAY);
    if (!hasDisplay && forced !== "1") return t.skip("нет дисплея (DISPLAY/WAYLAND_DISPLAY)");
    // P2-1: на дисплее проба обязательна — отсутствие скрипта/среды это fail, не skip.
    assert.ok(existsSync(probe), `нет пробы ${probe}: на дисплее live-слой обязателен`);

    const dir = mkdtempSync(join(tmpdir(), "gui-window-probe-"));
    const out = join(dir, "probe.json");
    const runsLog = join(dir, "window-runs.jsonl");
    try {
      const result = spawnSync("python3", [probe, "--json", out, "--runs-log", runsLog, "--timeout", "30"], {
        encoding: "utf8",
        timeout: 80_000,
      });
      assert.notEqual(
        result.status,
        2,
        `проба не нашла среду на дисплее (exit 2) — это fail, не skip: ${result.stderr?.trim()}`,
      );
      assert.equal(result.status, 0, result.stderr || result.stdout);
      const report = JSON.parse(readFileSync(out, "utf8"));
      assert.equal(report.ok, true);
      assert.equal(report.runs_log, runsLog, "проба должна писать в собственный временный журнал");
      const journal = JSON.parse(readFileSync(runsLog, "utf8").trim());
      assert.equal(journal.pid, report.pid, "журнал должен относиться к этому запуску пробы");
      // P1-1: отчёт и снятое окно принадлежат одному процессу пробы.
      assert.ok(Number.isInteger(report.pid) && report.pid > 0, `report.pid=${report.pid}`);
      assert.equal(report.window.pid, report.pid, "окно снято не у процесса пробы");
      assert.equal(report.emoji_categories, 10);
      assert.ok(report.emoji_cells >= 10, `emoji_cells=${report.emoji_cells} < 10`);
      assert.equal(report.settings_toggles, 38);
      assert.ok(report.chat_items >= 10, `chat_items=${report.chat_items}`);
      assert.ok(report.member_items >= 10, `member_items=${report.member_items}`);
      assert.ok(report.window.width >= 900 && report.window.height >= 600);
      assert.ok(report.window.non_uniform, "PNG окна одноцветный");
      assert.equal(report.snapshot.screenCount, 4);
      assert.equal(report.snapshot.chatRows, 100000);
      assert.equal(report.snapshot.memberRows, 24229);
      assert.equal(report.snapshot.settingsToggles, 38);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});
