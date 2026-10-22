# Отчёт воркера — <batch>/<card>/<seat>

## 1. Границы

- Файлы (только эти): <точные пути>
- Не трогал: shared-файлы, чужие тесты, доки, golden, Cargo/package.
- Пересечений с другими воркерами: нет.

## 2. Red

```sh
<команда>
# EXIT=<101/1/…>; <N> failed: <ID/имена>
```
Лог: `waves/<RW>/workers/<seat>/red-*.log`.

## 3. Green

```sh
<targeted-команда>
# EXIT=0; <N>/<N> passed
```
Лог: `waves/<RW>/workers/<seat>/green-*.log`.

## 4. Файлы и md5

| Файл | md5 |
|---|---|
| ... | ... |

## 5. Риски/вопросы

- <...>

## 6. Передача

- next: <TL-2 integration / QA / review>
- batch gate: <не запускал — у integration/QA>
