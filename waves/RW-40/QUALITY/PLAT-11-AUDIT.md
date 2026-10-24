# PLAT-11 — аудит инварианта «IPC не принимает произвольный путь»

Дата: 2026-10-24. HEAD: `0239543`. Режим: **read-only** (код/тесты/доки не менялись;
live/сеть/окно/commit не запускались). PM: DeepSeek. Аудитор: G4b/G5.
Метки: `[D]` — цитата кода/файла с путём и строкой, `[A]` — не подтверждено, gap.

Предмет: `QUALITY/PLATFORM-AUDIT.md` PLAT-11 (`STUB-13` attachments + будущий capture),
`QUEUE.md` §4 rank 6, `BUGS.md` строка PLAT-11.

---

## 0. Вердикт

| Вопрос | Ответ |
|---|---|
| Есть ли сейчас уязвимость «IPC читает любой файл»? | **Нет.** Ни одна из **29** `#[tauri::command]` не принимает путь: 0 аргументов типа `PathBuf`/`Path`/`&Path`, 0 аргументов с именем `path`/`file`/`dir`/`root` `[D]` |
| Есть ли дефект того же класса? | **Да, P2:** `send_message` подставляет `channelId` из webview в **URL-путь** без allowlist символов; остальные 7 мест с id валидируют строго (`messages.rs:147-153` против `messages.rs:267-274`) |
| Закрыт ли контроль-гэп из PLAT-11? | **Нет.** Meta-теста, который удержит инвариант при появлении attachments/capture, не существует; API-поверхность растёт (29 команд), а защиты «на будущее» нет |
| Что даёт этот аудит | 3 red-first теста (§4), контракт владения путём (§2), запрет на проектирование upload/capture «по аналогии» до `ATTACH-RE-live` (§6) |

Итог по приоритету: **P1 — контроль** (инвариант + meta-тест), **P2 — код**
(`send_message`). P0 нет: эксплуатация требует XSS в webview, а `innerHTML` в `ui/**` = 0
(G2) и upload заблокирован (`ATTACH-RE.md:23-33`).

---

## 1. Где IPC соприкасается с путём/URL/файлом

Все команды зарегистрированы одним списком `main.rs:559-589` (29 записей).
Корень доступа к ФС берётся **не из IPC**, а из окружения процесса:
`DL_FIXTURES` (`data.rs:75-99`) и `DL_SETTINGS_PATH` (`settings_store.rs:360-375`).
Значит граница доверия для путей — launcher/окружение, а не webview. Это правильно,
но нигде не записано как инвариант `[D]`.

### 1.1 Команды, читающие ФС

| Команда | Аргументы от frontend | Что открывает | Как ограничено | Вердикт |
|---|---|---|---|---|
| `ready` | — | `<root>/gateway/by-type/*` | root из env, константные сегменты `[D]` `data.rs:277` | OK |
| `messages_page` | `channel_id`, `cursor`, `limit` | те же кадры | `channel_id` **не** идёт в путь: allowlist `known_channels()` `[D]` `data.rs:592-601,862` | OK |
| `members_page` | `guild_id`, `cursor`, `limit` | те же кадры | lookup в карте гильдий `[D]` `data.rs:1360-1381` | OK |
| `settings` | `section` | `<root>/gateway/by-type/002-READY.json` | `section` — ключ в JSON, не путь `[D]` `data.rs:458` | OK |
| `search_page` | `query`, `filters`, `cursor`, `limit` | те же кадры | фильтры сравниваются со строками `[D]` `data.rs:1525` | OK |
| `settings_load` | — | `DL_SETTINGS_PATH` | путь только из env `[D]` `settings_store.rs:372,394` | OK |
| `settings_save` | `settings: UiSettings` | тот же файл | `UiSettings` — 5 полей, пути нет `[D]` `settings_store.rs:137-148` | OK |
| `process_metrics` | — | `/proc/self/status` | литерал `[D]` `main.rs:50` | OK |

Косвенные ФС-чтения в библиотеке: `/etc/timezone` (`auth.rs:417`, литерал) и обход
`by-type` (`data.rs:264,278`). Пути-константы, вход пользователя не участвует.

### 1.2 Команды, подставляющие вход в URL-путь

