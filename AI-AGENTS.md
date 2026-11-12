# 🤖 AI Agents Guide — Chord Project

> **Машиночитаемый протокол для ИИ-агентов.**
> Если ты ИИ-модель — читай этот файл как исполняемую инструкцию.
> Если ты человек — этот файл объясняет как ИИ-агенты работают в проекте.

---

## ЧАСТЬ 1 — ДЛЯ ИИ-АГЕНТОВ (машиночитаемо)

### 1.1 Идентификация

Перед началом работы выполни самоидентификацию:

```
Я: [название модели, версия]
Оператор: [GitHub @username человека]
Задача: [Issue #N или Task-ID]
Scope: [G1/G2/G4/G5/G7 или docs-only]
Начинаю: [timestamp UTC]
```

Эту информацию включи в ПЕРВЫЙ коммит и в описание PR.

### 1.2 Алгоритм запуска (выполни строго по порядку)

```bash
# ШАГ 1: Синхронизация
git fetch upstream
git checkout -b feature/task-<ISSUE_NUMBER>-<slug> upstream/unstable

# ШАГ 2: Прочитать задачу
cat .contracts/<module>_contract.md 2>/dev/null
# Если нет — уточнить у мейнтейнера через комментарий в Issue

# ШАГ 3: Прочитать файловые границы
# Только файлы из своего scope (см. 1.3)
# НИКОГДА не трогать Cargo.lock, TASKS.md, waves/, .claude/ecc2.toml

# ШАГ 4: TDD — сначала тест
# Написать тест → cargo test → убедиться что ПАДАЕТ (Red)
flock /tmp/chord-cargo.lock cargo test --package <pkg> --test <name>_contract 2>&1

# ШАГ 5: Написать минимальный код
# Только то что нужно чтобы тест прошёл

# ШАГ 6: Верификация (обязательно все 3)
cargo fmt --package <pkg> --manifest-path rust/Cargo.toml
flock /tmp/chord-cargo.lock cargo clippy --package <pkg> -- -D warnings
flock /tmp/chord-cargo.lock cargo test --package <pkg>

# ШАГ 7: Проверить файловый scope
git diff --name-only upstream/unstable...HEAD
# Должны быть ТОЛЬКО твои файлы!

# ШАГ 8: Коммит
git commit -m "feat(<scope>): <description> — Task #<N>

- <что сделано пункт 1>
- <что сделано пункт 2>
- Tests: <file> (<N> green, 0 red)
- Gate: fmt ✅ clippy ✅ tests ✅

Task: #<N>
AI-Model: <model name and version>
Operator: @<github username>"

# ШАГ 9: Push
git push origin feature/task-<N>-<slug>

# ШАГ 10: Создать PR (заполнить шаблон полностью)
```

### 1.3 Файловые границы — строго соблюдать

```yaml
scope_map:
  G1_backend:
    allowed:
      - "rust/crates/chord-core/src/**"
      - "rust/crates/chord-codec/src/**"
      - "rust/crates/chord-json/src/**"
      - "rust/crates/chord-store/src/**"
      - "rust/crates/chord-i18n/src/**"
      - "rust/chord-gpui/src/*.rs"
      - "rust/chord-gpui/tests/*_contract.rs"
    forbidden:
      - "rust/chord-gpui/src/views/**"
      - "rust/crates/chord-voice/**"
      - "tests/**"
      - "Cargo.lock"
      - "waves/**"

  G2_frontend:
    allowed:
      - "rust/chord-gpui/src/views/**"
      - "tests/unit/*.test.mjs"
    forbidden:
      - "rust/crates/**"
      - "rust/chord-gpui/src/*.rs"

  G4_qa:
    allowed:
      - "tests/unit/**"
      - "tests/smoke/**"
      - "tests/meta/**"
      - "tests/e2e/**"
      - "scripts/test-impact.mjs"
    forbidden:
      - "rust/crates/**"
      - "rust/chord-gpui/src/**"

  G5_voice:
    allowed:
      - "rust/crates/chord-voice/**"
      - "rust/crates/chord-platform/**"
      - "rust/chord-gpui/src/voice_devices.rs"
    forbidden:
      - "rust/crates/chord-core/**"
      - "rust/chord-gpui/src/views/**"

  G7_re_docs:
    allowed:
      - "docs/re/**"
      - "docs/specs/**"
      - "waves/**/RE-AUDIT/**"
    forbidden:
      - "rust/**"
      - "tests/**"

  docs_only:
    allowed:
      - "**/*.md"
      - "**/*.txt"
    forbidden:
      - "**/*.rs"
      - "**/*.js"
      - "**/*.ts"
      - "Cargo.lock"

  SHARED_coordinator_only:
    files:
      - "Cargo.lock"
      - "TASKS.md"
      - "AGENTS.md"
      - "PROCESS.md"
      - "waves/**"
      - ".claude/ecc2.toml"
    action: "STOP — do not modify, ask coordinator"
```

