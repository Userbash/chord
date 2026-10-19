# Фикстуры обратной разработки (конформанс-набор)

Курированный набор артефактов волны RE (R7), на котором будут проверяться кодеки, схема
шлюза, REST-слой, i18n и голосовой стек Rust-клиента (`docs/rust-client/TEST-PLAN.md` §3).

- Сырьё не редактируется: файлы — побайтовые копии из лаборатории `/home/sanya/re-deep`
  (кроме вырезанных секретов, см. ниже).
- Провенанс каждого файла (путь-источник, размер, md5, назначение, тесты) — в `MANIFEST.json`.
- Полная лаборатория (pcap, strace, дампы, каталоги локалей) — только в `/home/sanya/re-deep`;
  здесь минимальный срез, достаточный для L1–L3 тестов.
- **Сантизация:** значения ключей `analytics_token`, `session_id`, `auth_session_id_hash`,
  `static_client_session_id`, `qos_token`, `resume_gateway_url` (и email) → `REDACTED`.
  В декоде READY (`gateway/by-type/002-READY.json`) изначально был **реальный
  analytics_token** — при копировании вырезан (`docs/rust-client/VERIFICATION.md` §2.5).
  Сборка и скан санитизации — `tools/make-fixtures.mjs`; проверка — SAN-01 верификатора.

| Фикстура | Что это | Тесты |
|---|---|---|
| `gateway/by-type/` (164 файла) | декодированные диспатчи по типам (12 типов) | T-GW-08, T-DATA-06 |
| `gateway/ready-stats.json` | счётчики READY: 7 гильдий, 17 ЛС, 60 связей | T-GW-04, T-DATA-02 |
| `gateway/summary.json` | сводка кадров сокета (op/t/s/размеры) | T-GW-01, T-ENC-04 |
| `gw-proto/facts-frozen.json` | замороженные факты: ETF-теги (11), счётчики | T-ENC-02, T-GW-08 |
| `gw-proto/msgs-run2-95974_13072.jsonl` | декодированные кадры одного сокета | T-ENC-04, T-GW-02 |
| `rest/rest-static.json` | статический реестр REST (904 эндпоинта) | T-REST-01, T-REST-09 |
| `i18n/i18n-stats.json` | i18n: 23 836 ru-ключей, модули, локали | T-I18N-01, T-I18N-07 |
| `voice/rtc*-{frames,summary}` | RTC-сигнализация: кадры и сводки (сессии 1–2) | T-VOICE-01…03 |
| `voice/session3.timing` | тайминги ротации медиа-сокета (сессия 3) | T-VOICE-06 |

## Команды

```sh
node fixtures/tools/make-fixtures.mjs            # пересборка из лаборатории (точечно, README/tools не трогает)
node docs/rust-client/verify/verify-re.mjs       # сверка: md5, счётчики, константы, санитизация (SAN-01)
```
