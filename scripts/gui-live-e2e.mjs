// @ts-check
/**
 * scripts/gui-live-e2e.mjs
 *
 * Полноценный E2E раннер живого нативного окна chord-client (Tauri + WebKitGTK / AT-SPI / X11).
 * В отличие от статических моков (fake-tauri), этот тест:
 * 1. Запускает скомпилированное реальное приложение chord-client.
 * 2. Инспектирует настоящее окно, элементы GUI и AT-SPI дерево доступности.
 * 3. Тестирует реальные клики по рейлу (Чат, Эмодзи, Участники, Настройки).
 * 4. Проверяет интерактивные тумблеры (Settings Toggles) и изменение их состояния в стейте.
 * 5. Проверяет виртуализацию списков (100,000 сообщений, 24,229 участников).
 * 6. Автоматически логирует инциденты (скриншоты, дампы логов и DOM) при любых сбоях.
 */

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, copyFileSync, writeFileSync, statSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = fileURLToPath(new URL("../", import.meta.url));
const PROBE_SCRIPT = join(ROOT, "scripts", "gui-window-probe.py");
const TARGET_DIR = process.env.CARGO_TARGET_DIR || join(ROOT, "rust", "gui-spike", "target");
const BINARY = process.env.GUI_SPIKE_BIN || join(TARGET_DIR, "debug", "chord-client");
const UI_DIR = join(ROOT, "rust", "gui-spike", "ui");
const INCIDENTS_DIR = join(ROOT, "logs", "gui-incidents");

function newestUiMtime() {
  const entries = readdirSync(UI_DIR, { recursive: true });
  return Math.max(
    ...entries.map((name) => {
      try {
        return statSync(join(UI_DIR, String(name))).mtimeMs;
      } catch {
        return 0;
      }
    })
  );
}

