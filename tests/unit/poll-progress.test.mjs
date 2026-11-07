// @ts-check
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  computePollResults,
  formatPollExpiryRu,
  togglePollAnswerSelection,
} from '../../rust/gui-spike/ui/features/poll-progress.js';

describe('computePollResults', () => {
  const answers = [
    { answerId: 'a1', text: 'Да' },
    { answerId: 'a2', text: 'Нет' },
    { answerId: 'a3', text: 'Воздержался' },
  ];

  it('counts total votes and percentages', () => {
    const counts = [
      { id: 'a1', count: 3, meVoted: true },
      { id: 'a2', count: 1 },
      { id: 'a3', count: 0 },
    ];
    const result = computePollResults(answers, counts);
    assert.equal(result.totalVotes, 4);
    assert.deepEqual(
      result.items.map((i) => [i.answerId, i.count, i.percentage, Boolean(i.meVoted), i.isLeading]),
      [
        ['a1', 3, 75, true, true],
        ['a2', 1, 25, false, false],
        ['a3', 0, 0, false, false],
      ],
    );
    assert.equal(result.items[1]?.text, 'Нет');
  });

  it('rounds percentages to the nearest integer', () => {
    const counts = [
      { id: 'a1', count: 1 },
      { id: 'a2', count: 2 },
    ];
    const result = computePollResults(answers.slice(0, 2), counts);
    assert.equal(result.totalVotes, 3);
    assert.equal(result.items[0]?.percentage, 33);
    assert.equal(result.items[1]?.percentage, 67);
  });

  it('marks every answer tied at the max count as leading', () => {
    const counts = [
      { id: 'a1', count: 2 },
      { id: 'a2', count: 2 },
      { id: 'a3', count: 1 },
    ];
    const result = computePollResults(answers, counts);
    assert.deepEqual(result.items.map((i) => i.isLeading), [true, true, false]);
  });

  it('returns zero percentages and no leaders when nobody voted', () => {
    const counts = [
      { id: 'a1', count: 0 },
      { id: 'a2', count: 0 },
    ];
    const result = computePollResults(answers.slice(0, 2), counts);
    assert.equal(result.totalVotes, 0);
    assert.deepEqual(result.items.map((i) => i.percentage), [0, 0]);
    assert.deepEqual(result.items.map((i) => i.isLeading), [false, false]);
  });

  it('keeps answers with no matching count entry at zero', () => {
    const result = computePollResults(answers, [{ id: 'a1', count: 5 }]);
    assert.equal(result.totalVotes, 5);
    assert.deepEqual(result.items.map((i) => i.count), [5, 0, 0]);
    assert.deepEqual(result.items.map((i) => Boolean(i.meVoted)), [false, false, false]);
  });

  it('handles empty answers', () => {
    assert.deepEqual(computePollResults([], []), { totalVotes: 0, items: [] });
  });
});

describe('formatPollExpiryRu', () => {
  const now = 1_700_000_000_000;

  it('reports finished poll for missing or invalid expiry', () => {
    assert.equal(formatPollExpiryRu(null, now), 'Опрос завершён');
    assert.equal(formatPollExpiryRu(undefined, now), 'Опрос завершён');
    assert.equal(formatPollExpiryRu(Number.NaN, now), 'Опрос завершён');
    assert.equal(formatPollExpiryRu(Number.POSITIVE_INFINITY, now), 'Опрос завершён');
    assert.equal(formatPollExpiryRu(now, now), 'Опрос завершён');
    assert.equal(formatPollExpiryRu(now - 1, now), 'Опрос завершён');
  });

  it('reports less than a minute', () => {
    assert.equal(formatPollExpiryRu(now + 1, now), 'Осталось меньше минуты');
    assert.equal(formatPollExpiryRu(now + 59_999, now), 'Осталось меньше минуты');
  });

  it('reports minutes rounded up', () => {
    assert.equal(formatPollExpiryRu(now + 60_000, now), 'Осталось 1 мин.');
    assert.equal(formatPollExpiryRu(now + 60_001, now), 'Осталось 2 мин.');
    assert.equal(formatPollExpiryRu(now + 3_599_999, now), 'Осталось 60 мин.');
  });

  it('reports hours rounded up', () => {
    assert.equal(formatPollExpiryRu(now + 3_600_000, now), 'Осталось 1 ч.');
    assert.equal(formatPollExpiryRu(now + 5_400_000, now), 'Осталось 2 ч.');
    assert.equal(formatPollExpiryRu(now + 86_399_999, now), 'Осталось 24 ч.');
  });

  it('reports days rounded up', () => {
    assert.equal(formatPollExpiryRu(now + 86_400_000, now), 'Осталось 1 дн.');
    assert.equal(formatPollExpiryRu(now + 86_400_001, now), 'Осталось 2 дн.');
    assert.equal(formatPollExpiryRu(now + 7 * 86_400_000, now), 'Осталось 7 дн.');
  });
});

describe('togglePollAnswerSelection', () => {
  it('behaves as a radio when multiselect is disabled', () => {
    assert.deepEqual(togglePollAnswerSelection([], 'a1'), ['a1']);
    assert.deepEqual(togglePollAnswerSelection(['a1'], 'a2'), ['a2']);
    assert.deepEqual(togglePollAnswerSelection(['a1'], 'a1'), []);
  });

  it('adds and removes answers when multiselect is enabled', () => {
    assert.deepEqual(togglePollAnswerSelection(['a1'], 'a2', true), ['a1', 'a2']);
    assert.deepEqual(togglePollAnswerSelection(['a1', 'a2'], 'a1', true), ['a2']);
  });

  it('refuses new answers once maxChoices is reached', () => {
    const selected = ['a1', 'a2'];
    const next = togglePollAnswerSelection(selected, 'a3', true, 2);
    assert.deepEqual(next, ['a1', 'a2']);
    assert.notEqual(next, selected);
  });

  it('still allows removing an answer at maxChoices', () => {
    assert.deepEqual(togglePollAnswerSelection(['a1', 'a2'], 'a2', true, 2), ['a1']);
  });

  it('does not mutate the input array', () => {
    const selected = ['a1'];
    togglePollAnswerSelection(selected, 'a2', true);
    assert.deepEqual(selected, ['a1']);
  });
});
