# TZ W29c — P4-12 Устойчивость: политика reconnect/resume (headless)

Волна: RW-29. Дата: 2026-10-22. Крейт: `discord-voice`. Сеть/live не используются.

## 1. Замороженный контракт

```rust
pub const BACKOFF_MS: [u64; 6] = [1_000, 2_000, 4_000, 8_000, 16_000, 30_000];
pub const INVALID_SESSION_DELAY_MS: u64 = 1_000;

pub enum ReconnectAction { Resume, Identify, GiveUp }
pub struct ReconnectPlan { pub action: ReconnectAction, pub delay_ms: u64, pub attempt: u32 }

pub struct ReconnectPolicy { /* attempt, max_attempts */ }
impl ReconnectPolicy {
    pub fn new(max_attempts: u32) -> Self;       // 0 → 1
    pub fn attempts(&self) -> u32;
    pub fn on_disconnect(&mut self, resumable: bool) -> ReconnectPlan;
    pub fn on_invalid_session(&mut self) -> ReconnectPlan;
    pub fn on_established(&mut self);            // READY/RESUMED: attempt = 0
}
```

Инварианты:

- `on_disconnect(resumable=true)` → `Resume`, задержка `BACKOFF_MS[min(attempt, 5)]`, `attempt += 1`
  (насыщение на `u32::MAX`), при `attempt >= max_attempts` → `GiveUp`, `delay_ms = 0`.
- `on_disconnect(resumable=false)` → `Identify` с `BACKOFF_MS[...]` (новая сессия).
- `on_invalid_session()` (op9) → `Identify` с `INVALID_SESSION_DELAY_MS`, `attempt += 1`;
  лимит попыток тот же, что и у `on_disconnect`.
- `on_established()` сбрасывает `attempt = 0`; сброс после `GiveUp` тоже возвращает политику в работу.
- Никаких паник, `unsafe`, случайности/времени внутри: задержка — чистое значение, применение
  таймера — вне модуля.

Смысл: после разрыва сокета клиент сперва пытается RESUME (сессия и `s` продолжаются), при
INVALID_SESSION — заново IDENTIFY; лимит попыток защищает от бесконечного цикла.

## 2. Доказательная база

- `docs/re/gateway-protocol.md` §4: op7 RECONNECT, 3× RESUME→RESUMED с продолжением `s`
  (95, 664, 332), op9 INVALID_SESSION; задержки в захвате — секунды.
- `docs/rust-client/PLAN.md` P4-12 (устойчивость, авто-rejoin), T-VOICE-20 (L3).
- Ограничение: точные клиентские задержки Discord в захвате не измерены (jitter не наблюдался) —
  наш бэкофф детерминированный и документирован, wire-совместимость не заявляется.

## 3. Кейсы (red-first)

| ID | Проверка | Ожидание |
|---|---|---|
| r01 | первый resumable-разрыв | Resume, 1000 мс, attempt 1 |
| r02 | серия 6 разрывов | задержки 1000/2000/4000/8000/16000/30000 |
| r03 | exceeds max_attempts | GiveUp, delay 0 |
| r04 | `on_invalid_session` | Identify, 1000 мс, attempt+1 |
| r05 | `on_established` после серии | attempt 0, следующая задержка 1000 |
| r06 | resumable=false | Identify с бэкоффом |
| r07 | `new(0)` и насыщение attempt | 1 попытка; нет паники/переполнения |
| r08 | GiveUp → on_established → снова Resume | политика переиспользуема |

Бюджет: 8–10 тестов; мутации 4–6.

## 4. Команды

```sh
cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w29c cargo test --offline -p discord-voice --test reconnect_contract
```

Полный gate — один раз в integration-лейне (BATCH-TZ §4).

## 5. DoD

red-лог с exit code → green → мутации (продукт восстановлен по md5) → отчёт
`waves/RW-29/REPORT-W29c.md` → ревью → accepted.
