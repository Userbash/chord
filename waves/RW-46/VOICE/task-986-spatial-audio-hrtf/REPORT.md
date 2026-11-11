# Отчёт о выполнении задачи `TASK-986`: Пространственное 3D-позиционирование звука участников (HRTF)

## 1. Карточка и контекст
- **Карточка:** `TASK-986` (`Пространственное 3D-позиционирование звука участников (HRTF)`)
- **Отдел:** `G5 Voice / Platform` | **Приоритет:** `P2` | **Волна:** `RW-46`
- **Ветка:** `feature/sprints-1-5-build-004`
- **Orca Task ID:** `task_af2fafc11d56` | **Dispatch ID:** `ctx_f615c5459510`

## 2. Что сделано
1. **Контрактный и мутационный тест (`tests/unit/spatial-audio-hrtf-986.test.mjs`, 520 строк, 0 ошибок TypeScript):**
   - `SUB-986-0`: проверка наличия `rust/discord-voice/src/spatial_audio.rs`, `#![forbid(unsafe_code)]`, отсутствия `.unwrap()`/`.expect()` в prod-коде, лимитов размера (`checkRustSourceSizeLimits`: `< 50` строк на функцию, `< 800` строк на файл), отсутствия секретов (`scanTextForSecrets`), `rustfmt --check --edition 2024` и чистой компиляции `rustc --test --edition 2024 -C opt-level=2 -D warnings`.
   - `SUB-986-1`: сверка констант бинарника `discord_voice.node` (`size = 126136024`, `md5 = 5fe80ce0f5d89fd53d30b2fb25a6cdac`, `sha256`), встроенного движка Steam Audio (`Phonon`: `default.sofa`, `api_hrtf.cpp`, `api_binaural_effect.cpp`, `distance_attenuation.cpp`, `overlap_save_convolution_effect.cpp`), NAPI-символов (`isSpatialAudioEnabled`, `setSpatialAudioStatusCallback`, `SetSpatialAudioStatusCallback`), дизайн-токенов и состояний `peer-avatar-3d` (`center`, `drag position`, `panned`) с независимыми оракулами `docs/re/native-re.json` и `waves/kanban-board.json`.
   - `SUB-986-2`: сверка расчёта межушной разности времени (`ITD` до `0.7 мс` по сферической модели Вудворта с радиусом головы KEMAR `a = 0.0875 м`, `c = 343.0 м/с`), межушной разности уровня (`ILD` до `20.0 дБ`) и обратно-пропорционального затухания дистанции (`1 / max(r, 1.0)`) с независимым физико-математическим оракулом.
   - `SUB-986-3`: проверка бинауральной HRTF-свёртки (`HrtfBinauralConvolver`: 16-отводный FIR-фильтр + линейно-интерполированная линия задержки ITD + буфер истории Overlap-Save) для моно-кадров Opus (`48 кГц`, `960` сэмплов, `20 мс`) без разрывов на границах кадров.
   - `SUB-986-4`: проверка микширования 10 одновременных участников (`SpatialAudioMixer` + `benchmark_ten_participants_cpu`) с нагрузкой CPU `< 2.0%` от бюджета 20 мс кадра и покрытие всех 9 обязательных измерений `TEST-POLICY.md` (позитивные, негативные, граничные `0`/`u64::MAX`/`2000`/`10`/`25 МБ`/`offset=9975`/`volume=0..200`/`99+`, ошибки REST `401/403/404/413/429/5xx`, пустые/DTX `0xF8 0xFF 0xFE`, прогрев `HTTP 202`, права `VIEW_CHANNEL | CONNECT | SPEAK` и `AGE_GATED_SPACES`, деградация бэкенда `SofaHrtf -> KemarBinaural -> StereoPanning -> MonoBypass` и маскирование секретов `mfa.***`).
   - `SUB-986-5`: мутационный гейт `evaluateMutationGate` (`5/5` убитых мутантов `MUT-spatial-01..05`, `killRate = 1.0`).
2. **Модуль пространственного звука (`rust/discord-voice/src/spatial_audio.rs`, 792 строки):**
   - `#![forbid(unsafe_code)]`, 0 `.unwrap()`/`.expect()` в продуктовом коде, каждая функция `< 50` строк, чистый `rustfmt --edition 2024`.

## 3. Логи прогонов (TDD Red -> Green -> Mutation)
- **RED-прогон (`waves/RW-46/VOICE/task-986-spatial-audio-hrtf/red.log`):**
  - Команда: `node --test tests/unit/spatial-audio-hrtf-986.test.mjs` (до создания `rust/discord-voice/src/spatial_audio.rs`)
  - Результат: `tests 6, pass 0, fail 6` (зафиксировано отсутствие целевого модуля).
- **GREEN-прогон (`waves/RW-46/VOICE/task-986-spatial-audio-hrtf/green.log`):**
  - Команда: `node --test tests/unit/spatial-audio-hrtf-986.test.mjs`
  - Результат: `tests 6, suites 1, pass 6, fail 0` (`1291.66 ms`).
- **Мутационный гейт (`waves/RW-46/VOICE/task-986-spatial-audio-hrtf/mutation.log`):**
  - Команда: `node --test --test-name-pattern="SUB-986-5" tests/unit/spatial-audio-hrtf-986.test.mjs`
  - Результат: `5/5` мутаций убиты (`MUT-spatial-01-itd-sign-inversion`, `MUT-spatial-02-ild-zeroed`, `MUT-spatial-03-distance-attenuation-inverted`, `MUT-spatial-04-cpu-budget-bypass`, `MUT-spatial-05-convolver-delay-tap-dropped`).

## 4. Файлы и контрольные суммы (MD5)
| Файл | Строк | MD5 |
|---|---:|---|
| `rust/discord-voice/src/spatial_audio.rs` | 792 | `404b081ebdb909756cff6e07064e2aa5` |
| `tests/unit/spatial-audio-hrtf-986.test.mjs` | 520 | `32740517b6e1f77153eedfaaa0e6a5d2` |
| `waves/RW-46/VOICE/task-986-spatial-audio-hrtf/red.log` | 150 | `6a6567aef00ee4eef294ef435487268c` |
| `waves/RW-46/VOICE/task-986-spatial-audio-hrtf/green.log` | 16 | `11d361b405afbb460429deafdec7b3f5` |
| `waves/RW-46/VOICE/task-986-spatial-audio-hrtf/mutation.log` | 11 | `b5e85769f944e8b4c64cf9695626996a` |

## 5. Риски и ограничения
- Общие файлы (`TDD-PLAN.md`, `TASKS.md`, `waves/kanban-board.*`, `rust/crates/discord-voice/src/lib.rs`) и файлы смежных задач не затрагивались согласно правилам изоляции воркеров (`AGENTS.md` §1.2).
