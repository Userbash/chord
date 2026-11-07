// @ts-check
/**
 * T-188 (UI token map): контракт таблицы соответствия локальных CSS-переменных
 * каноническим токенам клиента Discord и аудита таблиц стилей.
 *
 * Проверяет: заморозку карт, точные 7 разрешённых коллизий, полноту
 * CANONICAL_TOKEN_MAP (>= 24), resolveClientToken и auditStylesheetTokens.
 */
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  CANONICAL_TOKEN_MAP,
  RESOLVED_COLLISIONS,
  auditStylesheetTokens,
  resolveClientToken,
} from "../../rust/gui-spike/ui/features/token-map.js";

/** @type {Record<string, string>} */
const EXPECTED_COLLISIONS = {
  "--bg-secondary": "--background-secondary",
  "--bg-tertiary": "--background-tertiary",
  "--text-muted": "--text-muted-discord",
  "--border-subtle": "--background-modifier-accent",
  "--brand-hover": "--brand-560",
  "--danger-fg": "--status-danger",
  "--surface-overlay": "--background-floating",
};

describe("RESOLVED_COLLISIONS", () => {
  it("содержит ровно 7 разрешённых коллизий с точными именами", () => {
    assert.equal(Object.keys(RESOLVED_COLLISIONS).length, 7);
    assert.deepEqual({ ...RESOLVED_COLLISIONS }, EXPECTED_COLLISIONS);
  });

  it("заморожен", () => {
    assert.ok(Object.isFrozen(RESOLVED_COLLISIONS));
  });
});

describe("CANONICAL_TOKEN_MAP", () => {
  it("заморожен и содержит не менее 24 записей", () => {
    assert.ok(Object.isFrozen(CANONICAL_TOKEN_MAP));
    assert.ok(Object.keys(CANONICAL_TOKEN_MAP).length >= 24);
  });

  it("включает все 7 ключей коллизий с теми же значениями", () => {
    for (const [local, discord] of Object.entries(EXPECTED_COLLISIONS)) {
      assert.equal(CANONICAL_TOKEN_MAP[local], discord, local);
    }
  });

  it("покрывает обязательные локальные токены", () => {
    const required = [
      "--bg-0",
      "--bg-1",
      "--bg-2",
      "--bg-3",
      "--bg-4",
      "--surface-1",
      "--surface-2",
      "--surface-3",
      "--surface-4",
      "--text-0",
      "--text-1",
      "--text-2",
      "--text-strong",
      "--accent",
      "--online",
      "--idle",
      "--dnd",
      "--offline",
    ];
    for (const token of required) {
      assert.ok(Object.hasOwn(CANONICAL_TOKEN_MAP, token), token);
    }
  });

  it("все ключи и значения — валидные CSS custom properties", () => {
    for (const [local, discord] of Object.entries(CANONICAL_TOKEN_MAP)) {
      assert.match(local, /^--[a-z0-9-]+$/, local);
      assert.match(discord, /^--[a-z0-9-]+$/, discord);
      assert.notEqual(local, discord, `identity mapping for ${local}`);
    }
  });
});

describe("resolveClientToken", () => {
  it("возвращает канонический токен Discord для известного локального", () => {
    assert.equal(resolveClientToken("--bg-0"), CANONICAL_TOKEN_MAP["--bg-0"]);
    assert.equal(resolveClientToken("--accent"), CANONICAL_TOKEN_MAP["--accent"]);
  });

  it("разрешает ключи коллизий в канонические имена", () => {
    for (const [local, discord] of Object.entries(EXPECTED_COLLISIONS)) {
      assert.equal(resolveClientToken(local), discord, local);
    }
  });

  it("возвращает null для неизвестного токена", () => {
    assert.equal(resolveClientToken("--definitely-not-a-token"), null);
  });

  it("возвращает null для нестроковых и пустых входов", () => {
    for (const bad of ["", null, undefined, 42, {}, []]) {
      // @ts-expect-error — проверяем защиту от неверных типов
      assert.equal(resolveClientToken(bad), null);
    }
  });
});

describe("auditStylesheetTokens", () => {
  it("возвращает mapped/unmapped/collisionsResolved", () => {
    const result = auditStylesheetTokens(":root { --accent: #5865f2; --mystery: red; }");
    assert.deepEqual(Object.keys(result).sort(), ["collisionsResolved", "mapped", "unmapped"].sort());
    assert.deepEqual(result.mapped, ["--accent"]);
    assert.deepEqual(result.unmapped, ["--mystery"]);
    assert.deepEqual(result.collisionsResolved, []);
  });

  it("помечает разрешённые коллизии отдельно", () => {
    const result = auditStylesheetTokens("a { color: var(--bg-secondary); border-color: var(--border-subtle); }");
    assert.deepEqual(result.mapped.slice().sort(), ["--bg-secondary", "--border-subtle"].sort());
    assert.deepEqual(result.collisionsResolved.slice().sort(), ["--bg-secondary", "--border-subtle"].sort());
    assert.deepEqual(result.unmapped, []);
  });

  it("дедуплицирует повторяющиеся токены", () => {
    const result = auditStylesheetTokens(".a{color:var(--accent)}.b{background:var(--accent)}.c{--accent:red}");
    assert.deepEqual(result.mapped, ["--accent"]);
    assert.equal(result.mapped.length, 1);
  });

  it("игнорирует текст без custom properties и нестроковый вход", () => {
    assert.deepEqual(auditStylesheetTokens("color: red; .x { margin: 0 }"), {
      mapped: [],
      unmapped: [],
      collisionsResolved: [],
    });
    // @ts-expect-error — проверяем защиту от неверных типов
    assert.deepEqual(auditStylesheetTokens(null), { mapped: [], unmapped: [], collisionsResolved: [] });
  });

  it("не считает коллизией ключ, который отсутствует в CSS", () => {
    const result = auditStylesheetTokens("--accent: #fff;");
    assert.deepEqual(result.collisionsResolved, []);
  });
});
