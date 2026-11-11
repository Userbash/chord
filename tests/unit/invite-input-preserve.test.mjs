// @ts-check
import test from "node:test";
import assert from "node:assert/strict";
import { loadApp } from "../support/gui-dom-harness.mjs";
import { createJoinServer } from "../../rust/gui-spike/ui/features/join-server.js";

test("TASK-825.4 / TASK-829.2: input инвайта сохраняется при ошибках resolve и accept", async () => {
  const gui = await loadApp();
  const doc = gui.window.document;

  let invokeCallCount = 0;
  /** @type {any} */
  let lastArgs = null;
  /**
   * @param {string} cmd
   * @param {any} [args]
   */
  const mockInvoke = async (cmd, args) => {
    invokeCallCount += 1;
    lastArgs = args;
    if (cmd === "resolve_invite") {
      throw { kind: "notFound" };
    }
    return {};
  };

  const modal = createJoinServer({
    invoke: mockInvoke,
  });

  modal.open();
  const input = /** @type {any} */ (doc.getElementById("join-invite-input"));
  const submitBtn = /** @type {any} */ (doc.getElementById("join-submit"));
  const errorEl = doc.getElementById("join-error");

  assert.ok(input, "Инпут инвайта должен существовать");
  assert.ok(submitBtn, "Кнопка submit должна существовать");

  // Вводим инвайт
  input.value = "https://discord.gg/test-failed-code";
  if (typeof input.dispatchEvent === "function") {
    input.dispatchEvent("input");
  }

  assert.equal(submitBtn.disabled, false, "Кнопка submit должна активироваться после ввода");

  // Нажимаем submit
  await modal.submit();

  // После ошибки 404/notFound:
  assert.equal(
    input.value,
    "https://discord.gg/test-failed-code",
    "Значение инпута должно сохраниться после ошибки сети/404"
  );
  assert.equal(input.getAttribute("aria-invalid"), "true", "Инпут должен быть помечен как невалидный");
  assert.ok(
    errorEl && !errorEl.hasAttribute("hidden"),
    "Сообщение об ошибке должно отображаться"
  );

  // Пользователь вносит исправление
  input.value = "test-fixed-code";
  if (typeof input.dispatchEvent === "function") {
    input.dispatchEvent("input");
  }
  assert.equal(input.hasAttribute("aria-invalid"), false, "aria-invalid снимается при изменении ввода");
  assert.ok(invokeCallCount >= 1, "mockInvoke был вызван");
  assert.deepEqual(lastArgs, { code: "test-failed-code" });
});
