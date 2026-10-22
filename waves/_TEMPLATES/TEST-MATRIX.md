# TEST-MATRIX — <batch>/<card>

| ID | Уровень | Проверка | Оракул (независимый) | Ожидание | Red-лог | Green |
|---|---|---|---|---|---|---|
| a01 | L1/unit | парсер поля X | fixture `010`, `jq '.d.x'` | ... | `red-a01.log` | `green.log` |
| r01 | L3/reducer | stale не меняет state | synthetic pair + oracle §... | ... | ... | ... |
| s01 | L3/selector | выбор лучшего scope | official bundle, строка ... | ... | ... | ... |

Правила:
- ID не переиспользуются между карточками;
- у каждого кейса есть оракул: реальная фикстура/RE-выдержка/jq/md5/loopback, а не «так
  написал код»;
- табличные варианты — один ID с `#[case]`/table, а не 10 копий;
- бюджет: S 6–10, M 10–16, L 16–24, XL 24–32 (`TEST-PLAN.md` §1.1);
- red-лог должен содержать `FAILED` нужного ID, а не только compile error;
- `TEST-CORRECTIONS.md` — если ожидание менялось после red.
