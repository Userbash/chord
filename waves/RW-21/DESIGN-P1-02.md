# B02 / P1-02 — zstd stream strict (contract)

Дата: 2026-10-21. Карточка **P1-02** (S). Файлы: `discord-codec/src/zstd.rs`,
`tests/zstd_stream.rs`. Только `ruzstd` уже в lock; новых зависимостей нет.

## 1. Цель

Сделать ошибки zstd-потока **терминальными и наблюдаемыми**, убрать двусмысленность
`is_finished()` у свежего потока и закрепить контракт удержания окна. Это закрывает пункты
RW-02 report (`death-state`) без попытки «дочитать» незавершённый кадр: `ruzstd` не может
безопасно отдать последние `window_size` байт, пока следующий блок может на них ссылаться.

## 2. Контракт

```rust
pub enum ZstdError {
    MissingMagic,
    Decode { msg: String },
    TrailingData { bytes: usize },
    Poisoned, // новое: push после любой предыдущей ошибки
}
```

- Любой `Err` (включая `MissingMagic`/`Decode`/`TrailingData`) переводит поток в `dead`;
  следующий `push` любого размера возвращает `Poisoned` и не трогает декодер.
- `is_dead() -> bool`; `is_finished()` у свежего потока — `false`, после завершённого кадра —
  `true`, у непрерывного потока шлюза — `false`.
- Удержание окна не маскируется: `push(lab)` отдаёт 759 280 из 824 816 Б; это документированный
  предел незавершённого кадра, а не ошибка. Тест фиксирует число и объясняет остаток.
- Пустой `push` после завершённого кадра остаётся `Ok(vec![])`; после `Poisoned` — `Poisoned`.

## 3. Тесты

| ID | Проверка |
|---|---|
| z01 | свежий поток `is_finished() == false`, `is_dead() == false` |
| z02 | `MissingMagic` терминален: следующий push → `Poisoned` |
| z03 | `Decode` терминален: следующий push → `Poisoned` |
| z04 | `TrailingData` терминален; байты после завершённого кадра не теряются молча |
| z05 | синтетический кадр: после завершения `is_finished`, пустой push Ok, непустой → TrailingData |
| z06 | lab 1389: 759 280 Б, 104 066 сжатых, `!is_finished`, окно 65 536 объяснено |

## 4. Мутации

M1 убрать `dead=true` в Decode; M2 убрать `Poisoned`-проверку; M3 `is_finished` вернуть
сырой `FrameDecoder::is_finished`; M4 не помечать TrailingData dead; M5 перепутать окно/
`TARGET_CAPACITY`. Все red-first, restore по md5.