### 1.4 Правила обработки ошибок CI

```
Если CI FAIL:
  1. Прочитать лог (кнопка Details рядом с красной проверкой)
  2. Идентифицировать тип ошибки:
  
  "error[E]: formatting" → cargo fmt --package <pkg>, push
  "warning[clippy]:"     → исправить каждое, cargo clippy чистый, push
  "FAILED" в тестах      → найти упавший тест, исправить КОД (не тест!), push
  "leak detected"        → gitleaks нашёл секрет, убрать из истории, push
  "target/" в diff       → git rm -r --cached target/, .gitignore, push
  
  3. git add -u && git commit --amend --no-edit && git push --force-with-lease
  4. CI перезапустится автоматически
  
Если ошибка непонятна → оставить комментарий в PR с описанием проблемы
```

### 1.5 Запрещённые паттерны (проверь себя)

```rust
// ❌ Эти паттерны ЗАПРЕЩЕНЫ в production коде:
.unwrap()                    // заменить на ?  или match
.expect("...")               // заменить на Result::map_err
unsafe { ... }               // без Security review
println!("{}", ...)          // только в тестах, не в prod
todo!()                      // не коммитить незаконченный код
unimplemented!()             // не коммитить
#[allow(clippy::...)]        // без обоснования в комментарии
// hardcoded token: "Bot MTk..." // НИКОГДА
```

```bash
# ❌ Эти команды git ЗАПРЕЩЕНЫ:
git push --force             # только --force-with-lease
git push origin main         # только через PR
git push origin unstable     # только через PR
cargo build                  # без flock /tmp/chord-cargo.lock
cargo fmt --all              # только --package <pkg>
```

---

## ЧАСТЬ 2 — ДЛЯ ЛЮДЕЙ-ОПЕРАТОРОВ ИИ

### 2.1 Как запустить ИИ-агента на задачу

```bash
# 1. Выбрать задачу в Issues с меткой 'ai-task'
# 2. Написать своему ИИ-агенту:

"Прочитай файл AI-AGENTS.md в репозитории https://github.com/Userbash/chord
 Возьми задачу #<N>.
 Твой scope: <G1/G2/G4/G5/docs-only>.
 Ты ИИ-модель <название>.
 Я оператор @<твой github>.
 Следуй протоколу AMCP v1.0 из AI-AGENTS.md."

# 3. Следить за PR в GitHub
# 4. Если CI упал — сказать ИИ: "CI упал: <вставить лог>"
# 5. После approve — merge (или попросить ИИ написать /lgtm)
```

### 2.2 Твоя ответственность как оператора

| За что отвечает оператор | За что отвечает ИИ |
|--------------------------|-------------------|
| Выбор правильной задачи | Написание кода |
| Контроль что ИИ не вышел за scope | Написание тестов |
| Финальная проверка кода | Исправление CI ошибок |
| Approve PR (нажать кнопку) | Соблюдение правил |
| Ответственность перед проектом | — |

> ⚠️ **Оператор несёт ответственность за код своего ИИ.** Если ИИ нарушил правила — PR закрывается, оператор получает предупреждение.

### 2.3 Работа двух и более ИИ-агентов

