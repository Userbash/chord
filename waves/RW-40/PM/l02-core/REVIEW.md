# REVIEW-L02-CORE

Дата: 2026-10-24
Режим: code review only; код, тесты и gate не запускались и не изменялись.

## Вердикт

**RETURN.** P1 findings остаются; L-02-CORE нельзя принять по текущему diff/доказательствам.

## Findings

### P1 — уже разобранные op0 теряются при ошибке в том же WS-фрагменте

`rust/crates/discord-core/src/ws.rs:648-652` вызывает `flush_dispatches` только после
успешного `pipe.push_binary(...)`. `GatewayPipe::push_binary` добавляет события в
`pipe.events` по мере разбора; если после валидного op0 в том же binary-фрагменте
возникает `TransportError::Zstd`/`Frame`, `?` выходит до flush. Supervisor получает
ошибку и закрывает попытку, а уже разобранный op0 не вызывается в `on_dispatch`.
Это нарушает заявленное «ровно каждый op0» и допускает потерю кадров. Тест с валидным
op0 перед повреждённым хвостом отсутствует. Нужна явная политика: гарантированно
дренировать уже разобранные события перед возвратом ошибки либо документировать и
считать потерю частью контракта.

### P1 — md5 в отчёте не соответствует проверяемому тесту

`REPORT.md:100` заявляет md5 `b9d806c6d243c625059eb1e924078d7e` для
`rust/crates/discord-core/tests/gateway_supervisor.rs`, но текущий файл имеет md5
`84f5fcc1404795faec939a24c15462d1`. Остальные перечисленные слепки совпали.
Следовательно, `green-core-targeted.log` и red-first доказательство нельзя
однозначно привязать к текущему reviewed test artifact. Обновить слепок и повторно
зафиксировать соответствие отчёта после проверки содержимого.

### P2 — oracle redaction не проверяет точный секретный ключ `token`

`rust/gui-spike/tests/net_contract.rs:361-367` считает секретом `*_token`, но не
точный ключ `token`; контракт `CONTRACTS/dispatch.md:90-93` требует удалять и
`token`, и `access_token`, и `refresh_token`. Реализация `net.rs:172-183` эти ключи
удаляет, но текущий независимый тест не доказывает точный `token` и его `redacted`-
счётчик. Добавить в oracle отдельный вложенный `token` и проверку, что он исчез и
учтён.

## Что проверено

- `RunOptions.on_dispatch` и `SupervisorConfig.on_dispatch` проброшены в каждую
  попытку; callback получает имя, `s`, `d`, включая raw unknown event.
- `flush_dispatches` стоит до `apply_effects`, поэтому события перед close/outcome
  не теряются в обычном успешном разборе.
- GUI-фильтр по шести именам, JSON-конверт, recursive redaction, `try_send`,
  counters и sequence dedup согласованы с контрактом; тесты проверяют порядок,
  overflow, emit error и reset на reconnect.
- В тестах не обнаружены `.only`/`skip`; fixture absence сделан skip-aware. Red log
  содержит ожидаемую compile failure до хука; green logs показывают заявленные
  targeted/all результаты.

## Не доказано этим срезом

- Подписка UI на `gateway://dispatch` и проекции по `kind` не реализованы (G2).
- Доставка события до реального окна и live Discord не проверялись; это прямо
  отмечено в `REPORT.md` и `CONTRACTS/dispatch.md`.
- Повреждённый кадр после уже разобранного op0 не покрыт oracle-тестом.

