**W25a / P7-01 выполнено.** Добавлены locale fallback, cardinal/ordinal plural и форматирование чисел, дат, времени и skeleton на данных CLDR. Contract-тесты сверяют 36 локалей с `cldr-oracle.json`.

Red-прогоны трёх targeted-наборов завершились ожидаемым падением на заглушках, exit code `101`; команды и вывод сохранены в [red-W25a.log](/home/user/orca/project/waves/RW-24/red-W25a.log). Финальные targeted-прогоны зелёные: locale — 2 теста, plural — 2, format — 3; результаты в [green-W25a.log](/home/user/orca/project/waves/RW-24/green-W25a.log). Clippy пакета и `rustfmt --check` назначенных файлов прошли. `cargo fmt --all -- --check` обнаружил форматирование в чужом существующем `tests/i18n_contract.rs`; этот файл не менял.

MD5:

- `locale.rs` — `25a8c4d0cf86d067e0d8b320d17261ac`
- `plural.rs` — `e34095b967a5a6aece93a6b79a64c726`
- `format.rs` — `b41504e77de0572fbc451834ea0900e7`
- `locale_contract.rs` — `859b387dd043b031d1685a2cc273cc7d`
- `plural_contract.rs` — `22b11f9fcaa341221705488c83c3c0e3`
- `format_contract.rs` — `5e80545ead5727e67cdef3467303ebf8`
- Red-лог — `202c0b67151a48d15693ae276837ab31`
- Green-лог — `31de078eb67e9ce6a3cdf8c55563cd69`

Риск: правило `he` для значений, оканчивающихся на 0, в TZ расходится с oracle (`10`: TZ ожидает `many`, oracle — `other`); реализация следует oracle. Полный batch gate не запускал согласно ТЗ.