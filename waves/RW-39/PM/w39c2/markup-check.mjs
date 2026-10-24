// W39c2: независимая проверка разметки — инварианты RW-38 (DOM-CONTRACT §2),
// id/классы §9.2, список <link> и запрет ассетов. Оракул — сам контракт
// (DESIGN/DOM-CONTRACT.md), а не ui/index.html. Запуск: node markup-check.mjs
import { existsSync, readFileSync } from "node:fs";

const ui = new URL("../../../../rust/gui-spike/ui/", import.meta.url);
const contract = new URL("../../DESIGN/DOM-CONTRACT.md", import.meta.url);
const html = readFileSync(new URL("index.html", ui), "utf8");
const md = readFileSync(contract, "utf8");

let fail = 0;
/** @param {boolean} ok @param {string} label @param {string} [detail] */
function check(ok, label, detail = "") {
  if (!ok) fail++;
  console.log(`${ok ? "ok  " : "FAIL"} | ${label}${detail ? ` | ${detail}` : ""}`);
}

/* 1. Экраны: ровно 4, порядок chat/emoji/members/settings. */
/** @type {string[]} */
const screens = [...html.matchAll(/id="screen-([\w-]+)"/g)].map((m) => m[1] ?? "");
check(
  JSON.stringify(screens) === JSON.stringify(["chat", "emoji", "members", "settings"]),
  "4 .screen в порядке chat/emoji/members/settings",
  screens.join(","),
);

