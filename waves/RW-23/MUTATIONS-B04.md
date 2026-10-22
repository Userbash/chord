# MUTATIONS B04 / RW-23

| # | Мутация | Тест | Результат | Лог |
|---|---|---|---|---|
| 1 | SSRC offset в discovery | `discovery_matches...` | caught 101 | `mutation-w24a-offset.log` |
| 2 | длина discovery | `response_round_trips...` | caught 101 | `mutation-w24a-len.log` |
| 3 | type discovery | `response_round_trips...` | caught 101 | `mutation-w24a-type.log` |
| 4 | интервал ротации | `rotation_boundary...` | caught 101 | `mutation-w24a-rotation.log` |
| 5 | keepalive magic | `keepalive_constants...` | caught 101 | `mutation-w24a-keepalive.log` |
| 6 | RR type | `empty_receiver_report...` | caught 101 | `mutation-w24a-rr.log` |
| 7 | request port запрещён | `discovery_matches...` | caught 101 | `mutation-w24a-fix-request-port.log` |
| 8 | keepalive sequence BE вместо LE | `keepalive_sequence...` | caught 101 | `mutation-w24a-keepalive-endian.log` |
| 9 | magic-проверка только seq1 | `loopback_send_receive...` | caught 101 | `mutation-w24a-keepalive-magic.log` |
| 10 | recv buffer 128→74 | `discovery_receivers...` | inconclusive (loopback skip) | `mutation-w24a-fix-truncation.log` |
| 11 | allowlist без CDLA | `checks_allowlist...` | caught 101 | `mutation-w24b-allowlist.log` |
| 12 | SPDX version 2.2 | `generates_spdx...` | caught 101 | `mutation-w24b-spdx.log` |
| 13 | AND/OR precedence | `license_expression...` | caught 101 | `mutation-w24b-precedence.log` |
| 14 | CI actual hook удалён | `ci_and_release...` | caught 101 после усиления теста | `mutation-w24b-ci.log` |
| 15 | namespace не sha256 | `generates_spdx...` | caught 101 | `mutation-w24b-namespace.log` |
| 16 | encode не валидирует Response | `encode_rejects_invalid_response` | caught 101 | `mutation-b04-encode-invalid.log` |
| 17 | peer-check удалён | `receivers_reject_foreign_peer...` | caught 101 | `mutation-b04-source-check.log` |
| 18 | rotate_if_due всегда None | `rotate_if_due_integrates...` | caught 101 | `mutation-b04-rotate-due.log` |
| 19 | RR header 00 07 вместо 00 01 | `empty_receiver_report...` | caught 101 | `mutation-b04-rr-header.log` |
| 20 | tokenizer глотает невалидные символы | `license_expression...` | caught 101 | `mutation-b04-invalid-token.log` |
| 21 | license fallback удалён | `generates_spdx...` | caught 101 | `mutation-b04-license-fallback.log` |
| 22 | LLVM-exception как standalone | `license_expression...` | caught 101 | `mutation-b04-llvm-standalone.log` |

Итог: **21 caught**, 1 inconclusive (не засчитана). После каждой мутации файл восстановлен;
для Rust — `cp` + `touch`, затем зелёный targeted.
