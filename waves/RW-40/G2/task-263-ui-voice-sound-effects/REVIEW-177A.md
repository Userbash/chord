# Technical Review 177A: PR-148 (`task-263` / `FE-P1-34` — Voice Session Sound Cues & Notification Audio Mute Helper)

- **Branch:** `feature/task-263-ui-voice-sound-effects` (HEAD `94b91b1`, без коммита)
- **Reviewer:** `opencode` (`deepseek/deepseek-v4.1-flash`, `TOKEN-LEAN`)
- **Scope:** `rust/gui-spike/ui/features/voice-sound-effects.js`, `tests/unit/voice-sound-effects.test.mjs`, `REPORT.md`
- **Verdict:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты

## 1. Baseline

| Проверка | Результат |
| --- | --- |
| `node --test tests/unit/voice-sound-effects.test.mjs` | `tests 12, pass 12, fail 0`, exit `0` |
| md5 `voice-sound-effects.js` | `99026507e8cdb8a17e2b345ebed814a4` — совпадает с `REPORT.md` |
| md5 `voice-sound-effects.test.mjs` | `1799f8674142d207e6ca134bacb10cec` — совпадает с `REPORT.md` |

## 2. Код-ревью

Контракт читается однозначно: `isKnownCue` закрывает нестроковый и неизвестный вход через
`hasOwnProperty` (не наследует `constructor`/`toString`), фильтры применяются в задокументированном
порядке `disableAllSounds → mutedEvents → selfDeaf → streamerMode`, `normalizeCueVolume` отсекает
`NaN`/нечисло/`Infinity` до клампа и возвращает шаг `0.01` в `[0..2]`. Тесты проверяют поведение по
независимым ожидаемым значениям из ТЗ, а не текущую реализацию; заморозка справочника проверяется
рантайм-проверкой (`isFrozen`, `isExtensible`, `TypeError` на записи).

Замечаний уровней `P0`/`P1` нет. Наблюдения без блокировки:

- `P2`: `mutedEvents?.includes(eventKey)` рассчитан на массив строк; нестроковые элементы в массиве
  просто не совпадут — тип `ReadonlyArray<string>` это покрывает, миграция значений уже отмечена
  в `REPORT.md`.
- `P2`: модуль пока не импортируется — это осознанная граница карточки (интеграция Web Audio и
  голосовых событий отдельной задачей), на качество контракта не влияет.
- `P3`: отклонение `Infinity → 1` от буквы ТЗ задокументировано и закреплено тестом — принимаю:
  битый вход не должен давать максимальную громкость.

## 3. Мутации (`5/5` убиты)

Каждая мутация вносилась в `voice-sound-effects.js` по одной, затем прогонялся targeted-тест,
после чего файл побайтово восстанавливался из бэкапа с проверкой `cmp` и `md5sum`.

| # | Мутация | md5 мутанта | exit | pass/fail | Убивший тест |
| --- | --- | --- | --- | --- | --- |
| M1 | `if (!isKnownCue(eventKey)) return false;` → `if (false) return false;` (неизвестный ключ играет) | `628d37976d8393f814a7b675f1f04bbe` | `1` | 11 / 1 | `shouldPlayVoiceCue: неизвестный ключ события не играет` |
| M2 | Убраны исключения `selfDeaf`: `&& eventKey !== "undeafen" && eventKey !== "deafen"` | `b19c11678b1a8858c7166c7daa3d4d18` | `1` | 10 / 2 | `selfDeaf оставляет слышимыми только deafen/undeafen`; `комбинация фильтров и приоритет selfDeaf над streamerMode` |
| M3 | `streamerMode`: `\|\|` → `&&` в условии `user_join`/`user_leave` | `1966dd307c00a081fb8a4412891d063f` | `1` | 11 / 1 | `streamerMode глушит только join/leave` |
| M4 | Убран `!Number.isFinite(volumePercent)` из guard (Infinity → `2` вместо `1`) | `aac724348779c0129ee40a779ff6be6f` | `1` | 11 / 1 | `normalizeCueVolume: нечисловой вход даёт 1` |
| M5 | Убрано округление: `Math.round(clamped) / 100` → `clamped / 100` | `bb844213f9f90c81cfdd46299fa9aacd` | `1` | 11 / 1 | `normalizeCueVolume: выход за диапазон обрезается, дробные округляются` |

Прозрачность: первая попытка M3 содержала ошибку экранирования в скрипте мутации (`\&\&`), из-за
чего тест падал на синтаксисе модуля (`pass 0, fail 1`), а не на ассерте. Попытка признана
недействительной и M3 переснята в корректном виде (проверено `diff`, `pass 11, fail 1`); в таблице
указан результат повторного, валидного прогона.

## 4. Финальное состояние

```
99026507e8cdb8a17e2b345ebed814a4  rust/gui-spike/ui/features/voice-sound-effects.js
1799f8674142d207e6ca134bacb10cec  tests/unit/voice-sound-effects.test.mjs
```

После последней мутации файл восстановлен побайтово (`cmp` + `md5sum` = исходный), финальный
`node --test tests/unit/voice-sound-effects.test.mjs` → `tests 12, pass 12, fail 0`, exit `0`.
Тестовый файл не изменялся, полный `gate:worker` не запускался, `git commit` не выполнялся.

В рабочем дереве остаются только ранее существовавшие изменения, к PR-148 не относящиеся:
`D node_modules`, `M package-lock.json`. Оба файла не трогались в ходе ревью.

## Итог

**APPROVED.** `P0 = 0`, `P1 = 0`, `5/5` мутаций убиты, md5 совпадает с `REPORT.md`. PR-148 готов к
`Ready for Merge`; полный `gate:worker` выполняется в integration-лейне по регламенту `T1`.
