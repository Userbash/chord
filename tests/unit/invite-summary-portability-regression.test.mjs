// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';
import { formatInviteSummaryRu } from '../../rust/gui-spike/ui/features/invite-parser.js';

// Independent contract: task-199 REPORT.md and approved REVIEW-110A.md specify
// Russian labels, the fallback «Сервер Discord», and nonnegative integer counts.
test('Russian invite summary uses the documented fallback when the name is omitted', () => {
  const invite = Object.freeze({ onlineCount: -5, memberCount: '3.9' });
  assert.deepEqual(formatInviteSummaryRu(invite), {
    valid: true,
    title: 'Сервер Discord',
    subtitle: 'В сети: 0 • Участников: 3',
    buttonText: 'Вступить',
  });
  assert.deepEqual(invite, { onlineCount: -5, memberCount: '3.9' });
});

test('Russian invite summary applies the same fallback to an empty name', () => {
  assert.deepEqual(formatInviteSummaryRu(Object.freeze({
    guildName: '', onlineCount: '12.9', memberCount: -2.5, joined: true,
  })), {
    valid: true,
    title: 'Сервер Discord',
    subtitle: 'В сети: 12 • Участников: 0',
    buttonText: 'Перейти',
  });
});

test('Russian invite summary preserves supplied names including product names and Unicode', () => {
  for (const guildName of ['Chord Server', 'Сервер Discord', 'Свой сервер 🎵']) {
    assert.deepEqual(formatInviteSummaryRu(Object.freeze({
      guildName, onlineCount: 12, memberCount: 340,
    })), {
      valid: true,
      title: guildName,
      subtitle: 'В сети: 12 • Участников: 340',
      buttonText: 'Вступить',
    });
  }
});

test('Russian invite summary keeps absent and malformed counters at zero', () => {
  assert.deepEqual(formatInviteSummaryRu(Object.freeze({})), {
    valid: true,
    title: 'Сервер Discord',
    subtitle: 'В сети: 0 • Участников: 0',
    buttonText: 'Вступить',
  });
  assert.deepEqual(formatInviteSummaryRu(Object.freeze({
    guildName: 'Свой сервер', onlineCount: 'not a count', memberCount: null,
  })), {
    valid: true,
    title: 'Свой сервер',
    subtitle: 'В сети: 0 • Участников: 0',
    buttonText: 'Вступить',
  });
});

test('Russian invite summary keeps the unavailable state for absent and expired invites', () => {
  for (const invite of [null, undefined, Object.freeze({ expired: true, guildName: 'Свой сервер', joined: true })]) {
    assert.deepEqual(formatInviteSummaryRu(invite), {
      valid: false,
      title: 'Приглашение недействительно',
      subtitle: 'Срок действия приглашения истёк или оно было удалено',
      buttonText: 'Недоступно',
    });
  }
});
