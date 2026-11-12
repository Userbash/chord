# Cargo Flock — Защита от параллельных сборок

> FIX-22: Обязательное правило для всех воркеров выполняющих `cargo` команды.
> Нарушение = OOM kill (25 GB RAM лимит при 5 параллельных воркерах).

## Правило

**ВСЕГДА** оборачивай `cargo` команды в `flock`:

```bash
# Шаблон (ОБЯЗАТЕЛЬНЫЙ):
flock /tmp/discord-lab-cargo.lock cargo <command> ...

# Примеры:
flock /tmp/discord-lab-cargo.lock cargo build --manifest-path rust/Cargo.toml -p chord-core
flock /tmp/discord-lab-cargo.lock cargo test --manifest-path rust/Cargo.toml -p chord-voice --test voice_contract
flock /tmp/discord-lab-cargo.lock cargo check --manifest-path rust/chord-gpui/Cargo.toml
```

## Почему

- `CARGO_TARGET_DIR = /home/user/orca/project/rust/target` — общий для всех воркеров
- Параллельный `cargo build` на одном `target-dir` → corrupted incremental cache → OOM
- AMD Ryzen 7 7800X3D: 1 cargo build × 8 потоков уже использует ~4-6 GB RAM
- При 5 воркерах без flock → 20-30 GB → OOM kill

## Исключения

- `cargo check` без кодогенерации — можно без flock (только для быстрой проверки типов)
- `cargo fmt` — всегда без flock (только форматирование)
- `cargo clippy --no-deps` — без flock (статический анализ без линковки)

## RAM мониторинг перед запуском cargo

```bash
CURRENT_RSS=$(ps -eo rss | awk '{s+=$1} END {printf "%.1f", s/1024/1024}')
echo "Текущее RSS: ${CURRENT_RSS} GiB"
# Если > 20 GiB — подожди освобождения слота перед запуском cargo
```
