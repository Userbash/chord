// @ts-check
import test from 'node:test';
import assert from 'node:assert/strict';

import {
  MAX_INVITE_EMBEDS,
  extractInviteCodes,
  formatInviteSummaryRu,
} from '../../rust/gui-spike/ui/features/invite-parser.js';

test('MAX_INVITE_EMBEDS равен 3', () => {
  assert.equal(MAX_INVITE_EMBEDS, 3);
});

test('extractInviteCodes: все поддерживаемые формы ссылок', () => {
  /** @type {Array<[string, string]>} */
  const cases = [
    ['https://discord.gg/abc', 'abc'],
    ['http://discord.gg/def', 'def'],
    ['discord.gg/ghi', 'ghi'],
    ['https://discord.com/invite/jkl', 'jkl'],
    ['https://discordapp.com/invite/mno', 'mno'],
  ];
  for (const [text, code] of cases) {
    assert.deepEqual(extractInviteCodes(`заходи ${text} скорее`), [code], text);
  }
});

test('extractInviteCodes: три формы в одном тексте дают три кода по порядку', () => {
  const text = 'a https://discord.gg/abc b https://discord.com/invite/jkl c discordapp.com/invite/mno';
  assert.deepEqual(extractInviteCodes(text), ['abc', 'jkl', 'mno']);
});

test('extractInviteCodes: дедупликация с сохранением порядка первого вхождения', () => {
  const text = 'https://discord.gg/abc https://discord.com/invite/xyz discord.gg/abc';
  assert.deepEqual(extractInviteCodes(text), ['abc', 'xyz']);
});

test('extractInviteCodes: ограничение MAX_INVITE_EMBEDS', () => {
  const text = 'a discord.gg/c1 b discord.gg/c2 c discord.gg/c3 d discord.gg/c4 e discord.gg/c5';
  const codes = extractInviteCodes(text);
  assert.equal(codes.length, MAX_INVITE_EMBEDS);
  assert.deepEqual(codes, ['c1', 'c2', 'c3']);
});

test('extractInviteCodes: код из [A-Za-z0-9-]{2,32}, минимум 2 символа', () => {
  assert.deepEqual(extractInviteCodes('discord.gg/a https://discord.gg/ab-12-CD'), ['ab-12-CD']);
});

test('extractInviteCodes: пустой/невалидный ввод даёт пустой массив', () => {
  assert.deepEqual(extractInviteCodes(''), []);
  assert.deepEqual(extractInviteCodes('нет ссылок тут'), []);
  assert.deepEqual(extractInviteCodes(/** @type {any} */ (null)), []);
  assert.deepEqual(extractInviteCodes(/** @type {any} */ (undefined)), []);
  assert.deepEqual(extractInviteCodes('https://discord.gg/'), []);
});

test('formatInviteSummaryRu: недействительное приглашение (null)', () => {
  assert.deepEqual(formatInviteSummaryRu(null), {
    valid: false,
    title: 'Приглашение недействительно',
    subtitle: 'Срок действия приглашения истёк или оно было удалено',
    buttonText: 'Недоступно',
  });
});

test('formatInviteSummaryRu: истёкшее приглашение', () => {
  assert.deepEqual(formatInviteSummaryRu({ expired: true, guildName: 'X' }), {
    valid: false,
    title: 'Приглашение недействительно',
    subtitle: 'Срок действия приглашения истёк или оно было удалено',
    buttonText: 'Недоступно',
  });
});

test('formatInviteSummaryRu: валидное приглашение, не вступил', () => {
  assert.deepEqual(
    formatInviteSummaryRu({ guildName: 'Мой сервер', onlineCount: 12, memberCount: 340 }),
    {
      valid: true,
      title: 'Мой сервер',
      subtitle: 'В сети: 12 • Участников: 340',
      buttonText: 'Вступить',
    },
  );
});

test('formatInviteSummaryRu: уже участник -> кнопка «Перейти»', () => {
  const res = formatInviteSummaryRu({ guildName: 'G', onlineCount: 1, memberCount: 2, joined: true });
  assert.equal(res.buttonText, 'Перейти');
  assert.equal(res.valid, true);
});

test('formatInviteSummaryRu: fallback имени и нормализация чисел', () => {
  const res = formatInviteSummaryRu({ onlineCount: -5, memberCount: 3.9 });
  assert.equal(res.title, 'Сервер Discord');
  assert.equal(res.subtitle, 'В сети: 0 • Участников: 3');
});

test('formatInviteSummaryRu: отсутствующие счётчики дают 0', () => {
  const res = formatInviteSummaryRu({ guildName: 'G' });
  assert.equal(res.subtitle, 'В сети: 0 • Участников: 0');
});

test('formatInviteSummaryRu: нечисловые счётчики дают 0', () => {
  const res = formatInviteSummaryRu({ guildName: 'G', onlineCount: 'abc', memberCount: null });
  assert.equal(res.subtitle, 'В сети: 0 • Участников: 0');
});
