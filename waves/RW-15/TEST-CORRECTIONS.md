# RW-15 — red-статус и поправки тест-спеки

## 1. Red

`cargo test --offline -p discord-core member_list` → `waves/RW-15/red-w15.log`,
`RED_EXIT=101`. Это **compile-red**: модуля `state::member_list` и типов `MemberList*` ещё не
было; проверить поведение ops до появления API нельзя. Отдельного behavior-red у W15a/b/c
нет; это ограничение честно зафиксировано.

После первого green-прогона найдены ошибки самой тестовой обвязки (не продукта) и исправлены
до финального green:

| Кейс | Было | Стало | Почему |
|---|---|---|---|
| `member_list_reducer` helper | импорт `list` и функция `list` конфликтовали | helper `mlist` | ошибка компиляции теста |
| `s08` | `021` (s=16), затем `008` (s=6) | `008`, затем `021` | второе событие stale по контракту P2-02 |
| `m04` | ожидал на index 2 старый element 7025… | index 2 = 9510…, index 3 = 7025… | INSERT сдвигает хвост |
| `m11` | `inconsistencies.len()==2` | считаются только `out-of-range` (2) | неизвестная гильдия даёт третью запись |
| `m06` | путь диагностики содержит `member` | путь `d.ops[0].items[1]` | диагностика адресует элемент списка |

## 2. Legacy 005/006

Исходные `005`/`006` имеют `guild_id=-1467272931325252608` — артефакт старого big-int
декодера. Политика:

- исходный кадр → `EventError::InvalidId { path: "d.guild_id" }`, не паника;
- shape/ops проверяются на копии, где в памяти заменён только `guild_id` на подтверждённый
  `1014537851418980372`; 98/1 отрицательных member-id → `malformed`, в items остаются
  только группы;
- это не утверждение, что 005/006 валидны, и не подгонка под код: отрицательные ID не
  попадают в `u64`-модель.

## 3. Ревью P2-1 (red-first)

Независимое ревью нашло дефект: `INSERT` в позицию существующего member-id не писал
`duplicate`, потому что проверка исключала `position == index`; после вставки элемент
сдвигался и оставался дубликатом.

- red: `member_list_reducer::m15_insert_duplicate_at_same_position_is_diagnostic` →
  `red-w15-p21.log`, **exit 101**, `duplicates=[]`, items `[42, 42]`;
- green: `green-w15-p21.log`, exit 0; проверка перед `INSERT` теперь идёт по всем существующим
  элементам.

P3 из ревью закрыты в `green-w15-p3b.log`:
unknown-op → `inconsistencies` с префиксом `member_list:{guild}:{list}:unknown-op:{op}`;
успешный in-range `UPDATE`; два list_id одной гильдии; битые `groups`/overflow `count`;
отсутствие/пустой `id`, не-u64 `guild_id`; одиночный `malformed` для битого item; пустые
ключ/значение `client_status` → `ignored`; out-of-range `INSERT`. Пустой `id` зафиксирован
как `WrongType` в DESIGN §3.

## 4. Архив

- `tests-final/` — финальные тесты (types 14, reducer 17, selectors 9 = **40**), md5 в
  `tests-final/MD5SUMS.txt`; md5 продукта — `tests-final/SOURCE-MD5SUMS.txt`.
- `tests-red/` пуст: red был compile-red до существования модуля; доказательство — лог с
  exit 101. Отдельная копия «тестов до API» не имеет смысла, потому что они не компилировались.
- Красные/зелёные логи: `red-w15.log`, `green-w15.log`, `green-core-final.log`,
  `green-workspace-final.log`, `clippy-final.log`.
