# Отчёт: V-07-UI-VOICE-DEVICES — контроллер «Голос и видео» (G2 Frontend)

- **Карточка / batch:** `RW-40` / `V-07-UI-VOICE-DEVICES`, отдел **G2 Frontend**.
- **HEAD:** `4e24048` (рабочее дерево до старта уже содержало чужие правки — они не трогались).
- **Контракт-оракул:** `rust/crates/discord-voice/src/devices.rs`, `waves/RW-40/DESIGN/VOICE.md`
  §2.4, §3.1–§3.5, `waves/RW-40/DESIGN/voice-spec.json`.
- **Границы файлов:** созданы только `rust/gui-spike/ui/features/voice-devices-settings.js`,
  `tests/unit/voice-devices-settings.test.mjs` и артефакты в `waves/RW-40/PM/v07-ui-devices/`.
  `app.js`, `index.html`, CSS, общие файлы и Rust-крейты не изменялись; `cargo` не запускался.

## 1. Что реализовано

`createVoiceDevicesSettings({ host, devices, service, onDeviceEvent })` — изолированный
контроллер без сети, IPC и медиа-API:

1. **Устройства ввода/вывода.** Два селекта с `aria-label` из `copy` (`hHMYbb`, `dl18zb`).
   Первичная привязка — `isDefault === true`, иначе первое устройство. Явный выбор идёт
   в `service.selectInput/selectOutput`; чужой `kind` и неизвестный id отклоняются без вызова
   сервиса, отказ показывается в `role="alert"`. Длинное имя не обрезается в DOM и доступно
   через `title` (VOICE.md §3.1).
2. **Hot-plug.** `setDevices()` повторяет `VoiceDevicesState::update_device_list`:
   присутствующее устройство сохраняется без событий; исчезнувшее откатывается на
   `isDefault === true` (не на первый) с `DeviceFallback { kind, previousId, fallbackId }`;
   пустой список даёт `NoDeviceAvailable { kind, previousId }`, `selected = null` и
   `aria-disabled` на селекте. Событие уходит в `onDeviceEvent` и в live-регион, сервис
   `select*` при hot-plug **не** вызывается (VOICE.md §3.2).
3. **Громкость.** Микрофон `0..=100` (`OX2Bnr`), вывод `0..=200` (`aUJ062`). Кламп считается
   до вызова сервиса, ползунок и `aria-valuenow/valuetext` показывают итог.
4. **Режим активации.** Тумблер `Режим рации` (`tG4Np5`) переключает
   `VoiceActivity { thresholdDb }` ↔ `PushToTalk { releaseDelayMs }`; `thresholdDb` клампится
   по `i8` (−128..127), `releaseDelayMs` — в 0..2000 мс (карточка V-07). Скрытые группы
   VAD/PTT переключаются атрибутом `hidden`; значения переживают переключение режима.
5. **Захват клавиши PTT.** Кнопка `o+BJQR`; клик включает захват, `Escape` отменяет,
   модификаторы игнорируются, первая обычная клавиша уходит в `service.setPttKey`.
   Пустое состояние — `nWRdnl` («Горячие клавиши не назначены»).
6. **Mute/deafen.** `effectiveMute = userMute || selfDeaf`; `data-effective-mute` на корне
   и на live-узле, `role="switch"` + `aria-checked` у обоих тумблеров. Deafen **не** снимает
   `aria-checked` микрофона (VOICE.md §2.4 п.3); после снятия deafen возвращается явный
   `userMute`.
7. **Ошибки сервиса** не глотаются: синхронный throw и отклонённый промис дают `role="alert"`.
8. **Приватность.** В модуле нет `console.*`, `fetch`, `WebSocket`, `navigator.mediaDevices`
   и API записи/превью; имена устройств живут только в DOM.

## 2. RED-прогон

```bash
node --test tests/unit/voice-devices-settings.test.mjs > waves/RW-40/PM/v07-ui-devices/red.log 2>&1
```

- **Exit code:** `1`.
- **Первая ошибка:** `Error [ERR_MODULE_NOT_FOUND]: Cannot find module '.../features/voice-devices-settings.js'`
  (импорт отсутствующего продуктового модуля).
- **Артефакт:** `waves/RW-40/PM/v07-ui-devices/red.log` (md5 `cf24c664db69970df740ae2582993b18`).

## 3. GREEN и targeted-проверки

| Команда | Exit | Результат | Артефакт |
|---|---:|---|---|
| `node --test tests/unit/voice-devices-settings.test.mjs` | `0` | **17 passed; 0 failed; 0 skipped** | `green-final.log` |
| `node --test tests/unit/voice-devices-settings.test.mjs tests/unit/voice-ui.test.mjs tests/meta/test-policy.test.mjs` | `0` | **78 passed; 0 failed; 0 skipped** (17 своих + смежный V-08 + мета-политика) | `targeted.log` |
| `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict ... voice-devices-settings.js` | `0` | ошибок нет | `tsc.log` |
| `npx tsc -p tsconfig.json` | `0` | ошибок нет (тест и импортированный модуль входят в программу) | `tsc.log` |

