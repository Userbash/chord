# TASK-299 (CORE-FIX-01) — Мост входящих событий `STREAM_*` в `rust/crates/discord-core/src/dispatch.rs` (`STUB-27-CORE-DISPATCH-INTEGRATION`, `L1 / P0`)

- **Отдел:** `G1 Backend (Rust Core & Tauri BFF)`
- **Уровень сложности / Модель:** `L1 (1 файл по готовому тесту dispatch_types.rs)` → `Antigravity Flash Low` (`--agent antigravity --model gemini-3.8-flash-low --effort low`) или `DeepSeek` (`deepseek/deepseek-v4.1-flash`)
- **Приоритет:** `P0 (Gate Blocker: cargo test -p discord-core --test dispatch_types)`
- **Ветка:** `bugfix/task-299-core-stream-dispatch-bridge` (от `unstable`)

---

## 1. Строгий Write-Scope (ровно 1 файл)

- `rust/crates/discord-core/src/dispatch.rs`

Никакие другие файлы менять нельзя.

---

## 2. Точный контракт (проверяется существующим тестом `rust/crates/discord-core/tests/dispatch_types.rs`)

В `rust/crates/discord-core/src/dispatch.rs` необходимо добавить мост к `crate::stream::{StreamAction, StreamDiagnostics, StreamDispatch, StreamDispatchError, parse_dispatch, parse_dispatch_with_diagnostics}`:

1. В `impl DispatchKind`:
   ```rust
   /// 4 серверных события Go Live / Stream (`STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_UPDATE`, `STREAM_DELETE`).
   pub const STREAM_NAMES: [&'static str; 4] = [
       "STREAM_CREATE",
       "STREAM_SERVER_UPDATE",
       "STREAM_UPDATE",
       "STREAM_DELETE",
   ];

   /// Проверяет, входит ли имя события в [`Self::STREAM_NAMES`].
   #[must_use]
   pub fn is_stream_name(name: &str) -> bool {
       StreamDispatchKind::from_name(name).is_some()
   }
   ```
2. Перечисление `StreamDispatchKind`:
   ```rust
   /// 4 входящих (server -> client) события Go Live / Stream (`op == 0`).
   #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
   pub enum StreamDispatchKind {
       /// `STREAM_CREATE`.
       StreamCreate,
       /// `STREAM_SERVER_UPDATE`.
       StreamServerUpdate,
       /// `STREAM_UPDATE`.
       StreamUpdate,
       /// `STREAM_DELETE`.
       StreamDelete,
   }

   impl StreamDispatchKind {
       /// Все 4 серверных типа событий стрима в каноническом порядке.
       pub const ALL: [StreamDispatchKind; 4] = [
           Self::StreamCreate,
           Self::StreamServerUpdate,
           Self::StreamUpdate,
           Self::StreamDelete,
       ];

       /// Точное регистрозависимое сопоставление имени провода `t`.
       #[must_use]
       pub fn from_name(name: &str) -> Option<Self> {
           match name {
               "STREAM_CREATE" => Some(Self::StreamCreate),
               "STREAM_SERVER_UPDATE" => Some(Self::StreamServerUpdate),
               "STREAM_UPDATE" => Some(Self::StreamUpdate),
               "STREAM_DELETE" => Some(Self::StreamDelete),
               _ => None,
           }
       }

       /// Каноническое имя события в проводе.
       #[must_use]
       pub fn as_str(self) -> &'static str {
           match self {
               Self::StreamCreate => "STREAM_CREATE",
               Self::StreamServerUpdate => "STREAM_SERVER_UPDATE",
               Self::StreamUpdate => "STREAM_UPDATE",
               Self::StreamDelete => "STREAM_DELETE",
           }
       }
   }
   ```
3. Структура `RoutedStreamDispatch`:
   ```rust
   /// Маршрутизированное событие стрима с сохранением `seq` (`s`).
   #[derive(Debug, Clone, PartialEq, Eq)]
   pub struct RoutedStreamDispatch {
       /// Тип события стрима (`STREAM_CREATE`..`STREAM_DELETE`).
       pub kind: StreamDispatchKind,
       /// Номер последовательности `s` из op0-конверта.
       pub seq: Option<u64>,
       /// Разобранная полезная нагрузка [`crate::stream::StreamDispatch`].
       pub dispatch: crate::stream::StreamDispatch,
   }

   impl RoutedStreamDispatch {
       /// Преобразовать в действие редьюсера [`crate::stream::StreamAction::Dispatch`].
       #[must_use]
       pub fn into_action(self) -> crate::stream::StreamAction {
           crate::stream::StreamAction::Dispatch {
               seq: self.seq,
               event: self.dispatch,
           }
       }
   }
   ```
