# TEST-MATRIX B04 / RW-23

| Требование | Тест | Оракул/данные | Статус |
|---|---|---|---|
| T-VOICE-04 discovery 74 Б | `discovery_matches_strace_request_and_response_oracles` | `strace-media-s3.jsonl` записи 0–2; request port 0 и 33801 | pass |
| T-VOICE-04 malformed discovery | `response_round_trips_and_rejects_malformed_fields` | length/type/len/address/port | pass |
| T-VOICE-04 encode validation | `encode_rejects_invalid_response` | 65-байтный/пустой/не-IP address, port 0 | pass |
| T-VOICE-04 oversized datagram | `discovery_receivers_reject_oversized_datagrams` | 75/9 Б loopback | pass |
| T-VOICE-05 keepalive | `keepalive_constants_match_re_and_interval` | `1337cafe`/`1337f00d`, 5 000 мс | pass |
| T-VOICE-05 sequence | `keepalive_sequence_matches_capture_and_increments` | capture `01000000`, `02000000` LE | pass |
| T-VOICE-06 rotation | `rotation_boundary_and_action_match_capture_contract` | 60 079/60 080, 120 260 | pass |
| T-VOICE-06 socket rotation | `rotate_if_due_integrates_schedule_and_socket` | schedule + loopback, order discovery→RR | pass |
| T-VOICE-06 SRTCP RR | `rotate_srtcp_sends_captured_28_byte_rr` | capture `80c90001...16000080` | pass |
| T-VOICE-16 RTCP RR | `empty_receiver_report_has_exact_rtcp_header_and_ssrc` | 8 Б, `80c90001` | pass |
| T-VOICE-16 SRTCP wrapper | `empty_srtcp_receiver_report_matches_capture` | 28 Б, tag + LE counter | pass |
| source/SSRC validation | `receivers_reject_foreign_peer_and_wrong_ssrc` | foreign loopback peer, wrong ssrc | pass |
| panic-free | `parser_never_panics_on_prefixes_or_arbitrary_packets` | все префиксы + 0..128 | pass |
| T-HARN-07 SPDX | `generates_spdx_23_with_locked_packages_and_licenses` | 85 lock packages, relationships, namespace, repeat | pass |
| T-HARN-07 allowlist | `checks_allowlist_and_rejects_forbidden_license` | `--check`, GPL reject | pass |
| T-HARN-07 precedence/WITH | `license_expression_precedence_and_with_exceptions_are_checked` | AND/OR/скобки/WITH/invalid chars/LLVM | pass |
| T-HARN-07 CI/release | `ci_and_release_dry_runs_keep_their_contracts` | dry-run + actual hook + `-ef` | pass |
| offline/no-network | `sbom_and_hooks_stay_offline_and_use_no_network_clients` | scripts source | pass |
| regression | full workspace | `test-workspace-b04-review.log` | 638/0 |
