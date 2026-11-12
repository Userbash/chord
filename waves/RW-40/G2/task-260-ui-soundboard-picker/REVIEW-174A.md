# REVIEW-174A — task-260 / PR-145 (FE-P0-17, Voice Soundboard Picker & Custom Sound Upload)

- **Ревьюер**: Технический ревьюер (DeepSeek, token-lean)
- **Карточка / ветка**: `task-260` (`feature/task-260-ui-soundboard-picker`)
- **Объект ревью**: `rust/gui-spike/ui/features/soundboard-picker.js` (544 строки), `tests/unit/soundboard-picker.test.mjs` (314 строк)
- **Входной отчёт**: `waves/RW-40/G2/task-260-ui-soundboard-picker/REPORT.md`
- **Дата**: 2026-10-31

## Вердикт: APPROVED

`P0 = 0`, `P1 = 0`, мутации `5/5` убиты (+2 дополнительные, `7/7`). Файл после каждой мутации восстановлен побайтово, финальный md5 совпадает с исходным и с `REPORT.md`.

## 1. Прогон и сверка артефактов

| Проверка | Результат |
|---|---|
| `node --test tests/unit/soundboard-picker.test.mjs` | `exit=0`, `tests 10`, `suites 5`, `pass 10`, `fail 0`, `duration ~46ms` |
| md5 `rust/gui-spike/ui/features/soundboard-picker.js` | `6e41082ccdde7d3a6b5c0227114c1271` — совпадает с `REPORT.md` (строка 38) |
| md5 `tests/unit/soundboard-picker.test.mjs` | `5fb863f07ecbf1bc7aad453db4e0fee4` — совпадает с `REPORT.md` (строка 39), файл не изменялся |
| `git status` по целевым файлам | оба `??` (untracked), коммитов нет, посторонних правок нет |

Оракульная сверка: маршруты из `REPORT.md` подтверждены в RE-дампе —
`reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`:
`GUILD_SOUNDBOARD_SOUNDS: e => "/guilds/${e}/soundboard-sounds"`,
`SEND_SOUNDBOARD_SOUND: e => "/channels/${e}/send-soundboard-sound"`.
Лимиты `2..=32` символа, `524288` байт и громкость `0..1` зафиксированы тестом T-260-01 и соответствуют карточке.

## 2. Мутации

Методика: одиночная точечная правка → `node --test tests/unit/soundboard-picker.test.mjs` → `cp -p` из бэкапа → `md5sum`. Логи: `mutations/M*.log`. Тестовый файл не правился.

| ID | Мутация | md5 мутанта | exit | Убившие тесты | Восстановление |
|---|---|---|---|---|---|
| M1 | `isValidSoundboardName`: `trimmed.length <= MAX` → `< MAX` (off-by-one верхней границы) | `6cb7ec1b1919a9d4a6b39d1e65137c39` | 1 | T-260-02 | OK, md5 исходный |
| M2 | `clampSoundboardVolume`: нечисловое/`NaN` → `return 0` вместо `return 1` | `df93210f3f0b3a05b5c553583c0f3478` | 1 | T-260-03 | OK, md5 исходный |
| M3 | `filterSoundboardSounds`: удалён `if (item.available === false) continue` | `47cd83a35b46546e18109273bb87490f` | 1 | T-260-04, T-260-05, T-260-06, T-260-08 | OK, md5 исходный |
| M4 | `handleUploadSound`: удалена проверка префикса `data:audio/` (`!startsWith(...)` → `false`) | `6e9264442c318689c2193afbbf9330de` | 1 | T-260-09 | OK, md5 исходный |
| M5 | `getState`: `sounds.map(cloneSoundboardSound)` → живой `sounds` | `14e75f0064ea9c362366c4c3257b9021` | 1 | T-260-10 | OK, md5 исходный |
| M6 (доп.) | `renderSoundList`: в `onPlaySound` передаётся `1` вместо `currentVolume` | `e78c3d15e923c81d0a6eb8c30d4d06ea` | 1 | T-260-07 | OK, md5 исходный |
| M7 (доп.) | обработчик `input` поиска: `currentQuery = ""` вместо `String(rawQuery)` | `4b227db9ddfc770cbe15b1624fc462d1` | 1 | T-260-08 | OK, md5 исходный |

Итог: 5 обязательных мутаций убиты, 2 дополнительные — тоже. Покрыты мутациями все 10 тестов, кроме T-260-01 (константы проверяются прямым сравнением значений, отдельной логики нет).

Финальное состояние:
```
6e41082ccdde7d3a6b5c0227114c1271  rust/gui-spike/ui/features/soundboard-picker.js
5fb863f07ecbf1bc7aad453db4e0fee4  tests/unit/soundboard-picker.test.mjs
```
`FINAL=BYTE_IDENTICAL` (сверка с бэкапом `/tmp/opencode/soundboard-picker.orig.js`).

## 3. Замечания

### P0 — 0
Нет.