4. В `impl DispatchEvent` (строки ~489–515):
   - Обновить `pub fn is_known(&self) -> bool` так, чтобы возвращало `self.kind.is_some() || self.is_extended() || self.is_stream()`.
   - Добавить методы:
     ```rust
     /// Если `name` относится к 4 событиям стрима (`STREAM_*`), возвращает [`StreamDispatchKind`].
     #[must_use]
     pub fn stream_kind(&self) -> Option<StreamDispatchKind> {
         StreamDispatchKind::from_name(&self.name)
     }

     /// Относится ли событие к 4 событиям стрима (`STREAM_CREATE`..`STREAM_DELETE`).
     #[must_use]
     pub fn is_stream(&self) -> bool {
         self.stream_kind().is_some()
     }

     /// Разобрать событие стрима через [`crate::stream::parse_dispatch`].
     #[must_use]
     pub fn parse_stream(
         &self,
     ) -> Option<Result<crate::stream::StreamDispatch, crate::stream::StreamDispatchError>> {
         crate::stream::parse_dispatch(&self.name, &self.data)
     }

     /// Разобрать событие стрима с накоплением мягких/жёстких диагностик в `diag`.
     pub fn parse_stream_with_diagnostics(
         &self,
         diag: &mut crate::stream::StreamDiagnostics,
     ) -> Option<Result<crate::stream::StreamDispatch, crate::stream::StreamDispatchError>> {
         crate::stream::parse_dispatch_with_diagnostics(&self.name, &self.data, diag)
     }

     /// Преобразовать событие в [`crate::stream::StreamAction::Dispatch`] (с сохранением `self.seq`).
     #[must_use]
     pub fn to_stream_action(
         &self,
     ) -> Option<Result<crate::stream::StreamAction, crate::stream::StreamDispatchError>> {
         self.parse_stream().map(|res| {
             res.map(|event| crate::stream::StreamAction::Dispatch {
                 seq: self.seq,
                 event,
             })
         })
     }
     ```
5. Функции верхнего уровня в `rust/crates/discord-core/src/dispatch.rs`:
   ```rust
   /// Маршрутизировать `op == 0` ETF-кадр в [`RoutedStreamDispatch`], если `t` входит в [`StreamDispatchKind::ALL`].
   #[must_use]
   pub fn route_stream_dispatch(
       value: &EtfValue,
   ) -> Option<Result<RoutedStreamDispatch, crate::stream::StreamDispatchError>> {
       let event = route_dispatch(value)?;
       let kind = event.stream_kind()?;
       let parsed = event.parse_stream()?;
       Some(parsed.map(|dispatch| RoutedStreamDispatch {
           kind,
           seq: event.seq,
           dispatch,
       }))
   }

   /// Маршрутизировать `op == 0` ETF-кадр в [`RoutedStreamDispatch`] с записью диагностик в `diag`.
   pub fn route_stream_dispatch_with_diagnostics(
       value: &EtfValue,
       diag: &mut crate::stream::StreamDiagnostics,
   ) -> Option<Result<RoutedStreamDispatch, crate::stream::StreamDispatchError>> {
       let event = route_dispatch(value)?;
       let kind = event.stream_kind()?;
       let parsed = event.parse_stream_with_diagnostics(diag)?;
       Some(parsed.map(|dispatch| RoutedStreamDispatch {
           kind,
           seq: event.seq,
           dispatch,
       }))
   }
   ```

---

## 3. Команды проверки

Используй общий `CARGO_TARGET_DIR=/home/user/orca/project/rust/target`:
```bash
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test -p discord-core --test dispatch_types
CARGO_TARGET_DIR=/home/user/orca/project/rust/target cargo test -p discord-core --lib
```
Запиши краткий отчёт в `/home/user/orca/project/waves/RW-40/G1/task-299-core-stream-dispatch-bridge/REPORT.md`.