| Команда | Поле | Что подставляется | Валидация | Вердикт |
|---|---|---|---|---|
| `search_remote` | `guild_id` | `%`-encode + `render_path` `[D]` `search.rs:190-200` | пусто/`{}`/CRLF → `InvalidArg`, затем percent-encode | OK |
| `join_guild` | `guild_id` | `render_path` `[D]` `guilds.rs:337-342` | `is_snowflake` (только цифры, `u64`) | OK |
| `leave_guild` | `guild_id` | конкатенация `[D]` `guilds.rs:348-351` | `is_snowflake` | OK |
| `edit_message` | `channel_id`,`message_id` | `format!("/channels/…")` `[D]` `messages.rs:215-217` | `validate_ids` = только ASCII-цифры `[D]` `messages.rs:267-274` | OK |
| `delete_message` | `channel_id`,`message_id` | то же `[D]` `messages.rs:235-236` | `validate_ids` | OK |
| `reply_message` | `channel_id`,`reply_to` | то же `[D]` `messages.rs:249-251` | `validate_ids` | OK |
| `toggle_reaction` | `channel_id`,`message_id`,`emoji` | `render_path` `[D]` `reactions.rs:117-130` | `is_numeric_id` + percent-encode emoji `[D]` `reactions.rs:166-178` | OK |
| **`send_message`** | `channel_id`, `content`, `nonce` | `render_path` `[D]` `messages.rs:154-159` | **только** непусто и нет `{}`/CRLF `[D]` `messages.rs:147-153` | **дефект §3.2** |

Хост не подменяется: `API_V9_BASE = "https://discord.com/api/v9"` `[D]`
`discord-core/src/lib.rs:75`; транспорт требует, чтобы target начинался с `/` и не содержал
`#`/непечатаемых байт `[D]` `transport.rs:217-224`. SSRF в чужой хост невозможен.

### 1.3 Команды без путей

`spike_config`, `report_bench`, `report_ui_snapshot`, `auth_*` (6), `net_*` (3) — только
скаляры/структуры, пути не принимают и не открывают `[D]` (сигнатуры `main.rs:48-520`).

### 1.4 Фронтенд

В `ui/**` нет `type="file"`, `FileReader`, `showOpenFilePicker`, `dialog.*`
`[D]` (grep = 0 совпадений); все `invoke` передают id/скаляры, например
`chat.js:432` `{ channelId: "news", content, nonce }`. Плагинов `tauri-plugin-fs`/`-dialog`
нет ни в `Cargo.toml`, ни в `Cargo.lock` `[D]`; capability — только `core:default`
`[D]` `capabilities/default.json:6`.

---

## 2. Инвариант (что должно быть записано до кода attachments/capture)

Формулировка, предлагаемая к фиксации в `ARCHITECTURE.md`/`TEST-POLICY` (PLAT-11 DoD):

1. **Путь выбирает backend.** В `#[tauri::command]` запрещены аргументы
   `PathBuf`/`Path`/`&Path`/`File`/`&[u8]`-«содержимое файла» и любые строки с именем
   `path|file|filepath|dir|folder|root|location`.
2. **Frontend передаёт токен/идентификатор, не путь.** Файл выбирает backend через
   системный диалог (`tauri-plugin-dialog`) и возвращает **непрозрачный handle**
   (`u64`-слот в реестре процесса), а не строку пути.
3. **Allowlist/root.** Любой backend-путь строится как `root.join(константа)`; `root` — из
   конфигурации процесса, не из IPC; вход может выбирать только **элемент allowlist**
   (снежинка/id/индекс), не сегмент пути.
4. **Canonicalize + containment.** Перед I/O — `std::fs::canonicalize`, затем
   `candidate.starts_with(canonical_root)`; иначе `InvalidArg` (не `Io`).
5. **`..` и разделители.** В id-строках запрещены `/`, `\`, `..`, `%2f`/`%2e`,
   `?`, `#`, NUL, CR/LF — независимо от того, идёт строка в ФС или в URL.
6. **Symlink escape.** Симлинк, ведущий за `root`, = отказ (п.4 ловит это
   канонизацией; для несуществующего файла — проверка родителя).
