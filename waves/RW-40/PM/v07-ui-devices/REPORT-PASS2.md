# V-07-UI-VOICE-DEVICES-PASS2 — отчёт по P1 из RW-40 REVIEW

**Карточка:** V-07-UI-VOICE-DEVICES-PASS2
**Вход:** `waves/RW-40/PM/v07-ui-devices/REVIEW.md`, P1 — `red.log` показывал
`ERR_MODULE_NOT_FOUND`, а не поведенческое падение.
**Что сделано:** red-first прогон на заглушке вместо отсутствующего файла. `cargo` не
запускался, продуктовый модуль и тест не менялись.

## 1. Резервная копия

Baseline скопирован побайтово в `/tmp/opencode/voice-devices-settings-pass2.baseline.js`
(28452 байта). MD5 копии и продуктового файла до подмены — `5fda00c8c063f9726fc1ab31ac13ef99`.

`git checkout` не годится как способ восстановления: оба файла карточки не отслеживаются
(`git status` → `??`), поэтому копия в `/tmp` — единственный источник точного восстановления.

## 2. RED: заглушка вместо модуля

`rust/gui-spike/ui/features/voice-devices-settings.js` временно заменён минимальной
заглушкой: экспортирует `createVoiceDevicesSettings`, очищает host через
`host.replaceChildren()` и возвращает no-op методы. Выбора default-устройства, клампинга
громкостей, hot-plug и разметки контролов в заглушке нет.

Команда и результат:

```
node --test tests/unit/voice-devices-settings.test.mjs > waves/RW-40/PM/v07-ui-devices/red-pass2.log 2>&1
EXIT=1
```

`red-pass2.log`: 17 tests, 16 fail, 0 skip, exit 1. Все 16 падений — `AssertionError
[ERR_ASSERTION]`, ни одного `ERR_MODULE_NOT_FOUND`, `TypeError` или падения загрузчика.

Первые строки ошибок (по TEST-POLICY §4):

```
✖ начальное состояние: default-устройства и громкости из оракула
  AssertionError [ERR_ASSERTION]: выбран default-микрофон, а не первый в списке
✖ длинное имя устройства: полный текст и title, без обрезки в DOM
  AssertionError [ERR_ASSERTION]: в контроллере нет #voice-devices-input
✖ hot-plug без устройств: NoDeviceAvailable, пустой disabled-список
  AssertionError [ERR_ASSERTION]: событие на каждое направление
✖ ARIA: русские подписи, роли, значения и live-регион
  AssertionError [ERR_ASSERTION]: Expected values to be strictly equal:
✖ ошибка сервиса показывается в alert, а не глотается
  AssertionError [ERR_ASSERTION]: в контроллере нет #voice-devices-input
```

Падения покрывают все контрактные группы: выбор default (`mic-builtin`, `spk-headset`),
явный выбор и отказ неизвестному id, hot-plug (сохранение, откат на `isDefault`,
`NoDeviceAvailable`), клампы 0..100 / 0..200 / 0..2000 / i8 −128..127, PTT и захват
клавиши, `effectiveMute = userMute || selfDeaf`, ARIA и проброс ошибок сервиса.

Один тест из 17 проходит — «приватность и запрет медиа-API». Это статическая проверка
исходника на отсутствие `console.`, `getUserMedia`, `fetch(` и т.п.; заглушка её
удовлетворяет по построению, потому что ничего не делает. Поведенческих assertion там нет,
поэтому на доказательство RED это не влияет. Отдельно отмечу: этот тест не различает
заглушку и реализацию, его сигнал — только против регрессий, а не против отсутствия кода.
Это наблюдение по покрытию, а не дефект pass2.

## 3. GREEN: точное восстановление и прогон

```
cp /tmp/opencode/voice-devices-settings-pass2.baseline.js rust/gui-spike/ui/features/voice-devices-settings.js
md5sum rust/gui-spike/ui/features/voice-devices-settings.js  # 5fda00c8c063f9726fc1ab31ac13ef99
node --test tests/unit/voice-devices-settings.test.mjs > waves/RW-40/PM/v07-ui-devices/green-pass2.log 2>&1
EXIT=0
```

`green-pass2.log`: 17 tests, 1 suite, 17 pass, 0 fail, 0 skipped, 0 todo, exit 0.

## 4. Мутации

`mutations.log` из pass1 не перегенерировался: продуктовый файл после восстановления
побайтово тот же (MD5 `5fda00c8c063f9726fc1ab31ac13ef99`), значит результаты 5/5 KILLED
остаются в силе для этого же кода. В `mutations.log` у каждого мутанта `Вердикт: KILLED` и
`Restore MD5: 5fda00c8c063f9726fc1ab31ac13ef99`.

## 5. Файлы и MD5

| Файл | MD5 |
| --- | --- |
| `rust/gui-spike/ui/features/voice-devices-settings.js` | `5fda00c8c063f9726fc1ab31ac13ef99` |
| `tests/unit/voice-devices-settings.test.mjs` | `bf7544e0c25c18ca0e8eaba579bca7bc` |
| `waves/RW-40/PM/v07-ui-devices/red-pass2.log` | `a1a4c9b7e87495541e085b36225ba7cb` |
| `waves/RW-40/PM/v07-ui-devices/green-pass2.log` | `4ba34d32935c9b64bd1626ecf3b370b6` |
| `waves/RW-40/PM/v07-ui-devices/mutations.log` | без изменений (pass1) |

Затронут только продуктовый файл карточки (дважды: подмена и восстановление), тест не
правился. Добавлены `red-pass2.log`, `green-pass2.log`, `REPORT-PASS2.md`.

## 6. Остаточные риски

- `npm run gate:worker` по-прежнему за integration-лейном (P2 из REVIEW не снят этим
  отчётом и не должен сниматься: batch-gate обязателен до приёмки).
- RED доказан на заглушке, а не на удалённом файле. Для REVIEW это закрывает P1: падение
  поведенческое. Но заглушка — искусственный оракул; настоящий red-first цикл из pass1
  остаётся историей с `ERR_MODULE_NOT_FOUND`.
- Резервная копия лежит в `/tmp/opencode`, вне репозитория. Если `/tmp` будет очищен до
  ревью, восстановление возможно только из артефакта `red-pass2`-последовательности; лучше
  не полагаться на неё как на долговременный источник.
