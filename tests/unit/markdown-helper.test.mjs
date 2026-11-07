// @ts-check
/**
 * T-218 (RW-40 / G2): контрактные тесты хелпера Markdown — спойлеры,
 * codeblock-независимые спойлеры и Discord timestamp-теги.
 *
 * Оракулы контрактов:
 * - Discord client: форматы `<t:UNIX>` / `<t:UNIX:STYLE>`, стили `t T d D f F R`,
 *   дефолтный стиль `f`;
 * - правило клиента: `||...||` внутри inline-бэктиков не является спойлером;
 * - русская относительная локализация как в `plural-format.js` (короткие единицы).
 */
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import {
  TIMESTAMP_STYLES,
  parseDiscordTimestampTag,
  extractSpoilerSpans,
  formatRelativeTimestampRu,
} from "../../rust/gui-spike/ui/features/markdown-helper.js";

const SOURCE = fileURLToPath(
  new URL("../../rust/gui-spike/ui/features/markdown-helper.js", import.meta.url),
);

test("markdown-helper: исходник помечен // @ts-check и содержит JSDoc", () => {
  const src = readFileSync(SOURCE, "utf8");
  assert.match(src, /^\/\/ @ts-check/m);
  assert.match(src, /@module/);
});

test("markdown-helper: TIMESTAMP_STYLES заморожен и совпадает с клиентом", () => {
  assert.ok(Object.isFrozen(TIMESTAMP_STYLES));
  assert.deepEqual([...TIMESTAMP_STYLES], ["t", "T", "d", "D", "f", "F", "R"]);
});

test("markdown-helper: parseDiscordTimestampTag разбирает валидные теги", () => {
  assert.deepEqual(parseDiscordTimestampTag("<t:1700000000>"), {
    unixSeconds: 1700000000,
    style: "f",
  });
  assert.deepEqual(parseDiscordTimestampTag("<t:1700000000:t>"), {
    unixSeconds: 1700000000,
    style: "t",
  });
  assert.deepEqual(parseDiscordTimestampTag("<t:1700000000:R>"), {
    unixSeconds: 1700000000,
    style: "R",
  });
  assert.deepEqual(parseDiscordTimestampTag("<t:-86400:D>"), {
    unixSeconds: -86400,
    style: "D",
  });
  assert.deepEqual(parseDiscordTimestampTag("<t:0:f>"), {
    unixSeconds: 0,
    style: "f",
  });
});

test("markdown-helper: parseDiscordTimestampTag отвергает невалидные теги", () => {
  const bad = [
    "<t:>",
    "<t:abc>",
    "<t:1700000000:>",
    "<t:1700000000:x>",
    "<t:1700000000:ff>",
    "<t:12.5>",
    "<t:1e9>",
    "<t:1700000000 >",
    "t:1700000000",
    "<t:1700000000",
    "",
    "   ",
  ];
  for (const raw of bad) {
    assert.equal(parseDiscordTimestampTag(raw), null, `должно быть null: ${raw}`);
  }
  assert.equal(parseDiscordTimestampTag(null), null);
  assert.equal(parseDiscordTimestampTag(undefined), null);
  assert.equal(parseDiscordTimestampTag(1700000000), null);
});

test("markdown-helper: extractSpoilerSpans собирает непустые спойлеры", () => {
  assert.deepEqual(extractSpoilerSpans("до ||секрет|| после ||второй||"), [
    "секрет",
    "второй",
  ]);
  assert.deepEqual(extractSpoilerSpans("||  пробелы  ||"), ["пробелы"]);
  assert.deepEqual(extractSpoilerSpans("без спойлеров"), []);
  assert.deepEqual(extractSpoilerSpans(""), []);
  assert.deepEqual(extractSpoilerSpans("||||"), []);
  assert.deepEqual(extractSpoilerSpans("||   ||"), []);
  assert.deepEqual(extractSpoilerSpans("||a|| ||b|| ||c||"), ["a", "b", "c"]);
});

test("markdown-helper: extractSpoilerSpans игнорирует inline-бэктики", () => {
  assert.deepEqual(extractSpoilerSpans("`||не спойлер||`"), []);
  assert.deepEqual(extractSpoilerSpans("код `||x||`, а это ||y||"), ["y"]);
  assert.deepEqual(extractSpoilerSpans("``||a||``"), []);
  assert.deepEqual(extractSpoilerSpans("||a`b`c||"), ["a`b`c"]);
});

test("markdown-helper: formatRelativeTimestampRu — секунды", () => {
  assert.equal(formatRelativeTimestampRu(1000, 1000), "только что");
  assert.equal(formatRelativeTimestampRu(1000, 1030), "только что");
  assert.equal(formatRelativeTimestampRu(1000, 941), "через несколько секунд");
});

test("markdown-helper: formatRelativeTimestampRu — минуты и часы", () => {
  assert.equal(formatRelativeTimestampRu(1000, 1060), "1 мин. назад");
  assert.equal(formatRelativeTimestampRu(1000, 1000 + 59 * 60 + 59), "59 мин. назад");
  assert.equal(formatRelativeTimestampRu(1000, 940), "через 1 мин.");
  assert.equal(formatRelativeTimestampRu(1000, 1000 + 3600), "1 ч. назад");
  assert.equal(formatRelativeTimestampRu(1000, 1000 + 3 * 3600 + 59 * 60), "3 ч. назад");
  assert.equal(formatRelativeTimestampRu(1000, 1000 - 5 * 3600), "через 5 ч.");
});

test("markdown-helper: formatRelativeTimestampRu — дни", () => {
  assert.equal(formatRelativeTimestampRu(0, 86400), "1 дн. назад");
  assert.equal(formatRelativeTimestampRu(0, 3 * 86400 + 500), "3 дн. назад");
  assert.equal(formatRelativeTimestampRu(0, -2 * 86400), "через 2 дн.");
  assert.equal(formatRelativeTimestampRu(0, 10 * 86400), "10 дн. назад");
});
