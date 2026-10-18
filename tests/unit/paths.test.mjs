// @ts-check
// RED (M1): реализация — src/core/paths.mjs. Контракт: пути проекта и имена скриншотов.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  functionsDir,
  logsDir,
  projectRoot,
  relativeToRoot,
  screenshotName,
  screensDir,
} from "../../src/core/paths.mjs";

const expectedRoot = resolve(fileURLToPath(new URL("../../", import.meta.url)));

describe("пути проекта", () => {
  it("projectRoot указывает на корень проекта", () => {
    assert.equal(projectRoot(), expectedRoot);
  });

  it("functionsDir / logsDir / screensDir лежат внутри корня", () => {
    assert.equal(functionsDir(), join(expectedRoot, "functions"));
    assert.equal(logsDir(), join(expectedRoot, "logs"));
    assert.equal(screensDir(), join(expectedRoot, "evidence", "screens"));
  });
});

describe("screenshotName", () => {
  // Имена проверены через ls evidence/screens | sort; slug паритетен функции slug
  // в scripts/ui-walk-settings.mjs: Unicode-буквы/цифры, один дефис, обрезка до 40.
  it("воспроизводит имена из evidence/screens и подписи из журнала UI", () => {
    /** @type {[string, number, string, string][]} */
    const cases = [
      ["ui-setting", 1, "Учётная запись", "ui-setting-01-Учётная-запись.png"],
      ["ui-setting", 2, "Информация об учётной записи", "ui-setting-02-Информация-об-учётной-записи.png"],
      ["ui-setting", 7, "Уведомления", "ui-setting-07-Уведомления.png"],
      ["ui-setting", 13, "Значок приложения", "ui-setting-13-Значок-приложения.png"],
      ["ui-setting2", 52, "Кто может присоединяться к моим играм", "ui-setting2-52-Кто-может-присоединяться-к-моим-играм.png"],
      ["ui-menu", 23, "Почта → вкладка Утилиты", "ui-menu-23-Почта-вкладка-Утилиты.png"],
      ["ui-menu", 24, "Почта → вкладка Ещё", "ui-menu-24-Почта-вкладка-Ещё.png"],
      ["ui-menu2", 35, "Меню сервера2 → Показать все каналы", "ui-menu2-35-Меню-сервера2-Показать-все-каналы.png"],
    ];
    for (const [prefix, step, label, expected] of cases) {
      assert.equal(screenshotName(prefix, step, label), expected);
    }
  });

  it("нумерует шаг с ведущим нулём и сохраняет кириллицу", () => {
    assert.equal(screenshotName("ui-setting", 7, "Мой аккаунт"), "ui-setting-07-Мой-аккаунт.png");
  });

  it("номер из трёх цифр не обрезается", () => {
    assert.equal(screenshotName("ui-menu", 100, "Профиль"), "ui-menu-100-Профиль.png");
  });

  it("последовательности пунктуации сворачиваются в один дефис", () => {
    assert.equal(screenshotName("ui-setting", 2, "Голос,  видео!"), "ui-setting-02-Голос-видео-.png");
  });

  it("длинные подписи обрезаются до 40 символов slug", () => {
    const label = "a".repeat(50);
    assert.equal(screenshotName("ui-setting2", 3, label), `ui-setting2-03-${"a".repeat(40)}.png`);
  });

  it("пустая подпись → item", () => {
    assert.equal(screenshotName("ui-setting", 3, ""), "ui-setting-03-item.png");
  });
});

describe("relativeToRoot", () => {
  // Пути взяты из реально имеющихся каталогов evidence/screens и functions/static.
  it("возвращает реальные артефактные пути относительно корня", () => {
    assert.equal(relativeToRoot(join(expectedRoot, "evidence", "screens", "ui-menu-01-Меню-сервера-открыто.png")),
      "evidence/screens/ui-menu-01-Меню-сервера-открыто.png");
    assert.equal(relativeToRoot(join(expectedRoot, "functions", "static", "discord_utils.node.nm.txt")),
      "functions/static/discord_utils.node.nm.txt");
  });
  it("путь внутри корня становится относительным", () => {
    const absolute = join(expectedRoot, "evidence", "screens", "ui-setting-01-x.png");
    assert.equal(relativeToRoot(absolute), "evidence/screens/ui-setting-01-x.png");
  });

  it("путь вне корня возвращается как есть", () => {
    assert.equal(relativeToRoot("/tmp/other.png"), "/tmp/other.png");
  });
});
