// @ts-check
/**
 * tests/unit/modules-voice-rtc-panel-contract.test.mjs
 *
 * Automated QA Contract Test for `voice-rtc-panel.js` (Modules & Components).
 * Validates:
 * 1. Phase Mapping: signaling, negotiating, connected, afk, error, timeout, disconnected.
 * 2. Status Colors: connected (#23a55a), error (#f23f43), negotiating (#f0b232), idle.
 * 3. Human-readable Russian labels and error reasons.
 * 4. Retry capability on error/timeout.
 * 5. Resiliency against null/undefined inputs.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  VOICE_CALL_PHASE_LABELS,
  resolveVoiceCallPhaseView,
} from '../../rust/gui-spike/ui/modules/voice-rtc-panel.js';

describe('QA Contract — Voice RTC Panel Module', () => {
  it('1. Constants: Phase labels are frozen and complete', () => {
    assert.ok(Object.isFrozen(VOICE_CALL_PHASE_LABELS));
    assert.equal(VOICE_CALL_PHASE_LABELS.signaling, 'Ожидание ответа сервера Discord...');
    assert.equal(VOICE_CALL_PHASE_LABELS.negotiating, 'Проверка UDP IP Discovery и ключей DAVE...');
    assert.equal(VOICE_CALL_PHASE_LABELS.connected, 'Голосовая связь установлена • DAVE E2EE');
    assert.equal(VOICE_CALL_PHASE_LABELS.afk, '🔕 AFK-канал (микрофон и звук отключены)');
  });

  it('2. Disconnected Phase: Default when null/empty input', () => {
    const viewNull = resolveVoiceCallPhaseView(null);
    assert.equal(viewNull.phase, 'disconnected');
    assert.equal(viewNull.isConnected, false);
    assert.equal(viewNull.isError, false);
    assert.equal(viewNull.canRetry, false);

    const viewUndef = resolveVoiceCallPhaseView(undefined);
    assert.equal(viewUndef.phase, 'disconnected');
  });

  it('3. Connected Phase: Reports green status and E2EE label', () => {
    const view = resolveVoiceCallPhaseView('connected');
    assert.equal(view.phase, 'connected');
    assert.equal(view.isConnected, true);
    assert.equal(view.isError, false);
    assert.equal(view.statusColor, '#23a55a');
    assert.equal(view.label, 'Голосовая связь установлена • DAVE E2EE');
  });

  it('4. Negotiating Phase: Reports warning color and DAVE discovery', () => {
    const view = resolveVoiceCallPhaseView('negotiating');
    assert.equal(view.phase, 'negotiating');
    assert.equal(view.isConnected, false);
    assert.equal(view.statusColor, '#f0b232');
  });

  it('5. Error & Timeout Phases: Flags retryable error and red status', () => {
    const viewErr = resolveVoiceCallPhaseView({
      phase: 'error',
      reason: 'RTC Connection Failed: DTLS Handshake Timeout',
    });
    assert.equal(viewErr.phase, 'error');
    assert.equal(viewErr.isError, true);
    assert.equal(viewErr.canRetry, true);
    assert.equal(viewErr.statusColor, '#f23f43');
    assert.match(viewErr.reason, /DTLS Handshake Timeout/);

    const viewTimeout = resolveVoiceCallPhaseView('timeout');
    assert.equal(viewTimeout.phase, 'timeout');
    assert.equal(viewTimeout.isError, true);
    assert.equal(viewTimeout.canRetry, true);
  });

  it('6. AFK Phase: Displays muted status', () => {
    const viewAfk = resolveVoiceCallPhaseView('afk');
    assert.equal(viewAfk.phase, 'afk');
    assert.equal(viewAfk.isConnected, true);
    assert.match(viewAfk.label, /AFK/);
  });
});
