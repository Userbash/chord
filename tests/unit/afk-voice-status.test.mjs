// @ts-check
import test from "node:test";
import assert from "node:assert/strict";
import { ShimDocument } from "../support/gui-dom-harness.mjs";
import {
  VOICE_CALL_PHASE_LABELS,
  resolveVoiceCallPhaseView,
  renderVoiceRtcPhaseDom,
} from "../../rust/gui-spike/ui/modules/voice-rtc-panel.js";

test("TASK-826 / TASK-829: resolveVoiceCallPhaseView распознаёт фазу AFK-канала", () => {
  assert.equal(
    VOICE_CALL_PHASE_LABELS.afk,
    "🔕 AFK-канал (микрофон и звук отключены)",
    "Лейбл AFK-канала должен быть определён в VOICE_CALL_PHASE_LABELS"
  );

  const afkViewFromStr = resolveVoiceCallPhaseView("afk");
  assert.equal(afkViewFromStr.phase, "afk");
  assert.equal(afkViewFromStr.label, "🔕 AFK-канал (микрофон и звук отключены)");
  assert.equal(afkViewFromStr.statusColor, "#949ba4");
  assert.equal(afkViewFromStr.isError, false);
  assert.equal(afkViewFromStr.isConnected, true);
  assert.equal(afkViewFromStr.canRetry, false);

  const afkViewFromObj = resolveVoiceCallPhaseView({
    phase: "afk",
    isAfk: true,
  });
  assert.equal(afkViewFromObj.phase, "afk");
  assert.equal(afkViewFromObj.label, "🔕 AFK-канал (микрофон и звук отключены)");
  assert.equal(afkViewFromObj.statusColor, "#949ba4");
});

test("TASK-826: renderVoiceRtcPhaseDom корректно отображает статус AFK без зависания в ожидании", () => {
  const doc = /** @type {any} */ (new ShimDocument());
  const panel = doc.createElement("section");
  panel.id = "voice-panel";

  const statusBtn = doc.createElement("button");
  statusBtn.className = "voice-status-trigger";
  const badge = doc.createElement("span");
  badge.id = "live-voice-status-badge";
  const rttPill = doc.createElement("span");
  rttPill.id = "live-voice-rtt-pill";
  const connStatus = doc.createElement("div");
  connStatus.id = "live-rtc-conn-status";

  statusBtn.append(badge, rttPill, connStatus);
  panel.appendChild(statusBtn);
  doc.body.appendChild(panel);

  renderVoiceRtcPhaseDom(panel, { phase: "afk", isAfk: true }, { doc });

  assert.ok(
    badge.textContent.includes("AFK-канал"),
    `Бейдж должен содержать AFK-канал, получено: ${badge.textContent}`
  );
  assert.equal(badge.style.color, "#949ba4");
  assert.equal(
    connStatus.textContent,
    "Микрофон и звук отключены сервером"
  );
});
