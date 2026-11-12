# TZ-W25b — markdown сообщений: 23 правила (P7-01 / T-MSG-01)

Ты — **W25b**, кодовый воркер. Пиши только файлы из §1. `src/lib.rs` — интеграционный,
не трогай. Сначала red-тесты, затем минимальный green. Оракул — `docs/re/i18n-deep.md`
§3 (регулярки/порядок simple-markdown) и таблица §4 ниже. Не подгоняй тесты под код.

## 1. Файлы

Пишешь:
- `rust/crates/discord-i18n/src/markdown.rs`
- `rust/crates/discord-i18n/tests/markdown_contract.rs`

Читаешь, но не меняешь:
- `docs/re/i18n-deep.md` §3, §4
- `waves/RW-24/WAVE-PLAN.md`

Не трогаешь: `src/lib.rs`, `src/locale.rs`, `src/plural.rs`, `src/format.rs`,
`src/format_data.rs`, `tests/data/*`, `Cargo.toml`, чужие тесты/доки. Зависимостей
не добавлять.

## 2. API (заморожен)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownNode {
    Text(String),
    Sequence(Vec<MarkdownNode>),
    Bold(Vec<MarkdownNode>),
    Underline(Vec<MarkdownNode>),
    Strike(Vec<MarkdownNode>),
    Italic(Vec<MarkdownNode>),
    InlineCode(String),
    CodeBlock { language: String, code: String },
    Spoiler(Vec<MarkdownNode>),
    BlockQuote(Vec<MarkdownNode>),
    Heading { level: u8, children: Vec<MarkdownNode> },
    Subtext(Vec<MarkdownNode>),
    List { ordered: bool, start: Option<u64>, items: Vec<Vec<MarkdownNode>> },
    Silent,
    UserMention { id: String, nickname: bool },
    Everyone,
    Here,
    RoleMention(String),
    ChannelMention(String),
    SlashCommand { name: String, id: String },
    CustomEmoji { animated: bool, name: String, id: String },
    Emoji { name: String, skin_tone: Option<u8> },
    Timestamp { unix: i64, style: Option<char> },
    Soundboard { guild_id: String, sound_id: String },
    StaticRoute { route: String, id: Option<String> },
    Link { label: String, url: String },
    AutoLink(String),
}

pub fn parse_markdown(input: &str) -> MarkdownNode;
pub fn render_markdown(node: &MarkdownNode) -> String;
```

`render_markdown` — нормализованный Discord-markdown (не HTML). `parse_markdown`
не паникует на любом входе (включая обрезанный UTF-8-граничный срез, пустую строку,
незакрытые маркеры, глубину >32). Внутренние лимиты: inline-depth ≤32, list-depth ≤11;
при превышении — literal text, без паники.

## 3. Порядок и безопасность

1. Блочные правила: fence → blockquote → heading 1–3 → subtext `-#` → list → paragraph.
2. Inline: escape `\X` (X не alphanumeric/space) → inline code → custom emoji/mentions/
   timestamp/sound/static route → markdown link → autolink/url → bold/underline/strike/
   spoiler/italic. Стили не должны «съедать» уже распознанные объекты.
3. `safe_url`: разрешены только `http://`, `https://`, `mailto:` (case-insensitive,
   trim); `javascript:`, `vbscript:`, `data:` и прочее — URL отбрасывается, остаётся
   текст label. HTML не экранируется и не исполняется: `<script>` остаётся текстом.
4. Незакрытые маркеры — literal; пустые пары (`****`) — literal.
5. ID: user/role/channel/slash/sound/static — ASCII digits (`1+`); custom emoji —
   `17..=19` digits; timestamp — `-?\d{1,17}`; style только `t T d D f F s S R`.
6. `@silent` — только с boundary (`@silentx` — текст); `@everyone`/`@here` — отдельные
   узлы; `<@!123>` сохраняет `nickname=true`.

## 4. 23 правила и ожидаемый `render_markdown`

