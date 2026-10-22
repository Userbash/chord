# TZ W32a — P6-07 Spellcheck: hunspell через FFI (headless)

Волна: RW-32. Дата: 2026-10-22. Крейт: `discord-platform`. Сеть/live не используются.
Системная библиотека `/usr/lib/libhunspell-1.7.so` (1.7.5).

## 1. Замороженный контракт

```rust
pub struct Token { pub text: String, pub start: usize, pub end: usize } // byte offsets

pub struct SpellChecker { /* *mut Hunspell */ }
pub struct SpellError { /* ... */ }
impl SpellChecker {
    pub fn new(aff: &std::path::Path, dic: &std::path::Path) -> Result<Self, SpellError>;
    pub fn check(&mut self, word: &str) -> bool;
    pub fn suggest(&mut self, word: &str) -> Vec<String>;
    pub fn add(&mut self, word: &str) -> bool;
    pub fn remove(&mut self, word: &str) -> bool;
}

/// Чистый Rust-токенайзер: слова с байтовыми смещениями.
pub fn tokenize(text: &str) -> Vec<Token>;
```

Инварианты:

- **FFI**: `#[link(name = "hunspell-1.7")]`; весь `unsafe` — в `src/ffi_hunspell.rs`
  (`#[allow(unsafe_code)]`, `// SAFETY:` на каждый блок); `Cargo.toml` крейта —
  `unsafe_code = "deny"`. Вне ffi — без `unsafe`/`unwrap`/`expect`/`panic!`.
- `new`: `hunspell_create(aff, dic)`; NULL → `SpellError` (файлы/битый словарь), без паники.
  Пути передаются C-строками без NUL (NUL → `SpellError`).
- `check`/`suggest`/`add`/`remove` делегируют в hunspell (`hunspell_spell`, `hunspell_suggest`
  — список освобождается `hunspell_free_list`, `hunspell_add`, `hunspell_remove`).
  `suggest` возвращает не более 10 вариантов, детерминированный порядок hunspell.
- `Drop` вызывает `hunspell_destroy` ровно один раз; `SpellChecker` — `!Send + !Sync`.
- `tokenize`: слово = последовательность Unicode-букв (`char::is_alphabetic`), внутри слова
  допускаются `'` и `-` (не в начале/конце); цифры и `_` разрывают слово. Пропускаются:
  URL (`://`), e-mail-подобные (`@`), содержимое обратных кавычек `` `…` `` и `code`-фрагментов
  (`` ` ``), упоминания/каналы `<@…>`, `<#…>`, `:emoji:` не считаются словами. Смещения —
  байтовые в исходной строке, `start < end`, `&text[start..end] == text`.
- Пустой текст → пустой вектор; текст без слов → пустой; не паникует на любом Unicode.

## 2. Данные

`fixtures/hunspell/test.aff` + `test.dic` (синтетический словарь: `hello`, `world`,
`привет`, `мир`, `don't`, `well-known`); словарь не является копией системных словарей.
В тестах — temp-копия фикстур (не менять исходники).

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| h01 | `new` на фикстурах | Ok |
| h02 | check: hello/привет/don't | true |
| h03 | check: helo/приветъ | false |
| h04 | suggest("helo") | содержит "hello", ≤10 |
| h05 | add/remove нового слова | check меняется |
| h06 | tokenize ASCII + пунктуация | смещения и слова |
| h07 | tokenize кириллица/дефис/апостроф | корректно |
| h08 | tokenize URL/e-mail/`code`/`<@id>`/`:emoji:` | пропущены |
| h09 | пустой текст и текст без слов | [] |
| h10 | битый/отсутствующий словарь | `SpellError`, без паники |
| h11 | `Drop` 100 циклов create/destroy | нет падения/утечки-краша |
| h12 | NUL в пути | `SpellError` |

Бюджет: 11–13 тестов; мутации 6–8 (границы слова, пропуск URL, suggest-лимит, add/remove,
Drop, NUL-guard).

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w32a cargo test --offline -p discord-platform --test spellcheck_contract
```

## 5. DoD

red-лог с exit code → green → мутации (`--nocapture`, SKIP=0, restore по md5) → отчёт
`waves/RW-32/REPORT-W32a.md`.