История green: первый прогон после реализации дал `16/17` — падение было в **фикстуре теста**
(в сценарии fallback не было выходного устройства, и контроллер корректно эмитил второй
`NoDeviceAvailable`). Фикстура дополнена выходным устройством до финального прогона;
продуктовый код не менялся. `green.log` — зелёный прогон того же набора до правки только
JSDoc-аннотаций теста (`bf7544e`); `green-final.log` — прогон финального файла. Оба 17/17.

## 4. Мутации (`mutations.log`)

Baseline MD5 продуктового файла: `5fda00c8c063f9726fc1ab31ac13ef99`.
Все 5 мутантов убиты, после каждого файл восстановлен и сверен по MD5.

| ID | Мутация | MD5 мутанта | Exit | Убившие тесты | Вердикт |
|---|---|---|---:|---|---|
| **MUT-1** | `effectiveMute` игнорирует `selfDeaf` | `3d21de0f742f070753cc3ea33750b941` | `1` | mute/deafen-инвариант | **KILLED** |
| **MUT-2** | откат на первый элемент вместо `isDefault` | `c43f6ee5aaa9123cae12a439f8a46610` | `1` | начальное состояние, выбор, hot-plug (4 теста) | **KILLED** |
| **MUT-3** | вывод клампится по `MAX_INPUT_VOLUME` (100) | `150486a98b4c5c8c838e7a29db3ea5b7` | `1` | громкость вывода `aUJ062` | **KILLED** |
| **MUT-4** | hot-plug не уведомляет колбэк/DOM | `aac941b79bce6efee8f890820ebd0db1` | `1` | `DeviceFallback`, `NoDeviceAvailable` | **KILLED** |
| **MUT-5** | `aria-checked` микрофона из `effectiveMute` | `6e4308f958983edceb55d4ae7a6cdb71` | `1` | mute/deafen + ARIA | **KILLED** |
| Restore | контрольный прогон | `5fda00c8c063f9726fc1ab31ac13ef99` | `0` | 17 passed | **GREEN** |

## 5. Файлы и MD5

| Файл | Статус | MD5 |
|---|---|---|
| `rust/gui-spike/ui/features/voice-devices-settings.js` | created | `5fda00c8c063f9726fc1ab31ac13ef99` |
| `tests/unit/voice-devices-settings.test.mjs` | created | `bf7544e0c25c18ca0e8eaba579bca7bc` |
| `waves/RW-40/PM/v07-ui-devices/red.log` | created | `cf24c664db69970df740ae2582993b18` |
| `waves/RW-40/PM/v07-ui-devices/green.log` | created | `1d32d8d289747b8ad5eef9b7e4595126` |
| `waves/RW-40/PM/v07-ui-devices/green-final.log` | created | `d3e1569e531207cd8609064542d8c175` |
| `waves/RW-40/PM/v07-ui-devices/tsc.log` | created | `55aae25e7f250aa4a49a2d0b239510e6` |
| `waves/RW-40/PM/v07-ui-devices/mutations.log` | created | `8a52c1ba0c5fb92075262f9f253b9227` |
| `waves/RW-40/PM/v07-ui-devices/targeted.log` | created | `a2515f808a9c8540908f8ceb549ae6b1` |

## 6. Риски и открытые границы

1. **Полный `npm run gate:worker` не запускался.** По правилу batch (`AGENTS.md` §3) воркер
   гоняет targeted-проверки, полный гейт — один раз в integration-лейне; рабочее дерево кроме
   этой карточки содержит незакоммиченные чужие правки. Для приёмки нужен batch-gate.
2. **Задержка PTT по умолчанию 20 мс — `[A]`.** В дампе значения нет (`VG-02`); выбранное
   число не заявляется как паритет. То же для подписи порога VAD.
3. **Диапазон `thresholdDb` — `i8` контракта Rust (−128..127).** В Discord ползунок
   чувствительности 0..100 и авто-режим (`lY6j47`); это другая модель, паритет не заявлен.
4. **Захват клавиши слушает саму кнопку, а не документ.** Так контроллер остаётся
   изолированным и тестируемым; глобальный keybind, состояние `blocked` и OS-ограничения —
   вне карточки (VOICE.md §3.3).
5. **Нет CSS.** Правка стилей запрещена границами файлов; классы (`voice-devices-*`) готовы
   для отдельной карточки оформления.
6. **Сериализация Rust → JS.** Контроллер принимает `isDefault` (как в задании). Если IPC
   отдаёт snake_case `is_default`, адаптер должен жить в слое IPC, а не в UI.
7. **`app.js` не подключает контроллер** — это изолированная карточка; интеграция в экран
   настроек и `service`-адаптер к `discord-voice` остаются за интеграционным batch.