### P1 — 0
Нет.

### P2 — 3 (не блокируют merge)

1. **Лимит размера считается по длине строки Data URI, а не по байтам файла** (`soundboard-picker.js:444-451`).
   `trimmedUri.length > MAX_SOUNDBOARD_BYTES` сравнивает длину base64-строки с лимитом байт аудио. Base64 раздувает payload в ~4/3, поэтому валидные файлы примерно `393..512 КБ` отклоняются с текстом «размером до 512 КБ». Верхняя граница при этом не нарушается (проверка консервативная: файл > 512 КБ пройти не может), но пользователь получает ложный отказ и неверное сообщение. Дополнительно: URI `data:audio/` без payload проходит валидацию.
   Проверка: `uriInput.value = "data:audio/mp3;base64," + "A".repeat(400000)` → ожидаемо должно уходить, фактически `[data-soundboard-error]` непустой, `onUploadSound` не вызван.
   Рекомендация: считать размер payload (`Math.floor(payload.length * 3 / 4)`) либо принимать `File`/`Uint8Array` и мерить `byteLength`; пустой payload отклонять. Потребует правки T-260-09 (тест сейчас фиксирует текущую семантику).

2. **Монтирование и `destroy()` очищают весь контейнер, а не только свой корень** (`soundboard-picker.js:531-537`).
   В конструкторе `replaceElementChildren(container, [root])`, в `destroy()` else-ветка — `replaceElementChildren(container, [])`. Для выделенного overlay-контейнера это корректно; если контейнер общий (shell/панель), соседние узлы будут уничтожены. Рекомендация: удалять только `root` (`container.removeChild(root)` / `root.remove()`), а не переписывать children контейнера.

3. **A11y модального диалога неполная** (`soundboard-picker.js:302-305`).
   Есть `role="dialog"` и `aria-label="Звуковая панель"` (контракт T-260-06 выполнен), но нет `aria-modal="true"`, обработки `Escape` и управления фокусом. В `ui/features/actions.js` и `context-menu.js` такой паттерн есть; в смерженном `automod-editor.js` — нет, то есть единого стандарта в репозитории пока не сложилось. Для модальной панели звуков стоит закрыть отдельной задачей (Escape → `destroy()`, `aria-modal`, возврат фокуса на триггер).

### P3 — 3 (наблюдения)

- `createSoundboardPickerController(options)` без `@returns` в JSDoc; тип выводится, `tsc` чистый, но для консистентности с остальными экспортами стоит указать `@returns {{ setState: Function, getState: Function, destroy: Function }}`.
- `replaceElementChildren`: fallback `Array.isArray(parentEl.children)` не сработает для реального `HTMLCollection`; безопасно, потому что в DOM есть `replaceChildren`, но fallback фактически мёртв.
- Тексты ошибок заданы литералами в модуле (`INVALID_NAME_ERROR_RU`, `INVALID_AUDIO_URI_ERROR_RU`). Если для UI-строк проекта планируется i18n-каталог, их нужно будет вынести; сейчас совпадает со стилем соседних `ui/features/*`.

## 4. Что проверял по существу

- **Безопасность**: в файле нет `innerHTML` / `insertAdjacentHTML` / `outerHTML` / `eval` / `new Function` — весь пользовательский текст (`name`, `emojiName`, `soundId`) выводится через `textContent` и `setAttribute`, XSS-вектора нет.
- **Иммутабельность**: `filterSoundboardSounds` и `normalizeSoundboardSounds` клонируют элементы, `getState()` клонирует `sounds`; мутация снимка не протекает внутрь (подтверждено T-260-04/T-260-10 и мутацией M5).
- **Границы**: имя `2` и `32` символа после `trim()` проходят, `1` и `33` — нет; громкость `-100 → 0`, `200 → 1`, `NaN`/`"0.5"`/`null`/`undefined → 1`; `-0 → 0` (обрабатывается явной веткой `volume === 0 ? 0 : volume`, тест на `Object.is` проходит).
- **Сортировка**: `localeCompare(..., "ru")` по возрастанию + tie-break по `String(soundId)`; недоступные (`available === false`) отбрасываются и в списке, и в рендере.
- **События**: `click` на кнопке звука передаёт актуальную `currentVolume` (мутация M6 доказывает, что тест держит эту связку); `input`/`change` на ползунке обновляют состояние; `submit` формы вызывает `preventDefault`.
- **Гейт**: `gate:worker` не запускался (режим ревью, ТЗ прямо запрещает), `git commit` не выполнялся, тестовый файл не изменялся.

## 5. Итог

PR-145 проходит техническое ревью: контракт T-260-01..T-260-10 зелёный, регрессионная защита подтверждена 7 убитыми мутациями, блокирующих замечаний нет. Три P2 стоит забрать отдельными карточками (размер файла в байтах, безопасный teardown контейнера, a11y модальности) — ни одна не мешает слиянию в `unstable`.