7. **Размер/тип.** Для будущего чтения файла: лимит байт, allowlist MIME по
   magic-байтам (не по расширению), потоковое чтение с жёстким потолком, отказ до аллокации.
8. **Никаких fd/pipewire node id из IPC.** Для capture backend сам создаёт поток
   (portal) и отдаёт frontend только состояние/индикатор; путь устройства, node id и fd
   через IPC не проходят `[A]` (формат handle портала не подтверждён, см. §5).

### 2.1 Соответствие сейчас

| Правило | Статус | Доказательство |
|---|---|---|
| 1 (нет path-аргументов) | **выполнено** | 29/29 сигнатур `[D]` §1; meta-теста нет |
| 2 (handle вместо пути) | не применимо (нет attachments) | `[D]` `STUB-MAP.md:34`, plugins отсутствуют |
| 3 (allowlist/root) | выполнено для данных | `DL_FIXTURES`/`DL_SETTINGS_PATH` + `known_channels` `[D]` |
| 4 (canonicalize/containment) | **не выполнено** | `canonicalize` в `rust/gui-spike/src` = 0 `[D]`; `root.is_dir()` только `data.rs:85-91` |
| 5 (`..` в id) | **выполнено частично** | 7 из 8 мест; `send_message` — нет (§3.2) |
| 6 (symlink escape) | **не выполнено** | нет канонизации ⇒ симлинк внутри `fixtures/` уводит чтение за root (§4 T3) |
| 7 (размер/тип) | не применимо | файлы не читаются по запросу |
| 8 (fd/node из IPC) | не применимо, зафиксировать заранее | capture-кода нет: в `discord-platform/src` нет portal/ScreenCast `[D]` |

---

## 3. Находки

### PLAT-11-A — нет meta-теста, удерживающего инвариант (P1-контроль)

Инвариант «IPC не принимает путь» сегодня держится **только дисциплиной ревью**.
Существующие wiring-тесты проверяют регистрацию команд и чтение токена
(`main.rs:599-700`), но не типы аргументов. Первый же воркер, добавляющий
`attachment_upload(path: String)`, не покраснеет ни на одном тесте.
Риск из PLAT-11 («чтение `~/.ssh` при XSS») реализуется именно так.
Фикс: T1 (§4), mutation-проверка «добавить `path: String` в команду → красный».

### PLAT-11-B — `send_message` подставляет `channelId` из webview в URL-путь (P2)

`messages.rs:147-153` проверяет только `is_empty` и `{}`/CRLF, затем строка уходит в
`routes::render_path` (`routes.rs:182-194`, запрещает лишь `{}`/пусто) и в
`ApiClient::post_once` → request-line. Транспорт (`transport.rs:217-224`) пропускает `..`,
`/`, `?`, `%`.

Воспроизведение (без сети, по коду): `channelId = "../../users/@me"` ⇒
target `/api/v9/channels/../../users/@me/messages` ⇒ после нормализации edge —
`/api/v9/users/@me/messages` с телом `{content, nonce, tts:false}`. Это
**forgery URL-пути** в пределах одного хоста и своего токена: не SSRF, не чтение файла, но
обход маршрутизации/allowlist и расхождение с остальными 7 командами (§1.2).
Эксплуатация требует XSS в webview (сейчас `innerHTML`=0), поэтому P2, не P0.

Замечание к фиксу: запрет «только снежинка» **сломает UI спайка** — `chat.js:432` шлёт
`channelId: "news"`, и текущие тесты это закрепляют (`messages_contract.rs:87-95`,
`"c1"`). Поэтому фикс — **allowlist символов** (`^[A-Za-z0-9_-]{1,64}$`), а не цифры;
перевод UI на снежинки — отдельная карточка паритета, не часть PLAT-11.

### PLAT-11-C — `csp: null` + `withGlobalTauri: true` усиливают любую будущую path-команду (P2, cross-ref PLAT-15)

`tauri.conf.json:20-22` `[D]`: CSP отключён, `window.__TAURI__` доступен глобально.
Любая HTML/SVG-инъекция сразу получает `invoke`. Пока нет path-команд и `innerHTML`=0 —
риск латентный; при появлении превью вложений (markdown/image preview) он становится
прямым. Триггер повышения: первый рендер чужого HTML/SVG или превью вложения.

