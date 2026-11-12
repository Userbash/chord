# 🤝 Contributing to Chord

> **Это руководство для людей и ИИ-агентов.**
> Раздел [для ИИ-агентов](#-для-ии-агентов-ai-agents) содержит машиночитаемые правила.

<p align="center">
  <a href="#-быстрый-старт">Быстрый старт</a> ·
  <a href="#-уровни-доверия">Уровни доверия</a> ·
  <a href="#-для-ии-агентов-ai-agents">Для ИИ-агентов</a> ·
  <a href="#-git-протокол">Git-протокол</a> ·
  <a href="#-проверки-ci">Проверки CI</a> ·
  <a href="#-ревью-и-merge">Ревью и Merge</a>
</p>

---

## 🚀 Быстрый старт

### Шаг 1 — Fork и клон

```bash
# 1. Fork на GitHub (кнопка Fork в правом верхнем углу)
# 2. Клонировать свой fork
git clone https://github.com/YOUR_USERNAME/chord.git
cd chord

# 3. Добавить upstream
git remote add upstream https://github.com/Userbash/chord.git
```

### Шаг 2 — Настройка окружения

```bash
# Rust 1.85+ обязателен
rustup default stable
rustup component add rustfmt clippy

# Node.js 24+ обязателен
node --version  # >= 24.0.0

# Системные библиотеки (Linux)
sudo apt install libasound2-dev libpipewire-0.3-dev libsecret-1-dev libenchant-2-dev

# Установка npm зависимостей
npm ci
```

### Шаг 3 — Убедиться что всё работает

```bash
# Rust workspace компилируется
cd rust && cargo check --workspace && cd ..

# Тесты зелёные
cargo test --workspace --manifest-path rust/Cargo.toml

# Node.js тесты
npm run test:meta
```

### Шаг 4 — Найти задачу

1. Открыть [Issues](https://github.com/Userbash/chord/issues) → фильтр `backlog`
2. Выбрать задачу с меткой по своему уровню (см. [Уровни доверия](#-уровни-доверия))
3. Написать комментарий: **«Беру эту задачу»**
4. Дождаться assign от мейнтейнера

### Шаг 5 — Работа и PR

```bash
# Синхронизировать с upstream
git fetch upstream
git checkout -b feature/task-<ID>-<short-slug> upstream/unstable

# ... работаем, пишем тесты, пишем код ...

# Локальные проверки ПЕРЕД push
cargo fmt --check --manifest-path rust/Cargo.toml
cargo clippy --workspace --manifest-path rust/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path rust/Cargo.toml

# Push и PR
git push origin feature/task-<ID>-<short-slug>
# Открыть PR в GitHub → target: unstable
```

---

## 🏅 Уровни доверия

> Уровень определяет какие задачи доступны и сколько ревьюеров нужно

| Уровень | Кто | Доступные задачи | Нужно ревьюеров |
|---------|-----|-----------------|-----------------|
| **L0 — Новичок** | Первый PR в проект | `docs-only`, `good-first-issue` | 1 |
| **L1 — Контрибьютор** | 1+ принятый PR | `P2`, `P3`, тесты, QA | 1 |
| **L2 — Доверенный** | 5+ принятых PR, хорошие ревью | `P1`, рефакторинг | 1 |
| **L3 — Мейнтейнер** | Invite от владельца | `P0`, shared-файлы, архитектура | 2 |
| **🤖 ИИ-агент (External)** | Fork + PR | `P2`, `P3`, `docs-only` | 1 (обязательно человек) |
| **🤖 ИИ-агент (Trusted)** | Прошёл 3+ ревью | `P1`, `P2` | 1 |

### Как повысить уровень

```
L0 → L1:  Один принятый PR с зелёными тестами
L1 → L2:  5 принятых PR + 3 качественных ревью чужого кода
L2 → L3:  Invite от @Userbash (не запрашивать вручную)
```

---

## 🤖 Для ИИ-агентов (AI Agents)

> Это секция написана специально для ИИ-моделей.
> Читай её как инструкцию к исполнению.

### Идентификация ИИ-агента в PR

В описании PR обязательно укажи:

```markdown
**AI-Agent:** true
**Model:** <Gemini-2.5-Pro / Claude-3.7-Sonnet / GPT-4o / DeepSeek-V3 / ...>
**Operator:** <@github-username человека который тебя запустил>
**Task-ID:** <номер Issue или P0-X.X>
```

### Правила для ИИ-агента — ОБЯЗАТЕЛЬНЫЕ

```
✅ ДЕЛАЙ:
  1. Читай этот файл целиком перед началом работы
  2. Создавай ветку от upstream/unstable (не от main!)
  3. Пиши тест ПЕРВЫМ — до кода (TDD: Red → Green)
  4. Трогай ТОЛЬКО файлы из своего scope (см. Файловые границы)
  5. Запускай cargo fmt + clippy + test локально перед push
  6. Указывай модель и оператора в описании PR
  7. Используй flock перед cargo: flock /tmp/chord-cargo.lock cargo <cmd>
  8. Читай ошибки CI и исправляй — не игнорируй

❌ НИКОГДА:
  1. Не трогай Cargo.lock (только через отдельный PR координатора)
  2. Не коммить target/, node_modules/, .env, *.key, *.pem
  3. Не делай git push --force (только --force-with-lease)
  4. Не делай cargo fmt на весь workspace (только --package <pkg>)
  5. Не пиши unsafe{} без Security review
  6. Не угадывай публичные API — читай .contracts/ или спрашивай
  7. Не изменяй тесты чтобы они прошли под баг
  8. Не делай PR в main/beta/testing напрямую
```

### Файловые границы (scope)

ИИ-агент работает ТОЛЬКО в своём scope. Перед началом — проверь issue на метку:

| Метка Issue | Твой scope | Запрещено трогать |
|-------------|-----------|------------------|
| `scope:backend` | `rust/crates/chord-core/`, `rust/crates/chord-codec/`, `rust/chord-gpui/src/*.rs` | Всё остальное |
| `scope:frontend` | `rust/chord-gpui/src/views/` | Backend, Voice |
| `scope:voice` | `rust/crates/chord-voice/`, `rust/crates/chord-platform/` | Backend, Frontend |
| `scope:tests` | `tests/unit/`, `tests/smoke/`, `tests/meta/` | Production код |
| `scope:harness` | `tests/e2e/harness/` | Production код |
| `scope:re` | `docs/re/`, `docs/specs/` | Production код |
| `docs-only` | `**/*.md` | Любой код |

### Контракт — читай перед кодированием

```bash
# Перед написанием кода прочитай контракт модуля:
cat .contracts/<module>_contract.md 2>/dev/null || echo "Нет контракта — уточни у мейнтейнера"

# Если контракта нет — напиши его в PR-описании:
# "Предлагаемый контракт: вход X → выход Y → HTTP Z"
```

### Шаблон commit message для ИИ

```
feat(auth): implement LoginService — Task #42

- POST /api/v9/auth/login via ApiClient
- TokenStore saves to SecretService D-Bus keyring  
- Tests: tests/unit/auth_contract.rs (5 green, 0 red)
- Gate: fmt ✅ clippy ✅ tests ✅

Task: #42
AI-Model: Gemini-2.5-Pro
Operator: @vasya
```

### Алгоритм работы ИИ-агента (пошагово)

```
START
  │
  ▼
1. git fetch upstream && git checkout -b feature/task-<N>-<slug> upstream/unstable
  │
  ▼
2. Прочитать Issue #N — понять задачу, scope, контракт
  │
  ▼
3. Написать ТЕСТ (Red) → cargo test → убедиться что ПАДАЕТ
  │
  ▼
4. Написать минимальный КОД → cargo test → убедиться что ПРОХОДИТ
  │
  ▼
5. cargo fmt --package <pkg>
   flock /tmp/chord-cargo.lock cargo clippy --package <pkg> -- -D warnings
   flock /tmp/chord-cargo.lock cargo test --package <pkg>
  │
  ▼
6. git diff --name-only → убедиться что только мои файлы
  │
  ▼
7. git commit -m "feat(...): ... — Task #N\n\nAI-Model: ...\nOperator: ..."
  │
  ▼
8. git push origin feature/task-<N>-<slug>
  │
  ▼
9. Открыть PR в GitHub → target: unstable
   Заполнить шаблон PR (все поля обязательны!)
  │
  ▼
10. Дождаться CI → если ❌: прочитать лог, исправить, push
    Дождаться Review → если REQUEST CHANGES: исправить
    После APPROVE → merge (автоматически или мейнтейнером)
  │
  ▼
END
```

---

## 📋 Git-протокол

### Иерархия веток

```
main          ← только стабильные релизы (теги v*.*.*)
  └─ beta     ← pre-release валидация
       └─ testing  ← интеграция QA
            └─ unstable  ← принятые PR (сюда делаешь PR!)
                 └─ feature/task-<ID>-<slug>  ← твоя ветка
                 └─ bugfix/task-<ID>-<slug>
                 └─ docs/task-<ID>-<slug>
```

> **Важно:** Всегда создавай ветку от `upstream/unstable`, не от `main`!

### Naming convention

```bash
feature/task-42-login-service      # новая функция
bugfix/task-43-token-store-crash   # исправление бага
docs/task-44-update-readme         # только документация
test/task-45-auth-contract         # только тесты
refactor/task-46-split-state       # рефакторинг
chore/task-47-update-deps          # зависимости
```

### Синхронизация с upstream (делать регулярно)

```bash
git fetch upstream
git rebase upstream/unstable
# Если конфликты — разрешить вручную, затем:
git add <resolved-files>
git rebase --continue
git push --force-with-lease origin feature/my-branch
```

> ⚠️ Используй **rebase**, не merge. Это сохраняет чистую историю.

---

## 🔍 Проверки CI (Gate)

При создании PR автоматически запускаются **8 параллельных проверок**:

```
┌─ L0 · Format ────── cargo fmt --check
│                     Ноль изменений = OK
│
├─ L0 · Clippy ────── cargo clippy -- -D warnings  
│                     Ноль предупреждений = OK
│
├─ L0 · Tests ─────── cargo test --workspace
│                     Все тесты зелёные = OK
│
├─ L0 · Harness ───── cargo build (tests/e2e/harness)
│                     Компилируется = OK
│
├─ L1 · FileBounds ── target/ не закоммичен
│                     Shared-файлы помечены для ревью
│
├─ L2 · Contract ──── Production код изменён?
│                     Предупреждение если .contracts/ не обновлён
│
├─ L3 · Secrets ───── gitleaks scan
│                     Ноль секретов = OK
│
└─ ✅ Gate Summary ── Все выше OK = PR готов к ревью
                      Любой FAIL = merge заблокирован
```

### Если CI упал — что делать

```bash
# 1. Кликни Details рядом с упавшей проверкой
# 2. Прочитай лог ошибки

# L0 Format упал:
cargo fmt --manifest-path rust/Cargo.toml
git add -u && git commit --amend --no-edit && git push --force-with-lease

# L0 Clippy упал:
cargo clippy --workspace --manifest-path rust/Cargo.toml -- -D warnings
# Исправить каждое предупреждение, затем commit + push

# L0 Tests упал:
cargo test --workspace --manifest-path rust/Cargo.toml 2>&1 | grep "FAILED"
# Найти упавший тест, исправить код (не тест!), commit + push

# L3 Secrets упал:
# НЕМЕДЛЕННО: убрать секрет из истории git
git rebase -i HEAD~N  # найти коммит с секретом
# Или: git filter-repo / BFG Repo Cleaner
```

---

## 👀 Ревью и Merge

### Кто делает ревью

```
Файлы G1 (backend)   → @Userbash (автоматически через CODEOWNERS)
Файлы G2 (frontend)  → @Userbash
Файлы G5 (voice)     → @Userbash
Shared-файлы         → @Userbash (только он)
Docs-only            → Любой L2+, или автоматически при зелёном CI
```

> **Правило:** Автор PR не может ревьюить свой же PR.  
> **Для ИИ-агентов:** Оператор ИИ не может ревьюить PR своего ИИ.

### Статусы ревью

| Статус | Значение | Действие |
|--------|----------|----------|
| ✅ **APPROVE** | Всё хорошо | Merge разрешён |
| 💬 **REQUEST CHANGES** | Нужны исправления | Исправить и push |
| ❌ **BLOCK** | Критическая проблема | Обсудить в комментах |

### После APPROVE

```bash
# Мейнтейнер нажимает "Squash and merge"
# GitHub автоматически:
#   1. Делает squash всех коммитов в один
#   2. Merge в unstable
#   3. Удаляет твою ветку
#   4. Закрывает связанный Issue
```

---

## 📐 Стандарты кода

### Rust

```rust
// ✅ Хорошо
pub async fn submit_login(email: &str, password: &str) -> Result<AuthToken, LoginError> {
    validate_email(email)?;
    let response = api_client.post("/auth/login", &payload).await?;
    AuthToken::from_response(response)
}

// ❌ Плохо  
pub fn login(e: &str, p: &str) -> AuthToken {
    let r = client.post(e, p).unwrap();  // unwrap запрещён!
    r.token                               // нет обработки ошибок
}
```

**Требования:**
- `#![forbid(unsafe_code)]` во всех крейтах (кроме C FFI)
- Нет `.unwrap()` / `.expect()` в production коде
- Функции < 50 строк
- Файлы < 800 строк (soft limit)
- `cargo fmt` перед каждым коммитом
- Нуль clippy предупреждений

### Тесты — TDD обязателен

```rust
// 1. СНАЧАЛА тест (Red phase)
#[tokio::test]
async fn test_login_returns_token_on_success() {
    // Arrange
    let mock_server = MockGateway::start().await;
    mock_server.expect_post("/auth/login").respond_with(200, TOKEN_FIXTURE);
    
    // Act
    let result = LoginService::submit("user@test.com", "pass").await;
    
    // Assert — проверяем КОНТРАКТ, не реализацию
    assert!(matches!(result, Ok(AuthToken { .. })));
}

// 2. ПОТОМ минимальный код (Green phase)
```

**Минимальные пороги (CI блокирует если ниже):**
```
Покрытие строк:    ≥ 90%
Покрытие веток:    ≥ 90%  
Покрытие функций:  ≥ 80%
Mutation score:    5/5 (100%)
Flaky tests:       0%
```

---

## 🔐 Безопасность

### Clean-Room методология (ОБЯЗАТЕЛЬНА)

```
✅ МОЖНО:
  - Анализировать публичные RFC и спецификации
  - Использовать сетевые трейсы (Wireshark, mitmproxy)
  - Изучать открытую документацию API
  - Смотреть на open-source реализации других клиентов

❌ НЕЛЬЗЯ НИКОГДА:
  - Копировать код из проприетарных клиентов
  - Включать Discord токены пользователей
  - Использовать приватные данные пользователей
  - Включать результаты декомпиляции проприетарного ПО
```

### Если нашёл уязвимость

**Не создавай публичный Issue!**

Напиши напрямую: создай приватный [Security Advisory](https://github.com/Userbash/chord/security/advisories/new)

---

## ❓ FAQ

**Q: Можно ли использовать несколько ИИ-агентов одновременно на одной задаче?**  
A: Нет. Одна задача = один воркер = один PR. Параллельно можно работать только над разными задачами с непересекающимися файлами.

**Q: Мой ИИ изменил Cargo.lock — это проблема?**  
A: Да. Cargo.lock — shared-файл. Убери изменение: `git checkout upstream/unstable -- Cargo.lock`

**Q: CI упал на secrets — но я не добавлял секретов**  
A: gitleaks мог найти тестовый fixture или placeholder. Проверь: `gitleaks detect --source . --verbose`

**Q: Можно ли делать PR напрямую в main?**  
A: Нет. Прямой push в main/beta/testing/unstable технически заблокирован. Только через PR в unstable.

**Q: Нужен ли человек-оператор для ИИ-агента?**  
A: Да. Каждый ИИ-PR должен иметь поле `Operator: @username`. Оператор несёт ответственность за PR.

**Q: Как долго ждать ревью?**  
A: Обычно 24-72 часа. Если дольше — добавь комментарий `@Userbash ping review`.

---

## 📞 Контакты

- **GitHub Issues:** https://github.com/Userbash/chord/issues
- **Discussions:** https://github.com/Userbash/chord/discussions
- **Security:** GitHub Security Advisories (приватно)

---

*Chord CONTRIBUTING.md · AMCP v1.0 · Обновлено 2026-11-13*
