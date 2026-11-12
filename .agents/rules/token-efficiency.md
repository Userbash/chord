# Token Efficiency — Правила экономии токенов для воркеров

> OPT-6: Запрет `cat` для больших файлов + объединение вызовов инструментов.
> Нарушение = P1 блокировка на code review.

## Запрещённые паттерны (FORBIDDEN)

```bash
# ЗАПРЕЩЕНО — грузит весь файл в контекст
cat rust/chord-gpui/src/state/mod.rs           # 2500+ строк = ~40K токен
cat tests/e2e/harness/trace_loader.rs
grep -rn "fn submit_login" rust/              # без | head -20 = мусор

# ЗАПРЕЩЕНО — повторный вызов одного инструмента
run_command("grep -n 'fn foo' file.rs")        # первый вызов
run_command("sed -n '100,150p' file.rs")       # второй вызов (можно объединить)
```

## Обязательные паттерны (REQUIRED)

### 1. Читать файл — только целевой диапазон строк
```bash
# ШАГ 1: найти номер строки
grep -n "fn submit_login\|fn connect_voice\|fn start_screen" rust/chord-gpui/src/state/mod.rs

# ШАГ 2: прочитать только нужный диапазон
sed -n '1380,1450p' rust/chord-gpui/src/state/mod.rs
# ИЛИ read_file(StartLine=1380, EndLine=1450)
```

### 2. Grep всегда с ограничением вывода
```bash
grep -rn "pattern" rust/ | head -20
grep -n "fn " file.rs | head -30
```

### 3. Объединяй независимые команды в один вызов
```bash
# ПРАВИЛЬНО — один вызов run_command
grep -n "fn submit_login" state/mod.rs && grep -n "fn connect_voice" state/mod.rs

# ИЛИ используй параллельные tool_calls если независимы
```

### 4. Используй `wc -l` перед `cat`
```bash
wc -l файл.rs
# Если > 100 строк — grep сначала, потом sed -n 'X,Yp'
# Если < 100 строк — можно read_file целиком
```

### 5. Harness smoke — всегда используй summary режим
```bash
# ПРАВИЛЬНО — одна строка, не нужен JSON парсинг
./chord-gpui --smoke=summary
# → SMOKE ok backend=headless guilds=5 ch=23 sqlite=true fps=true tick=420us vmrss=38.2MB

# ИЗБЕГАТЬ — полный JSON для AI-воркеров (только для CI)
./chord-gpui --headless-smoke
```

### 6. Поиск по кодовой базе — использовать codebase_search/understand_search
```
# Вместо grep -rn по всему проекту
codebase_search("submit_login")          # семантический поиск
understand_search("AppState login")     # архитектурный контекст
```

## Пороги файлов

| Строк | Действие |
|-------|---------|
| < 100 | `read_file` целиком ✅ |
| 100-500 | `grep -n` → `sed -n 'X,Yp'` ✅ |
| > 500 | Только целевые диапазоны, обязательно `grep` сначала ✅ |
| > 2000 | Запрос к Team Lead перед чтением ⚠️ |

## Объединение вызовов (call batching)

Инструменты можно вызывать параллельно если нет зависимостей:
```
# В одном turn — параллельно:
read_file("state/mod.rs", StartLine=1380, EndLine=1430)
read_file("dispatch/mod.rs", StartLine=1, EndLine=50)
run_command("cargo check -p chord-gpui 2>&1 | head -20")
```

Не делай цепочку N вызовов там где хватит 1-2.