### PLAT-11-D — UI-контракт id неоднороден (P3, контекст фикса)

`reactions.js:108-109` шлёт `channelId: "news"` в `toggle_reaction`, который требует
`is_numeric_id` (`reactions.rs:166-170`) ⇒ в live-режиме реакция вернёт `InvalidArg`.
Это не PLAT-11, но показывает: «снежинка везде» — не текущее состояние, и ужесточение
id-валидации должно быть отдельным решением с миграцией фикстур/UI.

### PLAT-11-E — attachments/capture: проектный гейт до кода (P1)

`STUB-13` — `aria-disabled`, upload **blocked** (`STUB-MAP.md:34`, `ATTACH-RE.md:23-36`);
в `NETWORK-CONTRACTS.md` нет ни строки про attachments `[D]` (grep «attach» = 0);
capture-кода нет — в `discord-platform/src` только dbus/keyring/power/autostart/
notifications/spellcheck `[D]`, portal/ScreenCast отсутствует `[D]` (grep = 0).
Любая реализация «по аналогии» (upload с путём из webview, `screen_capture(node_id: String)`)
нарушит §2 до появления первой строки кода. Гейт: дизайн attachments/capture проходит
ревью против §2 **до** ТЗ воркеру.

---

## 4. Три теста (red-first, без live)

Все три — без окна, без сети, без golden-путей. T1/T2 — Rust integration в
`rust/gui-spike/tests/`; T3 — `#[cfg(unix)]` temp-каталог (как PLAT-14).

### T1 — meta: ни одна IPC-команда не принимает путь (P1)

Оракул — исходник бинаря (тот же приём, что `main.rs:599-700` и
`guilds_contract.rs:331-349`), независимый от текущей реализации команд.

```rust
// tests/ipc_path_contract.rs
const FORBIDDEN_TYPES: [&str; 4] = ["PathBuf", "Path", "&Path", "File"];
const FORBIDDEN_NAMES: [&str; 7] =
    ["path", "file", "filepath", "dir", "folder", "root", "location"];

#[test]
fn no_ipc_command_accepts_a_filesystem_path() {
    let source = include_str!("../src/main.rs");
    for signature in command_signatures(source) {          // между #[tauri::command] и "->"
        for (name, ty) in signature.args {
            assert!(!FORBIDDEN_TYPES.iter().any(|t| ty.contains(t)),
                    "{name}: путь запрещён в IPC-команде");
            assert!(!FORBIDDEN_NAMES.contains(&name.to_ascii_lowercase().as_str()),
                    "{name}: имя аргумента похоже на путь");
        }
    }
}

#[test]
fn filesystem_reads_use_only_literal_or_env_paths() {
    let source = include_str!("../src/main.rs");
    // /proc/self/status — литерал; иных std::fs-вызовов в бинаре нет
    assert_eq!(source.matches("std::fs::").count(), 1);
}
```

**Red сейчас:** тест зелёный (это желаемое состояние) ⇒ корректный red-first — через
**мутацию**: добавить `fn probe(path: String)` с `#[tauri::command]` → T1 краснеет.
Мутация фиксируется в отчёте, из дерева удаляется (или прогоняется
`tests/meta`-скриптом на копии). Без mutation-прогона тест считается недоказанным
(`TEST-POLICY` §9).

### T2 — `send_message` отвергает path-significant `channelId` до I/O (P2)

```rust
#[test]
fn send_message_rejects_path_traversal_in_channel_id_before_sender() {
    for id in ["../../users/@me", "..%2f..%2fusers/@me", "/etc/passwd",
               "10/../../users/@me", "10?x=1", "10\\..\\x", "10\x00", "10\n"] {
        let sender = fake_ok(receipt());
        assert_eq!(run(send_message(&sender, id, "hi", "n1")),
                   Err(MessageError::InvalidArg), "id={id:?}");
        assert!(sender.calls.lock().unwrap().is_empty(), "id={id:?}: I/O произошёл");
    }
}
```

**Red сейчас (доказано кодом):** fake-sender получает строку как есть и возвращает
`Ok(receipt)` ⇒ `assert_eq!` падает на первом элементе. Зелёный — allowlist
`^[A-Za-z0-9_-]{1,64}$` в `messages.rs::send` (сохраняет `"news"`/`"c1"` UI-фикстур).