/* 2. rail-btn: 4, порядок data-screen, data-screen только на .rail-btn. */
const railTags = [...html.matchAll(/<button[^>]*class="rail-btn[^"]*"[^>]*>/g)];
const railScreens = railTags.map((m) => /data-screen="([^"]+)"/.exec(m[0])?.[1] ?? "");
check(
  JSON.stringify(railScreens) === JSON.stringify(["chat", "emoji", "members", "settings"]),
  "4 .rail-btn в порядке chat/emoji/members/settings",
  railScreens.join(","),
);
const allScreenAttrs = [...html.matchAll(/data-screen="/g)].length;
check(allScreenAttrs === railScreens.length, "data-screen только на .rail-btn", `вхождений ${allScreenAttrs}`);
check(railTags.every((m) => /type="button"/.test(m[0])), "у всех .rail-btn есть type=button");
check(
  /data-active-screen="chat"/.test(html) && !/<body[^>]*data-screen=/.test(html),
  "body использует data-active-screen, не data-screen",
);

/* 3. Заголовок экрана. */
check(
  /<div class="screen-title" role="heading" aria-level="2" aria-live="polite">Чат · 100 000 сообщений<\/div>/.test(html),
  ".screen-title: aria-level=2, aria-live=polite, текст чата",
);

/* 4. Пустые контейнеры, которые заполняет W39d. */
for (const [id, re] of /** @type {Array<[string, RegExp]>} */ ([
  ["chat-list", /class="list chat-list"[^>]*data-row-height="44"[^>]*><\/div>/],
  ["member-list", /class="list member-list"[^>]*data-row-height="40"[^>]*><\/div>/],
  ["emoji-tabs", /class="emoji-tabs"[^>]*><!-- gen --><\/div>/],
  ["settings-nav", /class="settings-nav"[^>]*><!-- gen --><\/nav>/],
])) {
  check(re.test(html), `#${id} пустой, data-/ARIA на месте`);
}

/* 5. Роли и структурные требования §2/§4.4/§6.4. */
for (const [label, re, ref] of /** @type {Array<[string, RegExp, string]>} */ ([
  ["#emoji-grid role=listbox", /id="emoji-grid"[^>]*role="listbox"/, "P1-2"],
  ["#emoji-grid aria-label", /id="emoji-grid"[^>]*aria-label="[^"]+"/, "P1-2"],
  ["#emoji-tabs role=tablist", /id="emoji-tabs"[^>]*role="tablist"/, "§4.4"],
  ["#emoji-tabs aria-orientation", /id="emoji-tabs"[^>]*aria-orientation="vertical"/, "§4.4"],
  ["#settings role=group", /id="settings"[^>]*role="group"/, "§4.4"],
  ["#emoji-search — сам input", /<input id="emoji-search"[^>]*type="search"/, "§4.4"],
  ["#member-search — сам input", /<input id="member-search"[^>]*type="search"/, "§4.4"],
  ["#bench существует", /id="bench"[^>]*type="button"/, "§2.9"],
  ["#chat-header вне #screen-chat", /<\/header>[\s\S]*<!-- 6\. Экран chat/, "§3"],
  ["#member-panel внутри #screen-chat", /id="screen-chat"[\s\S]*id="member-panel"/, "§6.4"],
  ["#member-list только в #screen-members", /id="screen-members"[\s\S]*id="member-list"[\s\S]*id="screen-settings"/, "§6.4"],
])) {
  check(re.test(html), label, ref);
}

/* 6. id/классы: ровно списки §9.2. */
const section92 = md.slice(md.indexOf("**id (29):**"));
const idsParagraph = section92.slice(0, section92.indexOf("**Классы"));
const classesParagraph = section92
  .slice(section92.indexOf("**Классы"))
  .split(/\n\s*\n/)[0] ?? "";
const expectedIds = [...idsParagraph.matchAll(/`([\w-]+)`/g)].map((m) => m[1] ?? "");
const expectedClasses = [...classesParagraph.matchAll(/`([\w-]+)`/g)].map((m) => m[1] ?? "");
const actualIds = [...new Set([...html.matchAll(/ id="([^"]+)"/g)].map((m) => m[1] ?? ""))];
const actualClasses = new Set();
for (const m of html.matchAll(/class="([^"]+)"/g)) {
  for (const name of (m[1] ?? "").split(/\s+/)) if (name) actualClasses.add(name);
}
const missingIds = expectedIds.filter((id) => !actualIds.includes(id));
const extraIds = actualIds.filter((id) => !expectedIds.includes(id));
const missingClasses = expectedClasses.filter((name) => !actualClasses.has(name));
const extraClasses = [...actualClasses].filter((name) => !expectedClasses.includes(name));
check(
  expectedIds.length === 29 && missingIds.length === 0 && extraIds.length === 0,
  "id совпадают со списком §9.2 (29)",
  `нет: [${missingIds}] лишние: [${extraIds}]`,
);
check(
  expectedClasses.length === 76 && missingClasses.length === 0 && extraClasses.length === 0,
  "классы совпадают со списком §9.2 (76)",
  `нет: [${missingClasses}] лишние: [${extraClasses}]`,
);

/* 7. Ассеты: ни url(), ни @font-face; бренд — текстом DL. */
check(!/url\(/.test(html) && !/@font-face/.test(html), "в index.html нет url()/@font-face");
check(/<div class="brand"[^>]*>DL<\/div>/.test(html), "бренд — текст DL");
const discordMentions = [...html.matchAll(/discord/gi)].length;
check(discordMentions === 2, "«discord» только в <title> и title бренда (имя проекта)", `вхождений ${discordMentions}`);

/* 8. Каждый <link> ведёт на существующий непустой файл. */
const links = [...html.matchAll(/<link[^>]*href="([^"]+)"/g)].map((m) => m[1] ?? "");
check(links.length === 9, "9 <link rel=stylesheet>", links.length.toString());
for (const href of links) {
  const file = new URL(href, ui);
  check(existsSync(file) && readFileSync(file, "utf8").trim().length > 0, `<link> ${href} существует и не пуст`);
}

/* 9. Побайтовая сверка с §3: допустимы только две задокументированные правки
 *    (путь токенов — D-05; role=grid→listbox + без aria-colcount — frozen тест)
 *    и комментарии W39c2. Всё остальное обязано совпадать строка-в-строку. */
const canon = /```html\n([\s\S]*?)```/.exec(md)?.[1] ?? "";
check(canon.length > 0, "канонический скелет §3 найден в контракте");
const expected = canon
  .replace('<link rel="stylesheet" href="components/tokens.css" />', '<link rel="stylesheet" href="tokens.css" />')
  .replace(
    '<div id="emoji-grid" class="emoji-grid" role="grid" aria-label="Эмодзи" aria-colcount="9"><!-- gen --></div>',
    '<div id="emoji-grid" class="emoji-grid" role="listbox" aria-label="Эмодзи"><!-- gen --></div>',
  )
  .split("\n")
  .map((line) => line.trimEnd())
  .filter(Boolean);
const ownComments = ["W39c2: токены", "role=listbox + ячейки role=option", "aria-colcount/aria-rowcount"];
const actual = html
  .split("\n")
  .map((line) => line.trimEnd())
  .filter((line) => line.trim() !== "")
  .filter((line) => !ownComments.some((marker) => line.includes(marker)));
/** @type {string[]} */
const firstDiff = [];
if (expected.length !== actual.length) firstDiff.push(`строк ${actual.length} вместо ${expected.length}`);
for (let i = 0; i < Math.min(expected.length, actual.length); i++) {
  if (expected[i] !== actual[i]) {
    firstDiff.push(`строка ${i + 1}: «${actual[i]}» ≠ «${expected[i]}»`);
    break;
  }
}
check(firstDiff.length === 0, "index.html = §3 (кроме D-05 и frozen role)", firstDiff.join("; "));

if (fail > 0) {
  console.error(`\n${fail} расхождений разметки`);
  process.exit(1);
}
console.log("\nразметка: все проверки пройдены");
