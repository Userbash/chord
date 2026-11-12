# MUTATION REPORT — B05 / RW-24 (P7-01)

Дата: 2026-10-21. QA-раннер: `run-mutations.py` (не автор кода).
Цель: ≥18 caught, 0 missed; inconclusive допустимы только с объяснением.

| Метрика | Значение |
|---|---:|
| Мутаций в матрице | 20 |
| caught | **20** |
| missed | **0** |
| inconclusive | **0** |
| восстановление | все исходники восстановлены, md5 до/после совпали |

## Покрытые контракты

- plural: ru few, pl 11–14, ar zero/two, fr million, en ordinal 11, he 0.5;
- format: ru NBSP, fr decimal, hi 3/2 grouping, ru month context, th +543,
  AM/PM, unknown currency, date/time second sample;
- markdown: bold, unsafe URL, list depth 11, escape, UTF-8 panic, list overflow,
  nested list, timestamp exact style, slash command charset/length;
- locale: disabled filter, fallback chain;
- integration: exact `=N` before offset, Date/Time style wiring, Number non-numeric.

## Воспроизведение

```sh
cd /home/user/orca/project
python3 waves/RW-24/run-mutations.py
```

Логи: `waves/RW-24/mutations/mutation-01.log` … `mutation-20.log`;
машиночитаемый итог — `waves/RW-24/mutations/results.json`.
