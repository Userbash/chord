| # | Мутация | md5 мутанта | exit | Итог | Убившие тесты |
|---|---|---|---|---|---|
| M1 | skip user_id snowflake validation | `122bf509e6a07c06b2ddaaf38d7062a5` | 101 | **KILLED** | `invalid_ids_are_rejected_before_any_transport_call`, `user_id_must_be_a_non_empty_ascii_digit_snowflake` |
| M2 | omit guild_id query param | `683b6cac457a5a13833906d15afddd97` | 101 | **KILLED** | `fetch_forwards_the_guild_scoped_path`, `guild_id_is_appended_last_exactly_as_in_the_capture` |
| M3 | allow empty user.id in parse_user_profile | `bbe2a39861cdbcde41e2a968999dadd7` | 101 | **KILLED** | `empty_or_missing_user_identity_is_invalid_response` |
| M4 | return retry_after_ms=0 on 429 | `af29752f6bd28991d5195fcbe0d5a0e5` | 101 | **KILLED** | `fetch_429_extracts_retry_after_from_headers`, `fetch_429_extracts_retry_after_from_json_body` |
| M5 | map 404 to Unauthorized | `355ee810d35a2b6c1e82e412e95446d8` | 101 | **KILLED** | `http_statuses_map_to_typed_errors` |

Итого: **5/5 убито**, восстановленный md5 = `c29689e4daf174f62ba2e0e3e993e4be` (ожидался `c29689e4daf174f62ba2e0e3e993e4be`).
