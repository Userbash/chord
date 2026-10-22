# RW-17 / P2-07 — mutation evidence (7/7 caught)

Все мутации вносились в рабочее дерево, затем исходники восстановлены. Проверка md5:

| Файл | до | после restore |
|---|---|---|
| `rust/crates/discord-core/src/state/read_state_reducer.rs` | `a18804708a45dccde3f5d2b105840924` | `a18804708a45dccde3f5d2b105840924` |
| `rust/crates/discord-core/src/state/read_state_selectors.rs` | `c050b2ce9ab3683b1f353bd4fd39c8b8` | `c050b2ce9ab3683b1f353bd4fd39c8b8` |

Актуальный набор: 17 reducer + 11 selectors + 12 types = 40 targeted-тестов.

## M1 — manual игнорируется

- Файл: `rust/crates/discord-core/src/state/read_state_reducer.rs`
- Команда: `cd rust && cargo test -p discord-core --test read_state_reducer`
- Итог: **CAUGHT** (exit 101)
- Лог: `waves/RW-17/mutation-logs/M1.txt`

```diff
--- before
+++ after-M1
@@ -80,7 +80,7 @@
     }
     // Non-manual ACK официального клиента очищает mentions; `mention_count` имеет смысл
     // только при manual, отрицательное значение семантически равно 0.
-    entry.mention_count = Some(if ack.manual {
+    entry.mention_count = Some(if false {
         ack.mention_count.unwrap_or(0).max(0)
     } else {
         0
```

Фрагмент результата:
```
test r03_manual_ack_sets_mention_count ... FAILED
test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

## M2 — stale guard инвертирован

- Файл: `rust/crates/discord-core/src/state/read_state_reducer.rs`
- Команда: `cd rust && cargo test -p discord-core --test read_state_reducer`
- Итог: **CAUGHT** (exit 101)
- Лог: `waves/RW-17/mutation-logs/M2.txt`

```diff
--- before
+++ after-M2
@@ -49,7 +49,7 @@
 
     // Локальная защита от регрессии ack-точки; это не заявленная семантика протокола.
     if let Some(current) = state.read_state.entries[index].last_message_id
-        && ack.message_id < current
+        && ack.message_id > current
     {
         state.diagnostics.inconsistencies.push(format!(
             "read_state.ack:{}:stale:{}",
```

Фрагмент результата:
```
test r07_stale_message_id_is_rejected_with_diagnostic ... FAILED
test r15_forward_ack_advances_ack_point ... FAILED
test result: FAILED. 15 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

## M3 — flags/last_viewed не обновляются

- Файл: `rust/crates/discord-core/src/state/read_state_reducer.rs`
- Команда: `cd rust && cargo test -p discord-core --test read_state_reducer`
- Итог: **CAUGHT** (exit 101)
- Лог: `waves/RW-17/mutation-logs/M3.txt`

```diff
--- before
+++ after-M3
@@ -72,12 +72,7 @@
     {
         entry.last_message_id = Some(ack.message_id);
     }
-    if let Some(flags) = ack.flags {
-        entry.flags = Some(flags);
-    }
-    if let Some(last_viewed) = ack.last_viewed {
-        entry.last_viewed = Some(last_viewed);
-    }
+    let _ = (ack.flags, ack.last_viewed);
     // Non-manual ACK официального клиента очищает mentions; `mention_count` имеет смысл
     // только при manual, отрицательное значение семантически равно 0.
     entry.mention_count = Some(if ack.manual {
```

Фрагмент результата:
```
test r01_fixture_ack_updates_channel_entry ... FAILED
test r09_missing_optional_fields_preserve_previous_values ... FAILED
test r02_unknown_channel_creates_entry_at_end ... FAILED
test r15_forward_ack_advances_ack_point ... FAILED
test r08_equal_message_id_updates_metadata_without_duplicate ... FAILED
test r14_ack_does_not_touch_unrelated_entry_fields ... FAILED
test result: FAILED. 11 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

## M4 — guild не учитывает threads

- Файл: `rust/crates/discord-core/src/state/read_state_selectors.rs`
- Команда: `cd rust && cargo test -p discord-core --test read_state_selectors`
- Итог: **CAUGHT** (exit 101)
- Лог: `waves/RW-17/mutation-logs/M4.txt`

```diff
--- before
+++ after-M4
@@ -128,10 +128,7 @@
 
     /// Гильдия записи: сначала каналы, затем треды; `None` для ЛС/неизвестных.
     fn entry_guild(&self, id: u64) -> Option<u64> {
-        self.channel_guild
-            .get(&id)
-            .or_else(|| self.thread_guild.get(&id))
-            .copied()
+        self.channel_guild.get(&id).copied()
     }
 }
 
```

Фрагмент результата:
```
test s03_guild_mentions_include_channels_and_threads_only ... FAILED
test result: FAILED. 10 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
```

## M5 — forward last_message_id не двигается

- Файл: `rust/crates/discord-core/src/state/read_state_reducer.rs`
- Команда: `cd rust && cargo test -p discord-core --test read_state_reducer`
- Итог: **CAUGHT** (exit 101)
- Лог: `waves/RW-17/mutation-logs/M5.txt`

```diff
--- before
+++ after-M5
@@ -68,7 +68,7 @@
 fn apply_fields(entry: &mut ReadStateEntry, ack: &MessageAck) {
     if entry
         .last_message_id
-        .is_none_or(|current| ack.message_id > current)
+        .is_none_or(|_| false)
     {
         entry.last_message_id = Some(ack.message_id);
     }
```

Фрагмент результата:
```
test r15_forward_ack_advances_ack_point ... FAILED
test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

## M6 — version с max-guard (обе ветки)

- Файл: `rust/crates/discord-core/src/state/read_state_reducer.rs`
- Команда: `cd rust && cargo test -p discord-core --test read_state_reducer`
- Итог: **CAUGHT** (exit 101)
- Лог: `waves/RW-17/mutation-logs/M6.txt`

```diff
--- before
+++ after-M6
@@ -30,7 +30,7 @@
         apply_fields(&mut entry, &ack);
         state.read_state.entries.push(entry);
         if let Some(version) = ack.version {
-            state.read_state.version = version;
+            state.read_state.version = state.read_state.version.max(version);
         }
         return;
     };
@@ -60,7 +60,7 @@
 
     apply_fields(&mut state.read_state.entries[index], &ack);
     if let Some(version) = ack.version {
-        state.read_state.version = version;
+        state.read_state.version = state.read_state.version.max(version);
     }
 }
 
```

Фрагмент результата:
```
test r16_version_is_assigned_without_max_guard ... FAILED
test r17_unknown_channel_version_has_no_max_guard ... FAILED
test result: FAILED. 15 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

## M7 — version max-guard только unknown-channel

- Файл: `rust/crates/discord-core/src/state/read_state_reducer.rs`
- Команда: `cd rust && cargo test -p discord-core --test read_state_reducer`
- Итог: **CAUGHT** (exit 101)
- Лог: `waves/RW-17/mutation-logs/M7.txt`

```diff
--- before
+++ after-M7
@@ -30,7 +30,7 @@
         apply_fields(&mut entry, &ack);
         state.read_state.entries.push(entry);
         if let Some(version) = ack.version {
-            state.read_state.version = version;
+            state.read_state.version = state.read_state.version.max(version);
         }
         return;
     };
```

Фрагмент результата:
```
test r17_unknown_channel_version_has_no_max_guard ... FAILED
test result: FAILED. 16 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```
