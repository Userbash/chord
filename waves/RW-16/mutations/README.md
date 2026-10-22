# P2-05 — мутационная проверка

Источник: `rust/crates/discord-core/src/state/member_chunk.rs` (md5 до/после
`13d19214a4e16f8c890797e9f5468e7e`). Мутации применялись по одной, прогонялись
targeted-тесты, затем файл восстанавливался из `/tmp/member_chunk.rs.orig`; после
восстановления — `green-w16-post-mutation.log` (43/43, exit 0).

| Мутация | Что сломано | Поймавший тест | Exit |
|---|---|---|---|
| `complete` | `is_complete()` всегда `true` | `r03_out_of_order_chunks_are_complete_and_ordered_by_index` | 101 |
| `duplicate` | условие duplicate снова только `nonce.is_some()` | `r13_uncorrelated_multi_chunk_duplicate_index_is_diagnostic` | 101 |
| `merge` | существующий member не заменяется, а пушится второй раз | `r10_existing_guild_member_is_updated_not_duplicated` | 101 |

Логи/exit: `mutation-complete.*`, `mutation-duplicate.*`, `mutation-merge.*`.
Все 3 мутации пойманы; продукт восстановлен, md5 совпадает с
`tests-final/SOURCE-MD5SUMS.txt`.
