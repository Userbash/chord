// @ts-check
import test from "node:test";
import assert from "node:assert/strict";

/**
 * Тесты цветовой коррекции захвата KWin (RGBA ↔ BGRA) и маршрутизации целей захвата.
 */

test("color conversion: swaps red and blue channels while preserving green and alpha", () => {
  // Симуляция алгоритма в C:
  // drow[col] = ((px & 0x000000FFU) << 16) | ((px & 0x00FF0000U) >> 16) | (px & 0xFF00FF00U);
  function swapRgbaToBgra(px) {
    const r = px & 0x000000ff;
    const b = (px & 0x00ff0000) >>> 16;
    const ga = px & 0xff00ff00;
    return ((r << 16) | b | ga) >>> 0;
  }

  // RGBA byte order on little-endian:
  // Byte 0: R, Byte 1: G, Byte 2: B, Byte 3: A
  // Чистый красный (R=255, G=0, B=0, A=255): px = 0xFF0000FF
  const redPx = 0xff0000ff;
  const redSwapped = swapRgbaToBgra(redPx);
  // В BGRA должно быть Byte 0: B=0, Byte 1: G=0, Byte 2: R=255, Byte 3: A=255 -> 0xFFFF0000:
  assert.equal(redSwapped, 0xffff0000 >>> 0); // Byte 2 is Red!

  // Чистый синий (R=0, G=0, B=255, A=255): px = 0xFFFF0000
  const bluePx = 0xffff0000;
  const blueSwapped = swapRgbaToBgra(bluePx);
  // В BGRA должно быть Byte 0: B=255, Byte 1: G=0, Byte 2: R=0, Byte 3: A=255 -> 0xFF0000FF
  assert.equal(blueSwapped, 0xff0000ff >>> 0); // Byte 0 is Blue!

  // Произвольный цвет (R=0x12, G=0x34, B=0x56, A=0x78): px = 0x78563412
  const arbPx = 0x78563412;
  const arbSwapped = swapRgbaToBgra(arbPx);
  // Ожидаем: R=0x12 на месте B (байт 2), B=0x56 на месте R (байт 0) -> 0x78123456
  assert.equal(arbSwapped, 0x78123456 >>> 0);

  // Двойной своп должен возвращать исходный пиксель
  assert.equal(swapRgbaToBgra(arbSwapped), arbPx);
});

test("target routing: detects window UUID vs screen and generates correct IPC commands", () => {
  function parseCaptureTarget(raw) {
    let targetKind = "screen";
    let targetHandle = "";
    let targetLabel = "";
    if (raw && typeof raw === "string") {
      const parts = raw.split(":");
      if (parts[0] === "windows" || parts[0] === "window") {
        targetKind = "window";
      }
      targetHandle = parts[1] || "";
      targetLabel = parts.slice(2).join(":") || "";
    }
    return {
      targetKind,
      targetHandle,
      targetLabel,
      isWindowUuid: targetHandle.startsWith("{") && targetHandle.endsWith("}"),
    };
  }

  // 1. Полный рабочий стол
  const screenTarget = parseCaptureTarget("screens:screen:0:Экран 1");
  assert.equal(screenTarget.targetKind, "screen");
  assert.equal(screenTarget.targetHandle, "screen");
  assert.equal(screenTarget.isWindowUuid, false);

  // 2. Окно браузера с KWin internalId
  const winTarget = parseCaptureTarget("windows:{6ad0aefd-103b-4fa1-b650-3e016c818a00}:YouTube - Brave");
  assert.equal(winTarget.targetKind, "window");
  assert.equal(winTarget.targetHandle, "{6ad0aefd-103b-4fa1-b650-3e016c818a00}");
  assert.equal(winTarget.targetLabel, "YouTube - Brave");
  assert.equal(winTarget.isWindowUuid, true);
});

test("state persistence: preserves relationships and private channels when REST sync returns empty", () => {
  function mergeRestReady(existingReady, fetchedReady) {
    const merged = { ...fetchedReady };
    const prevGuilds = existingReady?.guilds || [];
    const prevRels = existingReady?.relationships || [];
    const prevPrivs = existingReady?.private_channels || [];

    // Сохранение гильдий
    const curGuilds = [...(merged.guilds || [])];
    for (const pg of prevGuilds) {
      if (!curGuilds.some((cg) => cg.id === pg.id)) {
        curGuilds.push(pg);
      }
    }
    merged.guilds = curGuilds;

    // Сохранение друзей (relationships)
    const curRels = [...(merged.relationships || [])];
    if (curRels.length === 0 && prevRels.length > 0) {
      merged.relationships = prevRels;
    } else {
      for (const pr of prevRels) {
        if (!curRels.some((cr) => cr.id === pr.id)) {
          curRels.push(pr);
        }
      }
      merged.relationships = curRels;
    }

    // Сохранение личных каналов
    const curPrivs = [...(merged.private_channels || [])];
    if (curPrivs.length === 0 && prevPrivs.length > 0) {
      merged.private_channels = prevPrivs;
    } else {
      for (const pp of prevPrivs) {
        if (!curPrivs.some((cp) => cp.id === pp.id)) {
          curPrivs.push(pp);
        }
      }
      merged.private_channels = curPrivs;
    }
    return merged;
  }

  const existing = {
    guilds: [{ id: "100", name: "Сервер 1" }, { id: "200", name: "Сервер 2" }],
    relationships: [{ id: "555", type: 1, user: { id: "555", username: "Friend1" } }],
    private_channels: [{ id: "999", type: 1, recipient_ids: ["555"] }],
  };

  // REST вернул частичные данные (нет друзей, пустой массив гильдий)
  const incoming = {
    guilds: [{ id: "100", name: "Сервер 1" }],
    relationships: [],
    private_channels: [],
  };

  const result = mergeRestReady(existing, incoming);
  assert.equal(result.guilds.length, 2, "Сервер 2 должен быть сохранен из previous ready");
  assert.equal(result.relationships.length, 1, "Друзья должны быть сохранены при пустом ответе REST");
  assert.equal(result.relationships[0].id, "555");
  assert.equal(result.private_channels.length, 1, "Личные каналы должны быть сохранены");
});
