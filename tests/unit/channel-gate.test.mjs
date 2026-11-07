// @ts-check
import assert from "node:assert/strict";
import test from "node:test";

import {
  evaluateChannelAgeGate,
  evaluateComposerLockState,
} from "../../rust/gui-spike/ui/features/channel-gate.js";

const GATED = {
  gated: true,
  titleRu: "Канал с возрастным ограничением (18+)",
  confirmLabelRu: "Продолжить",
  cancelLabelRu: "Назад",
};

const OPEN = { gated: false, titleRu: "", confirmLabelRu: "", cancelLabelRu: "" };

const LOCKED_THREAD = {
  disabled: true,
  placeholderRu: "Эта ветка заблокирована модератором",
};

const NO_PERMISSION = {
  disabled: true,
  placeholderRu: "У вас нет прав на отправку сообщений в этом канале",
};

test("age gate: NSFW-канал без подтверждения требует 18+", () => {
  assert.deepEqual(evaluateChannelAgeGate({ id: "c1", nsfw: true }), GATED);
});

test("age gate: NSFW-канал с подтверждением открыт", () => {
  assert.deepEqual(
    evaluateChannelAgeGate({ id: "c1", nsfw: true }, ["c1"]),
    OPEN,
  );
});

test("age gate: подтверждение другого канала не помогает", () => {
  assert.deepEqual(
    evaluateChannelAgeGate({ id: "c2", nsfw: true }, ["c1"]),
    GATED,
  );
});

test("age gate: обычный канал и отсутствие канала открыты", () => {
  assert.deepEqual(evaluateChannelAgeGate({ id: "c1", nsfw: false }), OPEN);
  assert.deepEqual(evaluateChannelAgeGate({ id: "c1" }), OPEN);
  assert.deepEqual(evaluateChannelAgeGate(undefined, []), OPEN);
  assert.deepEqual(evaluateChannelAgeGate(null, []), OPEN);
});

test("composer: заблокированная ветка без прав управления", () => {
  assert.deepEqual(
    evaluateComposerLockState({ name: "bugs", threadLocked: true }),
    LOCKED_THREAD,
  );
});

test("composer: модератор может писать в заблокированную ветку", () => {
  assert.deepEqual(
    evaluateComposerLockState({
      name: "bugs",
      threadLocked: true,
      canManageThreads: true,
    }),
    { disabled: false, placeholderRu: "Написать в #bugs" },
  );
});

test("composer: нет права отправки сообщений", () => {
  assert.deepEqual(
    evaluateComposerLockState({ name: "rules", canSendMessages: false }),
    NO_PERMISSION,
  );
});

test("composer: обычный канал подставляет имя", () => {
  assert.deepEqual(evaluateComposerLockState({ name: "general" }), {
    disabled: false,
    placeholderRu: "Написать в #general",
  });
});

test("composer: без имени и без канала — заглушка #канал", () => {
  assert.deepEqual(evaluateComposerLockState({}), {
    disabled: false,
    placeholderRu: "Написать в #канал",
  });
  assert.deepEqual(evaluateComposerLockState(undefined), {
    disabled: false,
    placeholderRu: "Написать в #канал",
  });
});

test("composer: приоритет блокировки ветки над отсутствием прав", () => {
  assert.deepEqual(
    evaluateComposerLockState({ threadLocked: true, canSendMessages: false }),
    LOCKED_THREAD,
  );
});
