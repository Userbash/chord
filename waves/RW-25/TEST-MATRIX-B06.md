# TEST-MATRIX-B06 — волна B06 / RW-25

Собрано автоматически из исходников тестов (2026-10-22). Контракты — `TZ-W26*.md`,
RE — `research/`.

| Карточка | Тест | Оракул |
|---|---|---|
| P4-03 (crypto KAT) | `aes256_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `aes256_decrypt_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `aes256_wrong_key_len` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `aes256_key_debug_redacted_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `gcm_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `gcm_aad_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `gcm_empty_plaintext_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `gcm_tamper_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `gcm_short_input_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `chacha20poly1305_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `xchacha20poly1305_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `hkdf_sha1_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `hkdf_sha1_zero_salt_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `hkdf_sha1_limits_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `sbox_matches_fips197_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (crypto KAT) | `aead_rejects_overlong_message_k` | NIST SP 800-38D / FIPS-197 C.3 / RFC 8439 §2.8.2 / RFC 5869 A.2–A.3 / draft-irtf-cfrg-xchacha A.2–A.3.1 — векторы вшиты константами, сверены независимым оракулом (OpenSSL / Node `crypto` / Python `cryptography`) |
| P4-03 (SRTP контракт) | `rtp_layout_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `rtp_counter_shared_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `srtcp_layout_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `rtp_roundtrip_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `srtcp_roundtrip_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `rtp_tamper_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `srtcp_tamper_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `malformed_43b_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `short_packet_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `mode_names_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `unsupported_mode_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `non_rtpsize_mode_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `counter_overflow_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `rtp_version_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `rtp_ext_len_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `srtcp_version_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `incoming_counter_non_monotonic_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `xchacha_rtpsize_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `incoming_srtcp_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `crypto_error_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `counter_rewind_rejected_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P4-03 (SRTP контракт) | `decrypt_nonce_reuse_rejected_k` | capture `strace-media-s3.jsonl` (`research/RE-SRTP-LAYOUT.md`): 47/60/28/52/43 Б; функции клиента `PacketCrypter::HeaderLength` / `SpaceRequiredToEncrypt` (`research/RE-NATIVE-SRTP.md`) |
| P7-02 (перф-харнесс) | `determinism_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `different_seed_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `spec_reference_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `report_shape_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `ingest_applied_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `regression_none_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `regression_detected_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `regression_below_threshold_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `regression_zero_baseline_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `regression_zero_baseline_to_nonzero_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `json_roundtrip_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `json_float_roundtrip_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `presence_for_every_member_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `metric_names_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `threshold_constant_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `budget_cold_start_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P7-02 (перф-харнесс) | `reference_report_k` | независимый FNV-1a в тесте + публичный `apply` редьюсера + независимый разбор `serde_json::Value`; цифры — на машине волны |
| P8-01 (updater) | `manifest_parse_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `manifest_parse_pre_release_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `manifest_invalid_json_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `manifest_missing_field_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `manifest_invalid_version_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `manifest_invalid_digest_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `manifest_duplicate_artifact_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `signature_ok_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `signature_bad_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `signature_wrong_key_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `signature_tampered_body_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `signature_missing_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `version_order_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `version_parse_rejects_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `parse_manifest_stays_plain_manifest_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `verified_manifest_from_signed_bytes_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `verified_manifest_requires_valid_signature_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `plan_from_verified_manifest_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `plan_not_newer_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `plan_unknown_artifact_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `plan_steps_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `digest_mismatch_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `staged_swap_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `rollback_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `swap_failure_rollback_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `sha256_hex_known_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `rfc8032_test_vector_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `plan_rejects_unsafe_artifact_name_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `plan_keeps_all_paths_inside_install_dir_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `plan_accepts_safe_artifact_name_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `rollback_replaces_existing_target_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `rollback_refuses_occupied_temp_name_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `rollback_missing_backup_keeps_target_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |
| P8-01 (updater) | `rollback_rejects_corrupted_backup_k` | RFC 8032 TEST 1 (Ed25519), `sha256sum` для SHA-256, `openssl` для сверки публичного ключа; файлы — под `std::env::temp_dir()` |

- **P4-03 (crypto KAT)** — 16 тестов.
- **P4-03 (SRTP контракт)** — 22 тестов.
- **P7-02 (перф-харнесс)** — 17 тестов.
- **P8-01 (updater)** — 34 тестов.

Итого новых тестов в волне: **89** (+ 1 doctest в `discord-updater` — `compile_fail`, доказывающий, что `plan_update` не принимает неподписанный `&Manifest`; + 1 интеграционный `r19_presence_upsert_scales_without_full_store_scan` в `discord-core`).

Интеграционный патч I1 (TL-2, общий файл `discord-core/src/state/presence_reducer.rs`): `r19_presence_upsert_scales_without_full_store_scan` — red 3.17 с на 10 000 upserts (O(n²)), green 0.03 с (диапазон по ключу `(user_id, scope)`).