```
Оператор 1 (Vasya с Gemini)     Оператор 2 (Petya с Claude)
────────────────────────────    ─────────────────────────────
Задача #42, scope: G1 backend   Задача #43, scope: G2 frontend
Ветка: feature/task-42-login    Ветка: feature/task-43-modal
Файлы: rust/crates/chord-core/  Файлы: rust/chord-gpui/src/views/

ПРАВИЛО: файлы НЕ пересекаются → конфликтов НЕТ

После PR обоих → review → merge в unstable независимо
```

**Что делать при конфликте файлов:**
```
1. СТОП — оба прекращают работу на конфликтующем файле
2. Написать в Issues: "Conflict: file X needed by tasks #42 and #43"
3. @Userbash принимает решение — кто работает с этим файлом
4. Второй получает другой файл или ждёт
```

---

## ЧАСТЬ 3 — УРОВНИ ДОВЕРИЯ ДЛЯ ИИ-АГЕНТОВ

### 3.1 Прогрессия доверия

```
🔴 L0 — Новый ИИ-агент
   │  Первый PR в проект
   │  Доступно: docs-only, good-first-issue
   │  Нужно: 1 ревью человека
   │  Ограничение: только .md файлы
   ▼
🟡 L1 — Знакомый ИИ-агент  
   │  1+ принятый PR без нарушений
   │  Доступно: P2, P3, тесты
   │  Нужно: 1 ревью
   │  Ограничение: только G4 (тесты) или отдельные модули
   ▼
🟢 L2 — Доверенный ИИ-агент
   │  3+ принятых PR, стабильное поведение
   │  Доступно: P1, P2, рефакторинг
   │  Нужно: 1 ревью
   │  Нет ограничений по scope (кроме shared-файлов)
   ▼
🔵 L3 — Привилегированный ИИ-агент
      5+ PR, подтверждённая надёжность
      Доступно: все задачи кроме P0
      Нужно: 1 ревью
      Shared-файлы всё равно только через координатора
```

### 3.2 Как ИИ теряет доверие (downgrade)

```
-1 уровень:
  - Закоммитил секрет/токен
  - Вышел за файловый scope
  - Изменил тест чтобы скрыть баг
  - Изменил Cargo.lock без разрешения

Бан (отклонение всех PR):
  - Намеренный обход CI
  - Включение проприетарного кода Discord
  - Повторное нарушение после предупреждения
```

---

## ЧАСТЬ 4 — ПОЛНЫЙ ПУТЬ КОДА В ПРОЕКТ

```
                    РАЗРАБОТКА
┌─────────────────────────────────────────────────────┐
│ 1. Issue создан → метка backlog, ai-task, P0-P3     │
│ 2. Оператор берёт задачу → assign                   │
│ 3. ИИ создаёт ветку от unstable                     │
│ 4. TDD: тест → код → тест проходит                  │
│ 5. Локальные проверки: fmt + clippy + test           │
│ 6. PR открыт в unstable                             │
└─────────────────────────────────────────────────────┘
                    ↓
                    АВТОМАТИЧЕСКИЕ ПРОВЕРКИ CI (8 jobs)
┌─────────────────────────────────────────────────────┐
│ L0 Format:    rustfmt --check → 0 diff              │
│ L0 Clippy:    clippy -D warnings → 0 warns          │
│ L0 Tests:     cargo test --workspace → all green    │
│ L0 Harness:   harness compiles                      │
│ L1 FileBounds: только allowed files                 │
│ L2 Contract:  .contracts/ обновлён если надо        │
│ L3 Secrets:   gitleaks → 0 секретов                 │
│ Gate Summary: все выше OK?                          │
└─────────────────────────────────────────────────────┘
                    ↓ все зелёные
                    АВТОМАТИКА
┌─────────────────────────────────────────────────────┐
│ • Auto-label по scope (backend/frontend/voice/...)  │
│ • CODEOWNERS → добавлен ревьюер автоматически       │
│ • PR помечен "ready for review"                     │
└─────────────────────────────────────────────────────┘
                    ↓
                    РЕВЬЮ (человек)
┌─────────────────────────────────────────────────────┐
│ Ревьюер проверяет:                                  │
│   • Логика кода корректна?                          │
│   • Контракт соблюдён?                              │
│   • Нет лишнего кода вне scope?                     │
│   • Тесты проверяют контракт (не реализацию)?       │
│                                                     │
│ Результат:                                          │
│   APPROVE → merge разрешён                          │
│   REQUEST CHANGES → оператор исправляет с ИИ        │
│   BLOCK → обсуждение, новый ADR                     │
└─────────────────────────────────────────────────────┘
                    ↓ APPROVE
                    MERGE
┌─────────────────────────────────────────────────────┐
│ • Squash and merge в unstable                       │
│ • Ветка автоматически удалена                       │
│ • Issue закрыт                                      │
└─────────────────────────────────────────────────────┘
                    ↓ (когда unstable стабильна)
                    ИНТЕГРАЦИЯ
┌─────────────────────────────────────────────────────┐
│ unstable → testing:  QA integration build           │
│ testing → beta:      Pre-release validation         │
│ beta → main:         Stable release + tag v*.*.*    │
└─────────────────────────────────────────────────────┘
```