### T3 — symlink внутри `fixtures/` не уводит чтение за root (P2)

```rust
#[cfg(unix)]
#[test]
fn fixtures_root_rejects_symlink_escape() {
    let outside = temp_dir("plat11-outside");          // валидный 002-READY.json
    let root = temp_dir("plat11-root");
    std::os::unix::fs::symlink(&outside, root.join("gateway")).unwrap();
    let error = data::ready(&root).expect_err("симлинк за root должен быть отвергнут");
    assert!(matches!(error.kind(), "InvalidArg" | "NotFound"), "{error:?}");
    assert!(!format!("{error:?}").contains(outside.to_str().unwrap()));
}
```

**Red сейчас:** `data.rs:84-99` проверяет только `root.is_dir()`; `read_dir` следует
симлинку ⇒ `ready(&root)` вернёт `Ok` с данными из `outside` ⇒ `.expect_err` падает.
Зелёный — `canonicalize` + `starts_with(canonical_root)` в `read_frames`/`fixtures_root`.
Skip-условие: не-Unix платформа (причина печатается, не молча — PLAT-02).

---

## 5. Что нельзя проверить без live

| Область | Почему | Какой минимум нужен |
|---|---|---|
| Upload вложений | endpoint/multipart/лимиты — **gap** (`ATTACH-RE.md:25-28`); `NETWORK-CONTRACTS.md` не содержит attachments `[D]` | `ATTACH-RE-live` (согласованный захват upload); до него — только контракт §2 и T1/T2 |
| Portal `ScreenCast`/`Camera`: тип handle (fd? node id?) | в системе есть `libpipewire`, но кода нет; формат `OpenPipeWireRemote` не подтверждён `[A]` | mock-шина `discord-platform::dbus` (как `a04/a05`) + затем окно согласия |
| «XSS → invoke» целиком | нужен реальный webview с инъекцией | live-окно с тестовой страницей; частично моделируется `window.__TAURI__` на fake |
| Индикатор захвата = факт потока | нет потока | live capture (PLAT-10) |
| Реальные лимиты/типы файлов Discord | нет в дампе `[A]` | `ATTACH-RE-live` |

Ничего из этого не блокирует T1–T3: они проверяют **наш** контракт, а не сервер.

---

## 6. Приоритет, owner, DoD, связь с attachments/capture

| ID | P | Owner | Почему такой порядок |
|---|---|---|---|
| PLAT-11-A (meta-тест T1) | **P1** | G1 (код теста) + G4 (гейт) | дёшево, закрывает контроль-гэп; без него следующие карточки снова «на дисциплине» |
| PLAT-11-E (проектный гейт) | **P1** | PM + G1+G2 | до ТЗ на attachments/capture; иначе код появится раньше контракта |
| PLAT-11-B (T2) | P2 | G1 | 8 строк allowlist + тест; требует решения по `"news"` |
| PLAT-11-C (CSP) | P2 | G4 | вместе с PLAT-15 (CSP ≠ null, meta на capability) |
| T3 (symlink/canonicalize) | P2 | G1 | нужно до первого «файла по запросу»; сейчас данных из IPC нет |
| PLAT-11-D (id-контракт UI) | P3 | G1+G2 | отдельная карточка паритета, не смешивать с PLAT-11 |

**Расхождение владельцев `[D]`:** `QUEUE.md:46` — owner **G4b**; `PLATFORM-AUDIT.md:250` —
**G2+G1**. Решение за PM; ниже DoD сформулирован так, что код — G1, приёмка — G4b.

### DoD (закрытие PLAT-11)

1. §2 внесён в `ARCHITECTURE.md` (или `SECURITY`-раздел) как нумерованный инвариант —
   без «по аналогии».
2. T1 существует и **доказан мутацией** (мутация в логе, дерево чистое).
3. T2 зелёный; в `messages.rs::send` allowlist символов; `"news"`/`"c1"` по-прежнему
   проходят (тест на регресс UI-контракта).
4. T3 зелёный **или** явный waiver PM с причиной «файлов по запросу нет» и датой
   пересмотра (waiver письменный, `TEST-POLICY`).
