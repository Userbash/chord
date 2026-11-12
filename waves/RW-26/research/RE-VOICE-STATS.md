# RE-VOICE-STATS — RTCP/RTP-оракул для P4-07 (B07/RW-26)

Seat: RE-1. Дата: 2026-10-22. Метод: статический/офлайн-разбор pcap, живой Discord не
подключался, сеть не использовалась. Golden `reference-oracle/` — только чтение.

## 1. Источники и провенанс

| Артефакт | sha256 | Использование |
|---|---|---|
| `reference-oracle/voice/voice-udp4.pcap` | `0e6e2f8c…d40e1e2` | RR-структура, RTP-заголовки, DTX-кадры |
| `reference-oracle/voice/voice-udp3.pcap` | `b655c13a…495926f3` | второй прогон, SR-структура |
| `reference-oracle/voice/rtp-analyze.mjs` | — | сверка раскладки pcap (linktype 276) |
| `reference-oracle/voice/engine-log-session1.txt` | — | ранжирование регионов |

Разбор: `waves/RW-26/research/rtcp-extract.py` (Python stdlib, read-only скрипт).

```sh
python3 waves/RW-26/research/rtcp-extract.py \
  reference-oracle/voice/voice-udp4.pcap waves/RW-26/research/rtcp-udp4.json --fixture 20
```

Выход: `rtcp-udp4.json` (все RTCP/RTP-потоки), `fixtures/rtcp-voice-udp4-server.bin`
(20 первых RTCP-пакетов сервера с 4-байтным BE-префиксом длины), `fixtures/rtcp-voice-udp4-server.json`
(ожидаемый разбор), `fixtures/rtp-udp4-client.json` (8 RTP-заголовков клиента).

## 2. Подтверждённая структура на проводе

Голосовой UDP-поток Discord в capture — SRTCP, но **заголовок RTCP и первые 8 байт
открыты**; тело RR зашифровано, поэтому значения report block из capture **не извлекаются**.

```text
сессия 3 (voice-udp4.pcap), сервер -> клиент, 48 Б:
  0..4    RTCP common header: V=2, P=0, RC=1, PT=201 (RR), length=7
  4..8    sender SSRC (0x000023f8)
  8..32   зашифрованный report block (24 Б)
  32..48  trailer 16 Б (auth tag, rtpsize)

сессия 3, клиент -> сервер, 48 Б:
  0..28   RTCP SR: header 8 + sender info 20 (тело SRTP-шифруется)
  28..48  trailer 20 Б (tag 16 + counter 4)

сессия 2 (voice-udp3.pcap), другие формы:
  сервер -> клиент, 52 Б = RR 32 + trailer 20 (tag 16 + counter 4)
  клиент -> сервер, 60 Б = SR 28 + trailer 32
```

Важно: «sender info» и report block в capture — **шифротекст**. Парсер проверяет
структуру/длины/склейку NTP, но не должен трактовать значения как реальные потери/часы.
Семантика полей — только на расшифрованном RTCP.

Факты для парсера:

1. `parse_compound` обязан останавливаться на первом байте, у которого `V != 2`, и
   возвращать `trailer_len` (16 или 20) — это SRTCP-хвост, а не «мусор».
2. Длина RTCP-пакета = `(length_words + 1) * 4`; она не покрывает auth tag.
3. Реальные RR: `RC=1`, `PT=201`, `length=7` (32 Б). Реальные SR: `RC=0`, `PT=200`,
   `length=6` (28 Б).
4. В capture **нет** SR/RR с открытыми report block (тело SRTCP зашифровано) → значения
   fraction lost / cumulative lost / jitter / LSR / DLSR проверяются синтетическими
   векторами по RFC 3550, а не capture-ом.

## 3. RTP-заголовки (открыты в режиме `*_rtpsize`)

8 кадров клиента, SSRC 9208, PT 120 (Opus), заголовок 24 Б (12 фикс + 4 ext + 8 данных),
payload 23 Б (DTX/CN), интервал 20.8 мс (не 20.0 — реальный джиттер сети).

| seq | RTP ts | приход, мс | payload, Б |
|---:|---:|---:|---:|
| 17466 | 278470988 | 1792395568071.428 | 23 |
| 17467 | 278471948 | 1792395568091.893 | 23 |
| 17468 | 278472908 | 1792395568092.791 | 23 |
| 17469 | 278473868 | 1792395568113.603 | 23 |
| 17470 | 278474828 | 1792395568156.431 | 23 |
| 17471 | 278475788 | 1792395568157.201 | 23 |
| 17472 | 278476748 | 1792395568178.493 | 23 |
| 17473 | 278477708 | 1792395568220.841 | 23 |

Независимый оракул джиттера (RFC 3550 §6.4.1, clock 48 000):

- `J` = **222.141** timestamp-единиц = **4.628 мс**;
- потерянных пакетов: **0** (seq 17466…17473 без пропусков);
- gap p50 = 20.812 мс, max = 42.828 мс.

Эти числа зафиксированы в `fixtures/rtp-udp4-client.json` и обязаны воспроизводиться
Rust-реализацией на тех же входах.

## 4. Что НЕ подтверждено

- Значения report block (зашифрованы) — только RFC-векторы.
- Семантика LSR/DLSR на стороне Discord (стандартная RFC 3550, но живых открытых пар
  в capture нет) — RTT-оценка помечена как «по RFC», не «по capture».
- 43-байтные talkspurt/закрытые тела RTCP — см. `docs/re/voice-deep.md` §3.