---

## ЧАСТЬ 5 — МИНИМАЛЬНЫЕ ТРЕБОВАНИЯ

### Для человека БЕЗ ИИ

```
Обязательно:
  ✓ Rust 1.85+
  ✓ Node.js 24+
  ✓ git
  ✓ Linux (Ubuntu 22.04+ / Arch / Fedora)
  ✓ GitHub аккаунт

Знания:
  ✓ Базовый Rust (ownership, lifetimes)
  ✓ Понимание TDD
  ✓ git rebase (не merge!)

Не обязательно:
  ✗ Знание Discord протоколов (есть docs/re/)
  ✗ Опыт GPUI/WGPU (есть примеры)
  ✗ Знание async Rust (можно научиться)
```

### Для человека С ИИ-агентом

```
Обязательно:
  ✓ GitHub аккаунт
  ✓ ИИ-модель с доступом к файловой системе и git
  ✓ Понимание: что делает ИИ и что ты проверяешь

Не обязательно:
  ✗ Знание Rust
  ✗ Знание TDD
  ✗ Опыт разработки
  
Но ты всё равно отвечаешь за код своего ИИ!
```

### Для ИИ-агента

```
Технические требования к окружению:
  ✓ git 2.40+
  ✓ Rust toolchain (cargo, rustfmt, clippy)
  ✓ Node.js 24+
  ✓ flock (coreutils)
  ✓ Доступ к GitHub API (для PR)

Способности модели:
  ✓ Читать и писать Rust код
  ✓ Выполнять команды в терминале
  ✓ Читать ошибки компилятора
  ✓ Следовать инструкциям файла
  ✓ Создавать PR через gh CLI или GitHub API
```

---

## Быстрая справка (для ИИ — скопируй и используй)

```bash
# === SETUP ===
git fetch upstream
git checkout -b feature/task-N-slug upstream/unstable

# === BUILD CHECK ===
flock /tmp/chord-cargo.lock cargo check --workspace --manifest-path rust/Cargo.toml

# === FORMAT ===
cargo fmt --package <pkg> --manifest-path rust/Cargo.toml

# === LINT ===
flock /tmp/chord-cargo.lock cargo clippy --package <pkg> --manifest-path rust/Cargo.toml -- -D warnings

# === TEST ===
flock /tmp/chord-cargo.lock cargo test --package <pkg> --manifest-path rust/Cargo.toml

# === COMMIT ===
git add <only-my-files>
git commit -m "feat(scope): description — Task #N

AI-Model: <model>
Operator: @<username>"

# === PUSH ===
git push origin feature/task-N-slug

# === CHECK SCOPE ===
git diff --name-only upstream/unstable...HEAD

# === HARNESS CHECK ===
flock /tmp/chord-cargo.lock cargo build --manifest-path tests/e2e/harness/Cargo.toml
```

---

*AI-AGENTS.md · Chord Project · AMCP v1.0 · 2026-11-13*  
*Этот файл является частью официальной документации проекта*