5. `cargo test -p discord-gui-spike` (targeted) + `npm run gate:worker` — логи в
   `waves/<ID>/`.
6. В `STUB-MAP.md` STUB-13/STUB-27 добавлена ссылка: «IPC-путь запрещён — PLAT-11 §2».
7. Повторный grep-оракул (ниже) в отчёте приёмки: 0 path-аргументов.

### Связь с attachments/capture

- **Attachments (`STUB-13`, blocked):** до `ATTACH-RE-live` нельзя писать ни upload, ни
  «превью файла». Когда upload разблокируют — путь выбирает backend через
  `tauri-plugin-dialog`, IPC получает `AttachmentHandle(u64)`, backend хранит
  `HashMap<u64, PathBuf>` и перед чтением делает canonicalize+containment (§2 п.2-4, 7).
- **Capture (`STUB-27`, `PLATFORM-AUDIT` §4.1-4.2):** portal открывает поток; IPC отдаёт
  только состояние/индикатор. `node_id`/fd/путь устройства в IPC не попадают (§2 п.8).
  Это фиксируется до прототипа, иначе первый же `start_capture(path: String)` нарушит
  инвариант.

---

## 7. Evidence (воспроизводимо, read-only)

```bash
# E1 — 29 команд, ни одного path-аргумента (парсер сигнатур)
python3 - <<'PY'
import re
src = open('rust/gui-spike/src/main.rs').read()
# делим только по строке-атрибуту, иначе doc-comment в wiring-тестах даёт ложный блок
blocks = re.split(r'(?m)^#\[tauri::command\]$', src)[1:]
print('command attributes:', len(blocks))
print('path-like in signature:',
      sum(('PathBuf' in b.split('{')[0])
          or bool(re.search(r':\s*&?Path\b', b.split('{')[0])) for b in blocks))
PY
# -> command attributes: 29 / path-like in signature: 0

# E2 — ФС-чтения только литералами/env-корнем
grep -rn "std::fs::\|read_to_string\|read_dir" rust/gui-spike/src/*.rs
# -> main.rs:50 (/proc/self/status), auth.rs:417 (/etc/timezone),
#    data.rs:264,278, settings_store.rs:30,470

# E3 — канонизации нет
grep -rn "canonicalize" rust/gui-spike/src/ ; echo "exit=$?"
# -> exit=1 (0 совпадений)

# E4 — нарушитель: send_message валидирует только пусто/{}/CRLF
sed -n '147,159p' rust/gui-spike/src/messages.rs

# E5 — transport пропускает ".."/"/"/"?"
sed -n '217,224p' rust/crates/discord-core/src/rest/transport.rs

# E6 — attachments/capture отсутствуют в контрактах и коде
grep -c "attach" waves/RW-40/NETWORK-CONTRACTS.md          # -> 0
grep -rn "ScreenCast\|OpenPipeWireRemote" rust/crates/discord-platform/src/ ; echo "exit=$?"  # -> 1

# E7 — плагинов fs/dialog нет; capability только core:default
grep -rn "tauri-plugin" rust/gui-spike/Cargo.toml rust/gui-spike/Cargo.lock | wc -l   # -> 0
cat rust/gui-spike/capabilities/default.json
```

Наблюдаемые выходы соответствуют `[D]`-ссылкам выше (все команды выполнены в этой сессии,
сеть/окно не использовались).

### md5 аудированных артефактов (на момент аудита)

| Файл | md5 |
|---|---|
| `rust/gui-spike/src/main.rs` | `6ea7941df82e0b4bab7a2fa6b7a2e26d` |
| `rust/gui-spike/src/messages.rs` | `d0556da9f3203879831a29d9dd6f7a6a` |
| `rust/gui-spike/src/data.rs` | `6f674651419cfdecf92b143d606d2678` |
| `rust/gui-spike/src/settings_store.rs` | `561a4fa17953e6b0c3b508cc5eb7357b` |
| `waves/RW-40/QUALITY/PLAT-11-AUDIT.md` | вычисляется от финальной версии — в отчёте субагента |

Файлы кода не изменялись: суммы — это ровно те байты, которые читались при аудите.
