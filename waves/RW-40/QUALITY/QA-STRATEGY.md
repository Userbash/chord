# QA-STRATEGY — меньше повторных прогонов, сильнее проверки

Дата: 2026-10-24. PM. Дополняет `TEST-POLICY.md`, `PROCESS.md`, `QA-AUDIT.md`.

## 1. Роль QA

QA отвечает за **архитектуру проверок**, а не за повторение тестов авторов:

- test pyramid и impact selection;
- contract/e2e/mutation;
- bug regression ledger;
- gate/coverage/flake;
- независимая приёмка evidence.

QA не переписывает продуктовую логику и не «зеленит» тесты.

## 2. Уровни и когда что запускать

| Уровень | Кто | Когда | Что |
|---|---|---|---|
| **T0 targeted** | автор | каждый red/green | только файл/модуль + прямой контракт |
| **T1 batch** | G4/Integration | один раз на batch (3–6 карточек) | affected contracts + full frontend + `cargo test` gui + workspace + `gate:worker` |
| **T2 release** | PM/G4 | сборка волны | full unit/meta/smoke/e2e/coverage/keyring/golden |
| **T3 live** | PM+G4/G1 | окно | READY/send/voice/keyring locked, e2e browser |

Нельзя запускать T2 на каждую карточку; T0 не заменяет T1/T2.

## 3. Test impact selector

Скрипт: `node scripts/test-impact.mjs --changed <files...>` (или без args — `git status`).
Он **не запускает** тесты, а выдаёт минимальный набор команд и причину; QA/воркер
исполняет. Full suite — только `--full`/T2.

Правила:
- UI feature → профильный test + shared DOM/regression, не все 588;
- `ui/app.js`/`tokens.css` → все frontend + geometry;
- `src/**` → CLI/CDP/walk/meta;
- Rust module → crate/contract tests + workspace только на batch;
- docs → docs-sync/plan-sync; tests → plan-sync/test-policy.

## 4. E2E — только критические потоки

Не дублировать unit. E2E (fixture/headless + browser CI):

1. login → ready → rail.
2. open channel → send/edit/delete/reply.
3. search remote error → no fallback.
4. join invite → preview → accept → rail.
5. settings → density/theme → reload persistence.
6. reconnect/offline banner → retry.
7. voice panel disabled/idle (live only for audio).

Правила:
- e2e тест имеет `test id` и один пользовательский сценарий;
- fixture-версия без live; live — opt-in окно;
- skip только при отсутствии среды и виден в gate; `skip ≠ pass`;
- 4 текущих e2e — заменить/дополнить сценарными, не плодить дубли.

## 5. Внутренние тесты

- unit — чистые функции/парсеры/редьюсеры, быстрые;
- contract — IPC/REST/body/error, fake transport;
- UI DOM — состояния/aria/rollback, не скриншоты;
- mutation — 1–2 критичных на batch, ротация;
- coverage — release/qualify; для карточки — diff coverage (не полный прогон);
- source-scrape запрещён как единственный oracle; поведение обязательно.

## 6. Устранение дублей

| Дубль | Действие |
|---|---|
| help/version в unit и smoke | оставить smoke для бинарника, unit — только routing; один слой |
| «Frida unavailable» ×N | table-driven один кейс |
| full suite на каждой карточке | T0/T1 |
| coverage 99% на каждой карточке | release/diff |
| e2e skip как pass | gate warning/fail; browser CI |
| token source-scrape | behavior test keyring→Storage/Network |

## 7. Bug/regression

Каждый баг: `BUGS.md` + **один** regression test, привязанный к ID.
Повторный тест того же поведения — удалить/объединить после проверки, не теряя assert.
Mutation: фиксировать, какой тест убил мутанта; если никто — дыра.

## 8. Gate-политика

- `gate:worker`: T1, один раз на batch; не каждым автором.
- `gate:release`: T2; полный набор; e2e/browser и keyring-live — среде-зависимые,
  skip виден; live-подтверждение отдельно.
- `gate:qualify`: release + live keyring/qualification.
- Повтор e2e — 1 раз (flake policy), оба результата сохраняются.
- Coverage/keyring/live не ослаблять; исключения — только письменный waiver.

## 9. Метрики QA

- duplicate runs / batch;
- first-pass gate;
- e2e pass vs skip;
- mutation kill rate;
- regression test per bug;
- cycle time red→accepted.

Цель: меньше повторных прогонов при **большем** покрытии рисков, а не меньше тестов.
