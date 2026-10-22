# Ревью TL-1 — RW-26 (P4-07 + P4-11)

Дата: 2026-10-22. Автор кода: W26a/W26b. Ревьюер: TL-1 (не автор). Метод: чтение
контрактов/TZ, RE-доков, продукта и тестов, прогон targeted и мутаций, сверка провенанса
`sha256`/`md5`.

## Вердикт

**Принято после red-first фиксов.** P0=0, P1=0, P2=0. Остаточные P3 — в
`waves/RW-26/REPORT-B09.md` §4.

## Находки

### P2-1 — RTP timestamp wrap ломал джиттер (закрыт red-first)

`reception.rs` считал `timestamp_delta = f64::from(ts) - f64::from(prev)`. На переходе
`u32::MAX → 0` дельта становилась ≈ 4.29e9, джиттер прыгал до `221184000` единиц
(≈ 4608 с). Red: `waves/RW-26/workers/TL2/red-timestamp-wrap.log` (exit 101). Фикс:
`ts.wrapping_sub(prev) as i32` (знаковая модульная разность). Green:
`green-reception.log` (exit 0). Мутация M11 ловится (`11/11`).

### P2-2 — тест `regions_contract` проверял копию файла, а не крейт (закрыт)

`#[path = "../src/regions.rs"] mod regions;` + локальная копия `VoiceError` означали, что
тест компилировал `regions.rs` в контексте `cfg(test)`, а продукт — нет. В
`select_measured_region` была отдельная `#[cfg(test)]`-ветка, то есть тесты и продукт
шли разными путями. Фикс: `pub mod regions;` в `lib.rs`, тест использует
`discord_voice::regions::*`, `#[cfg]`-ветка удалена, production-путь один.

### P2-3 — граница `MAX_COMPOUND_PACKETS` не наблюдалась (закрыт тестом)

Существующий тест проверял только `MAX+1`; мутант `>` → `>=` проходил. Добавлен тест
«ровно 32 пакета принимаются»; M4 теперь caught.

### P3-1 — `TooManyRegions` для >32 endpoint'ов (закрыт)

Введён `TooManyEndpoints`; тест на 33 endpoint'а.

### P3-2 — неканонический порт (осознанно)

Отказ `+1`, `01`, ведущих нулей — гигиена ввода, не подтверждена RE. Зафиксировано в
backlog; при необходимости ослабить до digits-only.

### P3-3 — `DeepLink`/`query` в логах (вне RW-26, RW-27)

В RW-27 `DeepLink` деривит `Debug` и печатает `raw`/`query` с `token`. Это P2 для P6-04;
исправление — в RW-27 (manual `Debug` с redaction), не в RW-26.

## Проверка провенанса

- `voice-udp4.pcap` sha256 начинается `0e6e2f8c…` — совпадает с RE-доком;
- `rtcp-voice-udp4-server.bin` md5 `795d0af0…` — совпадает;
- `rtp-udp4-client.json` md5 `c6991460…` — совпадает;
- `regions-rtc.json` md5 `ffe52792…` — совпадает; 5×5 endpoint'ов;
- engine-log `frankfurt,rotterdam,paris,milan,brussels` воспроизводится тестом
  `ranking_matches_engine_log_and_ties_use_geo_then_lexical`.

## Что не проверялось

- Живой Discord/UDP — вне окна владельца; wire-совместимость latency-проб не заявлена.
- Значения report block в capture зашифрованы; RTT/потери проверены RFC-векторами.