| # | Вход | Выход |
|---:|---|---|
| 1 | `**bold**` | `**bold**` |
| 2 | `__under__` | `__under__` |
| 3 | `~~strike~~` | `~~strike~~` |
| 4 | `*em*` / `_em_` | `*em*` |
| 5 | `` `code` `` | `` `code` `` |
| 6 | ` ```rs\nlet x = 1;\n``` ` | тот же fence (lang + code) |
| 7 | `\|\|spoiler\|\|` | `\|\|spoiler\|\|` |
| 8 | `> quote` | `> quote` |
| 9 | `# H1` / `## H2` / `### H3` | тот же заголовок; `#### H4` — literal |
| 10 | `-# small` | `-# small` |
| 11 | `- a\n- b` / `1. a\n2. b` | тот же список (ordered start) |
| 12 | `@silent` | `@silent` |
| 13 | `<@123>` / `<@!123>` / `@everyone` / `@here` | те же (nickname сохраняется) |
| 14 | `<@&123>` | `<@&123>` |
| 15 | `<#123>` | `<#123>` |
| 16 | `</cmd arg:123>` | `</cmd arg:123>` |
| 17 | `<a:wave:123456789012345678>` / `<:wave:123456789012345678>` | те же |
| 18 | `:wave:` / `:wave::skin-tone-3:` | те же |
| 19 | `<t:1712345678:R>` | `<t:1712345678:R>` |
| 20 | `<sound:123:456>` | `<sound:123:456>` |
| 21 | `<id:home>` / `<id:browse:123>` | те же |
| 22 | `[x](https://a.test)` / `<https://a.test>` / `https://a.test` | `[x](https://a.test)` / `[https://a.test](https://a.test)` / `[https://a.test](https://a.test)` |
| 23 | `\*` / `\\` / `\a` | `*` / `\` / `\a` |

Дополнительно:
- unsafe: `[x](javascript:alert(1))` → `x`; `[x](data:text/html,...)` → `x`;
  `<https://safe.test>` безопасен; `[x](https://safe.test)` сохраняется.
- nested: `**bold *em***` → `**bold *em***` (или эквивалент, сохраняющий оба стиля);
  `**open` → `**open`; `||open` → `||open`; `****` → `****`.
- fence unclosed ` ```\nbody` → literal (без паники).
- `#` без пробела (`#tag`) — не заголовок; `-#` без пробела — текст.
- list depth: 12 вложенных `  - x` не паникует; глубина >11 не создаёт бесконечную
  рекурсию (можно схлопнуть в текст).
- `render_markdown(parse_markdown("")) == ""`.
- `@silentx`, `<@abc>`, `<:x:123>` (id <17) — literal text.
- HTML `<script>alert(1)</script>` остаётся текстом, не исполняется.

## 5. Тесты

`tests/markdown_contract.rs`:
- таблица 23 правил (можно массив `(input, expected)`), каждый — отдельный assert
  с номером правила;
- unsafe URL, HTML, unclosed, empty, boundary `@silentx`, invalid IDs;
- nested bold/italic, code protection (`**\`x\`**`), escape;
- fuzz-lite: 2 000 сгенерированных строк + все char-boundary срезы одной строки,
  без паник; проверка, что `render_markdown` не содержит `javascript:`/`data:`.

Сначала red-лог `waves/RW-24/red-W25b.log` (команда + падение), затем green
`waves/RW-24/green-W25b.log`. Команда:
`flock /tmp/opencode/discord-cargo.lock cargo test --offline --manifest-path rust/Cargo.toml -p discord-i18n --test markdown_contract`.
Затем `cargo fmt --all -- --check` и
`flock /tmp/opencode/discord-cargo.lock cargo clippy --offline --manifest-path rust/Cargo.toml -p discord-i18n --all-targets -- -D warnings`.

## 6. Запреты

- Никаких сетевых вызовов, новых зависимостей, `src/lib.rs`, `Cargo.toml`, `waves/*`.
- Не «зеленить» тесты правкой ожиданий; таблица — контракт.
- Не трогать `logs/`, `functions/`, `evidence/`, `reference-oracle/`.
- Не коммитить.
