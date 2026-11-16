// @ts-check
/**
 * tests/unit/modules-channel-tree-view-contract.test.mjs
 *
 * Automated QA Contract Test for `channel-tree-view.js` (Modules & Components).
 * Validates:
 * 1. Phantom Avatar Filter: rejects optimistic, pendingConfirmation, and phantom records.
 * 2. Signaling / Error / Timeout Filter: rejects unconfirmed states.
 * 3. Confirmed Voice State: accepts confirmed user_id + channel_id pair.
 * 4. Boundary Cases: null/undefined/missing user_id or channel_id.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { isConfirmedVoiceStateRecord } from '../../rust/gui-spike/ui/modules/channel-tree-view.js';

describe('QA Contract — Channel Tree View Module', () => {
  it('1. Confirmed Participant: Validates real confirmed voice state update', () => {
    const valid = {
      user_id: '123456789012345678',
      channel_id: '987654321098765432',
      session_id: 'sess-live-99',
      mute: false,
      deaf: false,
    };
    assert.equal(isConfirmedVoiceStateRecord(valid), true);
  });

  it('2. Phantom / Optimistic Filter: Rejects unconfirmed optimistic entries', () => {
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', optimistic: true }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', confirmed: false }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', pendingConfirmation: true }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', phantom: true }), false);
  });

  it('3. Signaling & Error Filter: Rejects transitional or failed states', () => {
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', phase: 'signaling' }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', phase: 'timeout' }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', phase: 'error' }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', error: 'RTC Error' }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '2', timeout: true }), false);
  });

  it('4. Boundary Values: Rejects nullish or disconnected (channel_id = null) entries', () => {
    assert.equal(isConfirmedVoiceStateRecord(null), false);
    assert.equal(isConfirmedVoiceStateRecord(undefined), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: null }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '', channel_id: '2' }), false);
    assert.equal(isConfirmedVoiceStateRecord({ user_id: '1', channel_id: '' }), false);
  });

  it('5. CamelCase Property Parity: Accepts userId and channelId aliases', () => {
    const camel = {
      userId: '111222333444555666',
      channelId: '777888999000111222',
      state: 'connected',
    };
    assert.equal(isConfirmedVoiceStateRecord(camel), true);
  });
});