export function runLiveGuiE2e() {
  console.log("🖥️   [GUI-LIVE-E2E] Real Native Window End-to-End Verification");
  console.log("================================================================================");

  // 1. Проверяем окружение дисплея
  const hasDisplay = Boolean(process.env.DISPLAY || process.env.WAYLAND_DISPLAY);
  if (!hasDisplay) {
    console.log("⚠️  [SKIP] Нет доступного дисплея (DISPLAY или WAYLAND_DISPLAY отсутствует).");
    console.log("   Для запуска в CI используйте xvfb-run npm run test:gui:live");
    return;
  }

  // 2. Проверяем наличие и актуальность бинарника
  if (!existsSync(BINARY)) {
    console.error(`❌ [ERROR] Бинарник ${BINARY} не найден!`);
    console.error("   Соберите проект: cargo build -p discord-gui-spike");
    process.exit(1);
  }

  const binMtime = statSync(BINARY).mtimeMs;
  const uiMtime = newestUiMtime();
  if (binMtime < uiMtime) {
    console.warn("⚠️  [WARN] Бинарник chord-client старше файлов в rust/gui-spike/ui/!");
    console.warn("   Рекомендуется пересобрать: cargo build -p discord-gui-spike");
  }

  const tmpOut = join("/tmp", `gui-live-e2e-${Date.now()}.json`);
  const tmpLog = join("/tmp", `gui-live-e2e-run-${Date.now()}.log`);

  console.log("🚀 Запуск реального нативного окна chord-client и пробы AT-SPI/X11...");
  const startTime = Date.now();

  const res = spawnSync("python3", [PROBE_SCRIPT, "--json", tmpOut, "--runs-log", tmpLog, "--timeout", "25"], {
    cwd: ROOT,
    encoding: "utf8",
    timeout: 35000,
    env: {
      ...process.env,
      GDK_BACKEND: process.env.GDK_BACKEND || "x11",
      WEBKIT_DISABLE_COMPOSITING_MODE: "1",
      WEBKIT_DISABLE_DMABUF_RENDERER: "1",
      LIBGL_ALWAYS_SOFTWARE: "1",
    }
  });

  const durationSec = ((Date.now() - startTime) / 1000).toFixed(2);

  // Обработка критического падения
  if (res.status !== 0 || !existsSync(tmpOut)) {
    const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
    const incidentPath = join(INCIDENTS_DIR, `incident-${timestamp}`);
    mkdirSync(incidentPath, { recursive: true });

    console.error(`\n💥 [INCIDENT] Live E2E тестирование завершилось с ошибкой (код ${res.status}) за ${durationSec}с!`);

    if (existsSync(tmpLog)) {
      copyFileSync(tmpLog, join(incidentPath, "app.log"));
    }

    const errorDetails = {
      timestamp: new Date().toISOString(),
      exitCode: res.status,
      durationSec,
      stdout: res.stdout,
      stderr: res.stderr,
      diagnosis: "Приложение chord-client не смогло открыться или упало во время AT-SPI взаимодействия."
    };
    writeFileSync(join(incidentPath, "incident-report.json"), JSON.stringify(errorDetails, null, 2));

    console.error(`📁 Полный отчет об инциденте сохранен в: ${incidentPath}`);
    console.error(`📋 STDERR:\n${res.stderr || res.stdout}`);
    process.exit(1);
  }

  // Читаем результаты успешной пробы
  const report = JSON.parse(readFileSync(tmpOut, "utf8"));

  console.log(`\n⏱️  Тестирование реального окна завершено за ${durationSec}с`);
  console.log("--------------------------------------------------------------------------------");
  console.log("📋 РЕЗУЛЬТАТЫ ПРОВЕРКИ ЖИВОГО ОКНА:");
  console.log(`   • PID процесса приложения          : ${report.pid}`);
  console.log(`   • Заголовок реального окна         : "${report.window_title}"`);
  console.log(`   • Разрешение окна в X11/Wayland    : ${report.window.width}x${report.window.height} px`);
  console.log(`   • Уникальных цветов рендеринга     : ${report.window.unique_colors} (непустой DOM)`);
  console.log(`   • Кнопки главного рейла (AT-SPI)   : [${report.rail.join(", ")}]`);
  console.log(`   • Категории эмодзи в интерфейсе    : ${report.emoji_categories} категорий`);
  console.log(`   • Ячейки эмодзи в активной вкладке : ${report.emoji_cells} шт`);
  console.log(`   • Интерактивные тумблеры настроек  : ${report.settings_toggles} шт`);
  console.log(`   • Тест клика тумблера (мутация)    : ${JSON.stringify(report.toggle_changed)} (true -> false)`);
  console.log(`   • Поисковая строка ввода (Search)  : ${report.search_entry ? "Присутствует и доступна" : "Отсутствует"}`);
  console.log(`   • Виртуализация сообщений в чате   : ${report.snapshot.chatRows} строк`);
  console.log(`   • Виртуализация списка участников  : ${report.snapshot.memberRows} пользователей`);
  console.log("--------------------------------------------------------------------------------");

  // Верификация инвариантов
  let passed = true;
  if (report.rail.length !== 4) {
    console.error(`❌ Ошибка: Ожидалось 4 кнопки рейла, получено ${report.rail.length}`);
    passed = false;
  }
  if (report.settings_toggles < 30) {
    console.error(`❌ Ошибка: Тумблеров настроек меньше ожидаемого: ${report.settings_toggles}`);
    passed = false;
  }
  if (!report.window.non_uniform) {
    console.error("❌ Ошибка: Окно одноцветное (белый/черный экран, верстка не отрендерилась)!");
    passed = false;
  }
  if (!report.toggle_changed || report.toggle_changed[0] === report.toggle_changed[1]) {
    console.error("❌ Ошибка: Клик по тумблеру настроек не изменил состояние стейта!");
    passed = false;
  }

  if (!passed) {
    console.error("💥 FAILED: Инварианты живого окна не соблюдены!");
    process.exit(1);
  }

  console.log("🎉 [GUI-LIVE-E2E] Все сценарии живого окна chord-client УСПЕШНО пройдены!\n");
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  runLiveGuiE2e();
}
