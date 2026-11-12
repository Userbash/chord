## 🤖 AI Worker PR

**Задача:** <!-- P0-X.X | H-FIX-X | ссылка на TASKS.md -->
**Модель:** <!-- Gemini-Argon-High / Claude-3.7 / GPT-4o / DeepSeek -->
**Воркер:** <!-- G1 Backend / G2 Frontend / G4 QA / G5 Voice / G7 RE -->
**Ветка:** `feature/task-<ID>-<slug>` → `unstable`

---

### Что сделано

- 
- 

### Тесты

| Файл контракта | Тестов | Статус |
|----------------|--------|--------|
| `tests/.../foo_contract.rs` | N | ✅ зелёные |

```
# Вставить вывод cargo test:
test result: ok. N passed; 0 failed
```

### Изменённые файлы (только мой scope)

- `src/...` — 

### Файлы НЕ тронуты ✅

- [ ] `Cargo.lock` — не изменён
- [ ] `TASKS.md` — не изменён  
- [ ] `waves/` — не тронуто
- [ ] Чужие scope-файлы — не тронуты

---

### Gate чеклист (до отправки на ревью)

- [ ] `cargo fmt --check` → 0 diff
- [ ] `cargo clippy -- -D warnings` → 0 warnings
- [ ] `cargo test -p <pkg> --test <name>_contract` → зелёный
- [ ] Нет secrets/токенов в коде (`grep -r "token\|password\|secret" src/`)
- [ ] `git diff --name-only` показывает только мои файлы
- [ ] CI все проверки зелёные

---

### Breaking changes

- [ ] Нет breaking changes в публичном API
- [ ] Есть breaking changes: <!-- описать + обновить .contracts/ -->

---

*Protocol: AMCP v1.0 | Auto-checks: AI Collab Gate workflow*
