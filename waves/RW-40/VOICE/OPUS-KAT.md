# G5-OPUS-KAT — конкретный план libopus KAT/FFI до live

Дата: 2026-10-24. Owner: G5 Voice/Platform. HEAD: `0239543` (дерево грязное: чужие правки
RW-35/36 и `rust/gui-spike/**` не трогались).
Карточка: `G5-OPUS-KAT` (продолжение `VOICE/G5-PREREQ.md`, `QUALITY/G5-VOICE-PLAN.md` §4).
Режим: **только чтение**. Код, тесты, фикстуры, `Cargo.*`, `scripts/gate.mjs`, системные пакеты
не менялись; `cargo`, live, сеть, аудиоустройства и `ffmpeg`-кодирование не запускались.
Выполнялись только чтение файлов и read-only запросы (`pkg-config --modversion`,
`grep` по `/usr/include`, `ffmpeg -h encoder|decoder=opus`, `ls /etc/alsa/conf.d`,
`getfacl`, `id -nG`, `pgrep`).

Это **план**, а не отчёт: он описывает, что именно будет написано, чем проверено и что остаётся
за live. Изменён только этот файл.

## 0. Легенда

| Метка | Значение |
|---|---|
| `[H]` | проверено на этом хосте read-only командой (см. §1) |
| `[L]` | подтверждено живым захватом (`docs/re/voice-deep.md`) |
| `[S]` | подтверждено протоколом/кодом клиента |
| `[A]` | наше решение, требует подтверждения PM |
| `[P]` | предложение к общим файлам (правит Team Lead) |

## 1. Факты хоста, на которых стоит план

| # | Факт | Значение | Метка |
|---|---|---|---|
| H-1 | `pkg-config --modversion opus` | `1.6.1`; `libopus.so`, `libopus.so.0` в `ldconfig` | `[H]` |
| H-2 | Прототипы | `opus_encoder_create(opus_int32,int,int,int*)`, `opus_encode(*mut, *const opus_int16, int, *mut u8, opus_int32)`, `opus_decoder_create/decode`, `opus_encoder_ctl`/`opus_decoder_ctl` **вариадические** (`opus.h:211,266,367,477,516,586`) | `[H]` |
| H-3 | `opus_strerror`, `opus_get_version_string` | объявлены в `opus_defines.h:850,860` (не в `opus.h`) | `[H]` |
| H-4 | Коды ошибок | `OPUS_OK 0`, `BAD_ARG -1`, `BUFFER_TOO_SMALL -2`, `INTERNAL_ERROR -3`, `INVALID_PACKET -4`, `UNIMPLEMENTED -5`, `INVALID_STATE -6`, `ALLOC_FAIL -7` | `[H]` |
| H-5 | `OPUS_GET_LOOKAHEAD_REQUEST 4027`, `OPUS_SET_COMPLEXITY_REQUEST 4010`, `OPUS_SET_DTX_REQUEST 4016`, `OPUS_GET_DTX 4017` | значения фиксированы заголовком | `[H]` |
| H-6 | Пакетные интроспекторы | `opus_packet_get_nb_frames(pkt,len)`, `opus_packet_get_nb_samples(pkt,len,Fs)`, `opus_packet_get_nb_channels(pkt)` (**без len**), `opus_packet_get_bandwidth(pkt)` | `[H]` |
| H-7 | ffmpeg | `n9.0.2`, собран `--enable-libopus`; при этом **нативный** кодек отдельный: `-h encoder=opus` → `dr1 delay small exp`, только 48 кГц / `fltp` / mono-stereo, AVOptions только `-opus_delay` (2.5…360 мс, **default 360**) и `-apply_phase_inv`; `-h decoder=opus` → нативный декодер без `exp` | `[H]` |
| H-8 | `opus-tools`/`alsa-utils` | `opusenc`/`opusdec`/`arecord`/`aplay` отсутствуют | `[H]` |
| H-9 | ALSA `default` | `/etc/alsa/conf.d/99-pipewire-default.conf`: `pcm.!default { type pipewire }` → «ALSA default» на этом хосте **и есть PipeWire**, а не железо | `[H]` |
| H-10 | Portal-аудио | интерфейсов `org.freedesktop.portal.Audio*` нет (0 файлов из 56) ⇒ **портального consent на микрофон не существует**; портал покрывает экран/камеру/глобальные хоткеи | `[H]` |
| H-11 | Права на PCM | `getfacl /dev/snd/pcmC2D0c` → `user:sanya:rw-`; `id -nG` = `sanya ollama wheel` (без `audio`) | `[H]` |
| H-12 | Линты | `discord-voice`/`discord-media` — `unsafe_code = "forbid"`; `discord-platform` — `deny` + `#[allow(unsafe_code)]` на модуль (как `ffi_hunspell`, `ffi_secret`) | `[H]` |
| H-13 | `discord-voice` зависит только от `discord-json`; `discord-platform` — только от `libc`; `discord-media` — без зависимостей | `[H]` |
| H-14 | `docs/rust-client/UNSAFE-REGISTRY.md` отсутствует; `discord-platform/Cargo.toml` не имеет `[features]`; `discord-media` не заявлен в `[workspace.dependencies]` | `[H]` |
| H-15 | Гейт: `scripts/gate.mjs:144-163` `commonPhases` = typecheck → ui-tsc → meta → unit → spike-rust → workspace-rust → smoke; фаз под feature-флаги нет | `[H]` |
| H-16 | `TEST-PLAN.md` §T-VOICE уже занимает `T-VOICE-01…23` (20 = устойчивость, 21–23 = MLS/DAVE) | `[H]` |

## 2. Поправки к предыдущим документам (найдено при подготовке)

| # | Где | Что не так | Как в этом плане |
|---|---|---|---|
| F-1 | `G5-PREREQ.md` §5.6 | предложены ID `T-VOICE-20…24`, но они **уже заняты** (`TEST-PLAN.md:232-235`) | новая серия `T-OPUS-01…10` (§6.2), правка TEST-PLAN — `[P]` |
| F-2 | `G5-VOICE-PLAN.md` §9 T-10 | «DTX: `classify_opus_payload` = CN/Empty» — эта функция (`media/rtp.rs:310`) принимает **длину шифротекста**, а не Opus-пакет; на DTX-кадре 3 Б она даёт `ComfortNoise` по совпадению, но тип кадра не доказывает | классификация — по **TOC открытого пакета** (§3.4, §3.9); `classify_opus_payload` остаётся эвристикой capture |
| F-3 | `G5-VOICE-PLAN.md` шаг 3 «ALSA capture» | на хосте `default` = PipeWire-плагин (`99-pipewire-default.conf`) ⇒ «ALSA-путь» и «PipeWire-путь» — не два разных этапа, а один; порядок уточнён в §5 | §5: `null` → fake-backend → (live) `default`-как-PipeWire → нативный `pw_stream` только при необходимости |
| F-4 | `DESIGN/VOICE.md` §3.3 / `G5-VOICE-PLAN.md` §6 | `data-ptt="blocked"` «если окно не в фокусе» — в нашей архитектуре **фокуса окна не видно** (нет протокола/API); проверяемо только «нет сессии/нет разрешения» | §5.4: `blocked` = нет сессии/отказ портала/нет триггера; фокус — UI-фейк (PT-05), не OS-факт |
| F-5 | `G5-PREREQ.md` §5.6 (KAT) | фраза «T-VOICE-20…24 зелёные на фиче» смешивает hermetic-тесты и независимость оракула | §3.0: оракул независим **только** для decode-плеча (пинованные байты чужого энкодера); encode проверяется структурно + evidence-прогоном ffmpeg, не в CI |
| F-6 | `G5-VOICE-PLAN.md` §4.3 п.3 | «тишина → `classify_opus_payload(len)`» как оракул DTX | DTX-контракт: `OPUS_GET_DTX==1`, `len<=3` для ≥95 % кадров, `nb_samples==960` на каждый пакет, число пакетов = числу входных кадров (§3.9) |
| F-7 | `G5-VOICE-PLAN.md` §4.1 | `#[link(name="opus")]` без feature ломает сборку на хосте без libopus, а `commonPhases` feature не передаёт | opt-in feature `system-opus` + жёсткая фаза гейта (§6.3) |

## 3. KAT: векторы, формат, допуски

### 3.0 Что именно доказывает каждая нога

| Нога | Вход → выход | Независимость | В CI |
|---|---|---|---|
| **L1 encode-structural** | наш encode → пакеты → **свой** парсер TOC (RFC 6716) | независима от libopus на уровне проверки битстрима | да, hard |
| **L2 decode-native** | **пинованные** пакеты чужого энкодера → наш decode → PCM | **полная**: байты созданы другой реализацией до теста | да, hard |
| **L3 round-trip** | наш encode → наш decode → PCM | нет (самосогласованность); ловит грубые ошибки | да, hard, пороги мягче |
| **L4 encode-quality** | наш encode → пакеты → `ffmpeg -c:a opus` decode → PCM | **полная**, но требует ffmpeg | `#[ignore]` (evidence-прогон), не в гейте |

Правило: **L2 — единственная нога, где «чужой» оракул герметичен.** Нельзя писать «KAT не зависит
от libopus» про L3/L4 и нельзя класть ffmpeg в гейт (H-7: версия и `exp`-статус нативного энкодера
меняются между выпусками).

### 3.1 Вход: формат и фикстуры

| Свойство | Значение | Обоснование |
|---|---|---|
| Формат | raw **S16_LE, mono, 48 000 Гц**, без WAV-заголовка | заголовок — не вход кодека; убирает неоднозначность парсера; `ffmpeg -f s16le` читает |
| Длина | ровно 48 000 сэмплов = **96 000 байт** = 1.000 с | 50 кадров по 960; целое число периодов у всех компонент |
| Кадр | 960 сэмплов = 20 мс = 1 920 байт PCM | `[L]` PT 120, `ts += 960` (`voice-deep.md` §2.3; `media/rtp.rs`) |
| Порядок | interleaved (для mono == straight) | контракт `opus_encode` |
| Endianness | LE | x86_64 + PipeWire/ALSA S16_LE |

Три вектора (`fixtures/voice/opus/*.s16`, имена фиксированы):

| ID | Файл | Формула (детерминированная, без RNG) |
|---|---|---|
| **A** | `tone-1k-48k-mono-s16.s16` | `x(t) = 0.5·sin(2π·1000·t)`, `t = n/48000`; линейный fade-in/out по **480** сэмплов (10 мс) |
| **B** | `speechlike-48k-mono-s16.s16` | `x(t) = Σᵢ Aᵢ·e(t)·sin(2π fᵢ t + φᵢ)`, `f = {250,450,1200,2400,3200}`, `A = {0.30,0.20,0.12,0.06,0.03}`, `φ = {0, π/3, 2π/3, π, 4π/3}`, `e(t) = 0.70 + 0.30·sin(2π·3·t)`; затем нормировка на `max|x| → 0.5 FS` |
| **C** | `silence-48k-mono-s16.s16` | все нули (только для DTX) |

Почему такие частоты: все `fᵢ` и 3 Гц дают **целое** число периодов за 1.000 с (250, 450, 1200,
2400, 3200, 3 циклов) ⇒ DC = 0 и окно бесшовно; доминирующая частота проверяется точно, без
спектральной утечки на краях.

Квантование: `s = clamp(round(x·32767.0), -32768, 32767) as i16`. `round` — обязателен
(усечение даёт DC-смещение −0.5 LSB на положительных отсчётах).

### 3.2 Генератор

`waves/RW-40/VOICE/gen-opus-fixtures.mjs` — Node ≥ 24, ESM, `// @ts-check`, **без зависимостей**.
Пишет три `.s16` в `fixtures/voice/opus/` и печатает `sha256`, `md5`, `bytes`, `samples`, `peak`,
`dc` для каждой. Формулы — дословно §3.1, чтобы перегенерация была возможна без чтения этого
документа.

Правило: **фикстуры коммитятся**, генератор нужен для провенанса и починки, тесты его не вызывают.
Если на другом хосте `sha256` не совпал — это **отчёт о расхождении**, а не «перегенерировали и
позеленили»: расхождение = stop, разбор (`f64::sin` в libm может отличаться на 1 ulp; при
`round(x·32767)` это влияет только на отсчёты, стоящие ровно на .5, — их должно быть ноль).

### 3.3 Контейнер пакетов `OKAT` (свой, не Ogg)

Ogg не годится: страницы, `pre-skip`, granule position и два служебных пакета (`OpusHead`,
`OpusTags`) — лишняя семантика, которую придётся парсить в тесте. Вводим плоский формат:

| Смещение | Размер | Поле | Значение |
|---:|---:|---|---|
| 0 | 4 | magic | ASCII `OKAT` |
| 4 | 4 | `version` u32 LE | `1` |
| 8 | 2 | `channels` u16 LE | `1` |
| 10 | 4 | `sample_rate` u32 LE | `48000` |
| 14 | 4 | `frame_size` u32 LE | `960` (сэмплов на канал) |
| 18 | 4 | `packet_count` u32 LE | `50` |
| 22 | 4 | `reserved` u32 LE | `0` |
| 26 | … | пакеты | `packet_count` × (`u32 LE len`, `len` байт) |

Инварианты парсера (все — hard, каждая = отдельный тест): длина файла **точно** `26 + Σ(4+len)`;
`version==1`; `channels==1`; `sample_rate==48000`; `frame_size==960`; каждый `len ∈ 1..=1275`;
хвостовых байтов нет; `packet_count==50`. Любое нарушение — `ContainerError` с **смещением**, без
паники и без доверия к `packet_count` (читаем итеративно, `checked_add`).

Замечание: `max_data_bytes` у `opus_encode` — 1275 Б на пакет; это верхняя граница и для фикстур.

### 3.4 Encode-путь (L1 + L3)

1. `encoder_create(48000, 1, OPUS_APPLICATION_VOIP)` → `err == OPUS_OK`.
2. `OPUS_SET_COMPLEXITY(10)`, `OPUS_SET_DTX(0)` для A/B, `OPUS_SET_BITRATE` **не трогаем** до live
   (default). `OPUS_GET_LOOKAHEAD` читаем до первого `encode`.
3. 50 × `opus_encode(st, pcm[i·960 ..], 960, buf[1275], 1275)`; отрицательный код → сразу
   `Err(OpusError)` с `opus_strerror`.
4. Структурные инварианты **без декодирования** (независимый парсер TOC из `discord-media`):
   - ровно 50 пакетов, каждый `1..=1275` Б;
   - TOC `code = toc & 0x03` == `0` (один кадр на пакет);
   - `config = toc >> 3` ∈ `{1,5,9,13,15,19,23,27,31}` — это ровно 20 мс во всех режимах
     RFC 6716 Table 2; **это и есть доказательство 20 мс на уровне битстрима**;
   - `stereo = (toc >> 2) & 1` == `0`;
   - все пакеты имеют одинаковый `config` (стабильность режима);
   - кросс-проверка: наш парсер и `opus_packet_get_nb_samples(pkt,len,48000)` дают **960**,
     `opus_packet_get_nb_frames` — **1**, `opus_packet_get_nb_channels` — **1**. Расхождение =
     дефект одной из сторон, тест падает с обоими значениями.
5. L3: пакеты → наш decode → PCM; метрики §3.7.

`opus_packet_get_nb_channels(pkt)` не принимает длину ⇒ вызывается **только** после `len >= 1`
(инвариант I-5). На `len == 0` — UB в libopus, а не ошибка.

### 3.5 Decode-путь (L2): чужой энкодер → наши байты

Генерация (отдельная карточка, не в CI):

1. `ffmpeg -f s16le -ar 48000 -ac 1 -i tone.s16 -c:a opus -strict -2 -ar 48000 -ac 1
   -sample_fmt fltp -opus_delay 2.5 -f ogg native-tone.ogg`
   (`-c:a opus` — **нативный** энкодер, не `libopus`; `-strict -2` — потому что `exp`;
   `-opus_delay 2.5` — потому что default 360 мс (H-7) иначе 1.000 с входа не даст 50 пакетов).
2. Демукс Ogg: разобрать lacing-таблицу страниц (RFC 3533), склеить пакеты, **отбросить первые
   два** (`OpusHead`, `OpusTags`). Проверки генератора (жёсткие, до записи):
   `packet_count == 50`, Σ `nb_samples` (по TOC) == 48 000, размеры `1..=1275`.
   Если 50 не выходит — генератор **падает** (не «подрезаем»), и разбирается `-opus_delay`.
3. Записать `native-tone.okat`, `native-speechlike.okat` + строки в манифест (версия ffmpeg,
   полная командная строка, `encoder=opus`, `decoder=opus`, sha256).

Тест (герметичный): читаем `.okat` → `decoder_create(48000,1)` → 50 × `opus_decode(...)` →
`ret == 960` каждый, всего 48 000 сэмплов → сравнение с исходным `.s16` по §3.6–3.7.

Почему это настоящий KAT: байты созданы **другим** кодеком и заморожены; наш декодер обязан их
понять. Изменение нашего кода не может «подкрутить» ожидание.

### 3.6 Выравнивание: lag и lookahead

Задержка кодека не хардкодится:

1. `lag` ищется нормированной кросс-корреляцией в окне `±960` сэмплов (±20 мс) на перекрытии.
2. `lag` обязан совпасть с `OPUS_GET_LOOKAHEAD` **±1 сэмпл** и быть **одинаковым на 3 прогонах**.
   Не совпало/нестабильно → провал KAT (нестабильность), а не «подгонка порога».
3. SNR/RMS/max|err| считаются **после сдвига** и без первых/последних 960 сэмплов (краевые
   эффекты fade и warm-up энкодера).

Почему не «по индексу»: на 1 кГц ошибка на 480 сэмплов = противофаза, SNR уходит в −∞ при
формально «правильном» сигнале.

### 3.7 Допуски

| Нога | Метрика | Кандидат | Статус |
|---|---|---|---|
| L3 round-trip, тон | SNR | ≥ 20 дБ | `[A]` |
| L3 round-trip, speechlike | SNR | ≥ 12 дБ | `[A]` |
| L2 native→libopus, тон | SNR | ≥ 10 дБ | `[A]` |
| L2 native→libopus, speechlike | SNR | ≥ 8 дБ | `[A]`, самый слабый оракул |
| L2/L3 тон | Гёрцель: амплитуда на 1000 Гц минус на 900 Гц и на 1100 Гц | ≥ 6 дБ каждый | `[A]` |
| L2/L3 speechlike | Гёрцель на 250 Гц vs 200/300 Гц | ≥ 3 дБ | `[A]` |
| все | max abs err | ≤ 0.25 FS (8192/32768) | `[A]` |
| все | RMS | ±3 дБ от эталона | `[A]` |
| все | длительность/кадры | ровно 48 000 сэмплов / 50 пакетов | контракт |
| все | `lag` | == `lookahead` ±1, стабилен на 3 прогонах | контракт |

**Правило калибровки (одноразовое, до принятия D-113):** порог := `max(кандидат,
измеренное − 3 дБ)` по первому зелёному прогону; фактические значения и итоговые пороги
записываются в `KAT-OPUS.json` и в карточку. После принятия порог **заморожен**; снижение —
только решением PM с записью в решениях. Это единственная разрешённая «настройка» порога, и она
не может превратиться в ослабление: `измеренное − 3 дБ` — верхняя граница на снижение, а не
подгонка под текущий результат.

**Байтовое равенство пакетов в гейте не проверяется.** libopus выбирает SIMD-путь по CPU (RTCD),
поэтому хэш пакетов валиден только с парой (версия libopus, CPU). Вместо него:
- в гейте — hard-проверка **детерминизма в процессе** (3 прогона → одинаковый sha256);
- сравнение с пиновым хэшем — `#[ignore]`-тест «канонический хост» (виден гейту как `ignored: N`)
  и evidence-скрипт, который пишет `host`, `cpu`, `libopus_version`, хэш.

### 3.8 Негативные и устойчивость (обязательная часть DoD)

| # | Вход | Ожидание | Точность |
|---|---|---|---|
| N-1 | `encoder_create(12345, 1, VOIP)` | `NULL` + `err == OPUS_BAD_ARG` | код фиксируется первым прогоном, допустимое множество `{BAD_ARG}` |
| N-2 | `encoder_create(48000, 0, …)` / `(48000, 3, …)` | `NULL` + `BAD_ARG` | то же |
| N-3 | `encode(frame_size = 480)` | `OPUS_BAD_ARG`, энкодер **остаётся рабочим** (следующий 960 → >0) | код фиксируется |
| N-4 | `encode(max_data_bytes = 16)` | `ret < 0`; **канарейка**: 32 байта после 16-байтного буфера не изменены | `{BUFFER_TOO_SMALL, BAD_ARG}` |
| N-5 | `decode` пакета `[0xFF; 20]` и пакета, обрезанного на 5 Б | `ret < 0` (`INVALID_PACKET`), без паники и чтения за границей | `{INVALID_PACKET, BAD_ARG}` |
| N-6 | `decode(NULL, 0, out, 960, 0)` | `ret == 960` (PLC), не ошибка | контракт |
| N-7 | `decode(..., frame_size = 480)` для 20-мс пакета | `ret == OPUS_BUFFER_TOO_SMALL` | фиксируется |
| N-8 | `decode(..., data != NULL, len = 0)` | `ret < 0`, без паники | `{INVALID_PACKET, BAD_ARG}` |
| N-9 | двойное освобождение | невозможно: `Encoder`/`Decoder` — `!Clone`, владелец один, `Drop` | типы + ревью |
| N-10 | вызов после `Drop` | невозможно: `Drop` потребляет `self` | типы |

### 3.9 DTX/CN и FEC — отдельные векторы

**DTX (а) — энкодер.** Новый энкодер с `OPUS_SET_DTX(1)`, вход — фикстура C (50 кадров нулей):
- `OPUS_GET_DTX` → `1` (ctl-путь реально сработал);
- число пакетов == **50** (DTX не «съедает» кадры, а укорачивает их);
- `len <= 3` для ≥ 95 % пакетов; фактические длины записываются, **не подгоняются**;
- каждый пакет: `code == 0`, `nb_samples == 960`;
- наш decode каждого пакета → `ret == 960`.
Классификация — **по TOC/длине открытого пакета** (§3.4), не по `classify_opus_payload` (F-2).

**DTX (б) — подавление передачи** — это `RtpSender::silence(elapsed_frames)` (`media/rtp.rs:286`),
отдельный юнит-тест G1: `sequence` не растёт, `timestamp += 960·elapsed`, `marker` взведён.
К libopus отношения не имеет и в `opus_kat.rs` не проверяется.

**FEC (в).** `OPUS_SET_INBAND_FEC(1)` + `OPUS_SET_PACKET_LOSS_PERC(10)`; на фикстуре B: после
потери пакета `i` вызов `opus_decode(пакет i+1, decode_fec = 1)` → `ret == 960`. Проверяет путь,
который реально использует джиттер-буфер; без него FEC останется непроверенной строкой в конфиге.
Пометка: FEC-данные появляются не на каждом пакете — если `ret` иной, тест фиксирует **факт**
(`fec_available = false`) и требует ручного решения, а не молчаливого pass; до этого кейс
остаётся `#[ignore]` с причиной в имени (не «зелёный»).

### 3.10 Манифест фикстур и `KAT-OPUS.json`

`fixtures/voice/opus/manifest.json` (схема `discord-lab/opus-kat-fixtures-v1`), поля на файл:

```json
{
  "path": "voice/opus/native-tone.okat",
  "kind": "packets|pcm",
  "bytes": 0, "sha256": "…", "md5": "…",
  "sample_rate": 48000, "channels": 1, "frame_size": 960,
  "packets": 50, "samples": 48000,
  "generator": "ffmpeg n9.0.2 (-c:a opus, -strict -2, -opus_delay 2.5) | gen-opus-fixtures.mjs",
  "encoder": "opus (native, exp)", "decoder_oracle": "opus (native)",
  "license": "repo (синтез, не запись человека)"
}
```

`KAT-OPUS.json` (артефакт прогона, пишется в `waves/<ID>/`): `libopus_version`, `host`, `cpu`,
`lookahead`, `lag` (3 прогона), `packets`, `bytes_min/max/mean`, `config_set`, `bandwidth_set`,
SNR/RMS/max|err| по каждой ноге, `dtx_lengths`, `fec_available`, `thresholds` (фактические),
`git_head`, `ffmpeg` (для L4). Ни PCM, ни ключей, ни payload-байтов в артефакте.

## 4. FFI / ABI

### 4.1 Размещение и линковка

| Файл | Содержимое | Линт |
|---|---|---|
| `discord-platform/src/ffi_opus.rs` (**new**) | `#[link(name="opus")]`, `unsafe extern "C"`, opaque-типы, typed ctl-хелперы | `#[allow(unsafe_code)]` на модуле |
| `discord-platform/src/opus.rs` (**new**) | безопасный API `Encoder`/`Decoder`/`OpusError`, без `unsafe` | `deny` сохраняется |
| `discord-media/src/opus.rs` (**new**) | **чистые** типы: `OpusError`, `OkatFile`, TOC-парсер, `InboundPcm`, константы 960/48000/1275 | `forbid` сохраняется |
| `discord-media/src/lib.rs` | `+ pub mod opus;` | `forbid` |
| `discord-platform/Cargo.toml` | `[features] system-opus = []` + `discord-media` | — |
| `discord-platform/src/lib.rs` | `#[cfg(feature = "system-opus")] #[allow(unsafe_code)] mod ffi_opus;` + `#[cfg(feature = "system-opus")] pub mod opus;` | — |

Почему так:
- **FFI только в `discord-platform`** — `forbid` в voice/media не ослабляется (PLAT-12/15).
- Всё, что не требует libopus, живёт в `discord-media` ⇒ **всегда** в `workspace-rust`-фазе гейта,
  без feature. Это главное отличие от плана-предшественника, где под feature уходил весь KAT.
- Feature **off by default**: workspace собирается на хосте без libopus; фаза гейта (§6.3)
  передаёт `--features system-opus` явно.
- `#[link]` без feature не применяется, поэтому линковка не «протекает» в остальные крейты.

Зависимость `discord-platform → discord-media` — новая; `discord-media` **не** в
`[workspace.dependencies]` (H-14) ⇒ одна строка в `rust/Cargo.toml` — правка Team Lead `[P]`.

### 4.2 Объявления (точные типы)

```rust
/// Непрозрачные типы: раскладку знает только libopus, мы не разыменовываем.
#[repr(C)] pub struct OpusEncoder { _private: [u8; 0] }
#[repr(C)] pub struct OpusDecoder { _private: [u8; 0] }

// SAFETY: сигнатуры сверены с /usr/include/opus/opus.h 1.6.1 (H-2); libopus линкуется системно.
#[link(name = "opus")]
unsafe extern "C" {
    // opus_int32 == int32_t -> i32; C `int` -> c_int. НЕ взаимозаменять с i64.
    fn opus_encoder_create(fs: i32, channels: c_int, application: c_int, error: *mut c_int) -> *mut OpusEncoder;
    fn opus_encode(st: *mut OpusEncoder, pcm: *const i16, frame_size: c_int,
                   data: *mut u8, max_data_bytes: i32) -> i32;
    fn opus_encoder_destroy(st: *mut OpusEncoder);

    fn opus_decoder_create(fs: i32, channels: c_int, error: *mut c_int) -> *mut OpusDecoder;
    fn opus_decode(st: *mut OpusDecoder, data: *const u8, len: i32, pcm: *mut i16,
                   frame_size: c_int, decode_fec: c_int) -> c_int;
    fn opus_decoder_destroy(st: *mut OpusDecoder);

    // Вариадические: единственное «тонкое» место ABI (C-19 в G5-PREREQ).
    fn opus_encoder_ctl(st: *mut OpusEncoder, request: c_int, ...) -> c_int;
    fn opus_decoder_ctl(st: *mut OpusDecoder, request: c_int, ...) -> c_int;

    // Интроспекторы битстрима (только чтение TOC).
    fn opus_packet_get_nb_frames(packet: *const u8, len: i32) -> c_int;
    fn opus_packet_get_nb_samples(packet: *const u8, len: i32, fs: i32) -> c_int;
    fn opus_packet_get_nb_channels(packet: *const u8) -> c_int; // len НЕ принимает
    fn opus_packet_get_bandwidth(packet: *const u8) -> c_int;

    fn opus_strerror(error: c_int) -> *const c_char;
    fn opus_get_version_string() -> *const c_char;
}
```

Вариадический ctl закрывается **одним хелпером на запрос** (никаких прямых вызовов снаружи):

```rust
fn ctl_int(st: *mut OpusEncoder, request: c_int, value: c_int) -> Result<(), OpusError>;
fn ctl_int_out(st: *mut OpusEncoder, request: c_int, out: &mut i32) -> Result<(), OpusError>;
```

Разрешённые запросы — только из белого списка (`SET_COMPLEXITY 4010`, `SET_DTX 4016`,
`GET_DTX 4017`, `SET_INBAND_FEC 4012`, `SET_PACKET_LOSS_PERC 4014`, `GET_LOOKAHEAD 4027`).
Значения — только `c_int` (не `i64`, не `u32`, не указатель, кроме явного `*mut i32` в
`ctl_int_out`). Причина: в C `...` не проверяется, ошибка типа = UB без ошибки компиляции.

### 4.3 Unsafe-инварианты и что их ломает

| ID | Инвариант | Ломающая мутация → красный тест |
|---|---|---|
| **I-1** | `Encoder`/`Decoder` владеют `NonNull<T>`, `Drop` вызывает `*_destroy` ровно один раз, `!Clone` | заменить `Drop` на `mem::forget` → утечка (T-OPUS-09 фиксирует счётчик созданных/уничтоженных через тестовый счётчик) |
| **I-2** | указатели берутся только из `&[i16]`/`&mut [u8;1275]`, длины — из `slice.len()` (никогда из входных данных) | подменить `frame_size` на константу 960 при коротком срезе → I-3-тест краснеет |
| **I-3** | `pcm.len() == frame_size * channels` проверяется до FFI; `frame_size` — константа `FRAME_SIZE: usize = 960` | `960 → 480` в константе → T-OPUS-02/03 краснеют |
| **I-4** | `max_data_bytes == buf.len() == 1275`, не из входа | сузить буфер → канарейка N-4 краснеет |
| **I-5** | `opus_packet_get_nb_channels` вызывается только после `len >= 1`; для остальных `len` из `slice.len()`, `len <= 1275` | убрать проверку `len >= 1` → тест пустого пакета падает (ASAN-подобная канарейка) |
| **I-6** | ctl — только через `ctl_int`/`ctl_int_out`, request из белого списка, тип `c_int`/`*mut i32` | meta-grep: `opus_(en|de)coder_ctl` вне `ffi_opus.rs` = 0; добавить прямой вызов в тест-мутацию → meta краснеет |
| **I-7** | `opus_strerror`/`opus_get_version_string` копируются в `String` немедленно (`CStr::from_ptr(..).to_string_lossy()`), указатель не хранится | сохранить `*const c_char` в структуре → не собирается (тип не `Send`), ревью-чеклист |
| **I-8** | выходной буфер decode — `[i16; 960]` на канал, `frame_size = 960`; `ret <= 960` проверяется до чтения | читать `ret` сэмплов без проверки `ret <= 960` → тест с `ret == -2` краснеет |
| **I-9** | `decode_fec ∈ {0,1}`, тип `c_int` | передать `true as i32` без проверки — допустимо; передать `2` → тест валидации краснеет |
| **I-10** | Нет `null`-хендла: `create == NULL` → `Err`, дальше `NonNull`; уничтоженное состояние недостижимо (владение) | вернуть `Encoder` при `NULL` → N-1/N-2 краснеют |
| **I-11** | `Debug`/`Display` ошибок не содержат PCM/payload/указателей; только `code` + короткий текст | добавить `{:?}` буфера в `Display` → тест приватности краснеет |
| **I-12** | Паники в FFI-пути нет: в `unsafe`-блоках нет `unwrap`/`expect`/индексации без проверки; Rust-callback в C не передаётся ⇒ PLAT-06 не применяется | `unwrap()` на результате → clippy-`unwrap_used`-чеклист ревью; тест «битый пакет не паникует» |

`unsafe impl Send/Sync` — **запрещено** (meta-тест PLAT-15 уже требует 0). `Encoder` остаётся
`!Send`/`!Sync` через `PhantomData<*mut ()>`: аудиопоток один, случайная отправка в другой поток —
дефект, а не фича.

### 4.4 Error mapping

```rust
pub enum OpusError {
    BadArg,                 // -1
    BufferTooSmall,         // -2
    Internal,               // -3
    InvalidPacket,          // -4
    Unimplemented,          // -5
    InvalidState,           // -6
    AllocFailed,            // -7
    CodecInit { code: i32 },// create вернул NULL; code из err-out
    InvalidArgument { what: &'static str }, // до FFI: frame_size/channels/max_len/len
    Container { offset: usize, what: &'static str }, // OKAT-парсер
    Unknown { code: i32 },  // всё, чего нет в таблице
}
```

Правила:
1. Код C → вариант по таблице H-4; `Unknown{code}` вместо паники на новых кодах (libopus
   аддитивен).
2. Текст `opus_strerror(code)` **не** становится источником истины: он попадает в
   `Display`/лог рядом с кодом, но тесты сверяют **код** (текст меняется между версиями).
3. `BadArg` от `create` превращается в `CodecInit` (различаем «не смогли создать» и «неверный
   аргумент вызова»).
4. Ошибка никогда не содержит сэмплов/пакетов/указателей (I-11); в лог — только код, операция,
   размеры (не содержимое).
5. `-ENODEV`/xrun — **не** `OpusError`, это ALSA-слой (§5), чтобы коды не смешивались.

### 4.5 Что можно проверить без live (полный список)

- ABI-smoke: `opus_get_version_string()` непустая и `starts_with("libopus 1.6")` (`[H]` 1.6.1).
- Создание/уничтожение 1 000 раз: RSS/счётчик I-1 не растёт (утечки нет) — без внешних средств,
  через счётчик в тестовой обёртке + `Drop`.
- Весь encode/decode KAT (L1/L2/L3) — детерминированные байты, никакой сети/устройств.
- Негативные коды (N-1…N-10), PLC, DTX-энкодер, FEC-путь.
- Структурный TOC-разбор (свой парсер) + кросс-проверка с интроспекторами libopus.
- Контейнер `OKAT` (парсер/валидатор) — целиком в `discord-media`, без FFI.
- Приватность: ни один `Debug`/`Display` ошибки не содержит payload.
- Метрики: SNR/lag/lookahead/Гёрцель — чистый Rust, без зависимостей.
- **Нельзя**: «звучит как Discord», «битрейт как у Discord», «сервер примет поток» (§10).

## 5. ALSA `null` → PipeWire: порядок и предпосылки

### 5.1 Порядок (и почему он именно такой)

```text
S0  contract: trait PcmBackend { open, read_exact_960, write, recover, close }
S1  NullBackend        — ALSA pcm "null" (alsa.conf:335)        offline, без демона
S2  FakeBackend        — детерминированные частичные чтения      offline, always-on gate
S3  DefaultBackend     — ALSA pcm.!default == PipeWire plugin     live-окно W-1
S4  PipeWireBackend    — pw_stream (нативный клиент)              только если S3 не хватает
```

Обоснование каждого шага:

- **S1 `null`** (`[H]`: `pcm.null` в `alsa.conf:335`) доказывает только механику FFI:
  `snd_pcm_open("null")`, `snd_pcm_set_params` (не вариадический, `pcm.h:681`), `readi`/`writei`
  по 960, `snd_pcm_recover`, `snd_strerror`. Ни прав, ни consent, ни реального времени он не
  проверяет: `null` генерирует нули и не имеет тайминга.
- **S2 fake** обязателен, потому что `null` **всегда отдаёт ровно запрошенное** — реальный
  backend может вернуть меньше. Аккумулятор «до ровно 960» иначе останется непроверенным.
  S2 — единственный шаг, который стоит в `gate:worker` без флагов.
- **S3** — здесь ловушка: `pcm.!default` на этом хосте **и есть PipeWire**
  (`/etc/alsa/conf.d/99-pipewire-default.conf`, H-9). То есть «перейти с ALSA на PipeWire» —
  это не смена этапа, а смена **того же** `default`-пути: ALSA-плагин PipeWire. Проверять
  `hw:*` напрямую нельзя: это обходит PipeWire, даёт эксклюзивный захват и ломает чужие сессии.
- **S4** нативный `pw_stream` нужен **не для микрофона в MVP**, а для кадра портала
  (`OpenPipeWireRemote`), и требует `catch_unwind`-дисциплины в C-callback (PLAT-06/D-9).
  Смешивать его с Opus/ALSA-этапом нельзя: другая дисциплина `unsafe`.

Следствие для плана: **S4 не блокирует Opus и не блокирует микрофон.** Микрофон MVP =
ALSA-плагин PipeWire (S3). Это снимает из критического пути FFI-callback риск.

### 5.2 Device prerequisites

| Предпосылка | Факт | Что делать |
|---|---|---|
| Права на PCM | ACL `user:sanya:rw-` на `pcmC2D0c` (H-11), группы `audio` нет | ничего; но полагаться на группу нельзя — проверять код ошибки `open`, а не «моя группа» |
| Формат | S16_LE / 48000 / mono | `snd_pcm_set_params(..., soft_resample=1, latency_us=20000)`; если устройство не поддерживает — `DeviceUnavailable` (не «конвертируем втихую») |
| Ровно 960 | `readi` вправе вернуть меньше | аккумулятор 960 с `checked` счётчиком; недобор → ждать, перебор → ошибка контракта |
| xrun | `-EPIPE`/`-ESTRPIPE` | `snd_pcm_recover` + счётчик; 3 подряд → `DeviceUnavailable` |
| Исчезновение | `-ENODEV` | typed `DeviceUnavailable` → `device-missing` (VOICE §3.2), разговор не рвём |
| Перечисление | `snd_device_name_hint(-1, "pcm", …)` + фильтр `IOID` | наружу `{index, direction}`, описание — **не логировать** (PII, VOICE §5.1 п.6) |
| Hot-plug | poll 1000 мс | событие обновляет список, **не** переключает устройство |
| Никакой записи | `startLocalAudioRecording`/`setAecDump`/`setLoopback` не вызываются | meta-grep + тест приватности (VC-05) |

### 5.3 PTT prerequisites (что должно быть готово до «PTT работает»)

| # | Предпосылка | Состояние | Владелец |
|---|---|---|---|
| P-1 | `Value::U64` + выравнивание 8 в D-Bus-клиенте (сигнал `Activated(..., t timestamp, ...)`) | **нет** (`dbus/mod.rs`), блокер разбора сигнала | G5, до PTT |
| P-2 | публичный typed-вызов с fd и пустым `a{sv}` | **нет** (нужен для `OpenPipeWireRemote`, не для PTT) | G5 |
| P-3 | mock-портал `GlobalShortcuts` v2 (см. `G5-VOICE-PLAN` T-16) | нет | G5 |
| P-4 | `blocked` без фокуса окна | **непроверяемо** (F-4): фокуса не видно; `blocked` = нет сессии / отказ / нет триггера | G2 (UI) + G5 |
| P-5 | «ограниченный режим» (`JMyQin`) | `[A]`, вне MVP: без фокуса не определить | PM |

PTT в MVP **выключен по умолчанию**, клавиша **не назначается** (в дампе `keybind=null`).
До P-1 любой PTT-тест — только UI-фейк без D-Bus.

## 6. Тест-план Rust

### 6.1 Файлы

| Файл | Крейт | Feature | Что |
|---|---|---|---|
| `discord-media/src/opus.rs` | media | — | `FRAME_SIZE`, `SAMPLE_RATE`, `MAX_PACKET`, `OpusError`, `OkatFile`, TOC-парсер |
| `discord-media/tests/opus_container.rs` | media | — | контейнер, TOC, error mapping (always-on) |
| `discord-platform/src/ffi_opus.rs` | platform | `system-opus` | объявления + ctl-хелперы |
| `discord-platform/src/opus.rs` | platform | `system-opus` | `Encoder`/`Decoder` безопасный API |
| `discord-platform/tests/opus_kat.rs` | platform | `system-opus` | L1/L2/L3, негативные, DTX, FEC |
| `discord-voice/tests/voice_pipeline.rs` | voice | — | fake PCM → encode-пакеты из фикстур → `RtpSender` → fake sink (G1) |

### 6.2 Кейсы (red-first; ID не пересекаются с `T-VOICE-01…23`)

| ID | Ур. | Проверка | Оракул | Offline | Owner |
|---|---|---|---|---|---|
| **T-OPUS-01** | L1 | ABI-smoke: версия, create/destroy ×1000 без роста счётчика | `opus_get_version_string` | да | G5 |
| **T-OPUS-02** | L1 | encode A/B: 50 пакетов, 1..=1275 Б, TOC 20 мс, 1 кадр, mono | свой TOC-парсер + интроспекторы | да | G5 |
| **T-OPUS-03** | L2 | native-пакеты → наш decode → 48 000 сэмплов, lag==lookahead, SNR | пиновые `.okat` + `.s16` | да | G5 |
| **T-OPUS-04** | L3 | наш encode → наш decode → SNR/RMS/max\|err\|/Гёрцель | эталон `.s16` | да | G5 |
| **T-OPUS-05** | L1 | негативные коды N-1…N-8, канарейки, PLC | таблица H-4 | да | G5 |
| **T-OPUS-06** | L1 | DTX-энкодер: `GET_DTX==1`, 50 пакетов, `len<=3` ≥95 %, `nb_samples==960` | TOC | да | G5 |
| **T-OPUS-07** | L1 | контейнер `OKAT`: happy + 8 битых вариантов (обрезка, `len=0`, `len>1275`, `version=2`, хвост, `packet_count` врёт, пустой файл, `channels=2`) | спецификация §3.3 | да | G5 |
| **T-OPUS-08** | L2 | FEC: `decode_fec=1` на следующем пакете → 960 | пакеты B | да (`#[ignore]` до подтверждения) | G5 |
| **T-OPUS-09** | L2 | приватность/утечки: `Debug`/`Display` без payload, `!Clone`, `!Send`, `Drop` один раз | контрпримеры | да | G4/G4b |
| **T-OPUS-10** | L3 | пайплайн: 50 пакетов → `RtpSender`: `seq+50`, `ts+48000`, `marker` на первом, 0 plaintext в «шифре» | `media/rtp.rs` + fake sink | да | G1 |
| **L4-evidence** | — | наш encode → `ffmpeg -c:a opus` decode → SNR | ffmpeg native | `#[ignore]`, evidence | G5 |
| **L5-live** | — | реальный голос/UDP/SRTP | — | **нет** | §10 |

Red-first: на каждую карточку фиксируется красный прогон. Для T-OPUS-07 red = паника/`todo!` в
парсере; для T-OPUS-02/03 red = отсутствие модуля (compile-red) — допустимо, но лог компиляции
прикладывается (как RW-15/P2-04).

### 6.3 Гейт

`[P]` (правит Team Lead) — фаза в `commonPhases` (`scripts/gate.mjs:144`):

```js
{
  name: "opus-kat",
  command: "cargo",
  args: ["test", "--offline", "-p", "discord-platform",
         "--features", "system-opus", "--test", "opus_kat"],
  cwd: "rust",
  countsKind: "cargo",
  failOnSkips: true,
  preflight: opusPreflight, // печатает версию libopus / отсутствие и путь .so
}
```

Почему **hard**, а не skip-aware:
- фикстуры закоммичены ⇒ внешнего оракула в CI нет; нужна только `libopus.so`;
- libopus 1.6.1 на хосте есть (H-1); на хосте без неё фаза падает **ошибкой линковки** — это
  честный сигнал «нужен пакет `opus`», а не «зелёный отчёт без прогона» (риск 13 из G5-PREREQ);
- `failOnSkips: true` ловит `#[ignore]` (T-OPUS-08, канонический хэш), чтобы они были видны
  числом, а не «прошли» (урок PLAT-02).

`discord-media`-часть (`T-OPUS-07` + TOC) идёт в `workspace-rust` **без** флагов и без feature.

### 6.4 Мутации (обязательны)

| Мутация | Краснеет |
|---|---|
| `FRAME_SIZE 960 → 480` | T-OPUS-02, T-OPUS-04, T-OPUS-10 |
| `OPUS_SET_DTX(0)` вместо `1` в DTX-тесте | T-OPUS-06 |
| убрать проверку `ret <= frame_size` | T-OPUS-05 (N-7) |
| убрать `len >= 1` перед `nb_channels` | T-OPUS-05 (N-8), T-OPUS-07 |
| `max_data_bytes = 4096` | T-OPUS-05 (N-4) канарейка |
| убрать `Drop` у `Encoder` | T-OPUS-01 (счётчик) |
| TOC-парсер: `config>>3` → `config>>2` | T-OPUS-02 (20 мс) |
| `lag`-допуск ±1 → ±480 | T-OPUS-03 (нестабильность на 3 прогонах) |
| добавить `opus_encoder_ctl` вне `ffi_opus.rs` | meta-grep (I-6) |
| `unsafe impl Send for Encoder` | meta `unsafe-inventory` (PLAT-15) |

### 6.5 Evidence

`waves/<ID>/`: red-лог, green-лог, targeted, gate-лог, `KAT-OPUS.json`, sha256 фикстур,
вывод `opus_get_version_string`, версия ffmpeg (только для L4), транскрипт генератора.
Без PCM/ключей/токенов в логах.

## 7. Владение файлами

| Файл | Owner | Замечание |
|---|---|---|
| `waves/RW-40/VOICE/OPUS-KAT.md` | **G5** | этот документ |
| `waves/RW-40/VOICE/gen-opus-fixtures.mjs` | **G5** | генератор PCM/парсер Ogg |
| `fixtures/voice/opus/*` (5 файлов) | **G5** | новые файлы; каталог создаётся впервые |
| `rust/crates/discord-media/src/opus.rs`, `tests/opus_container.rs` | **G5** | +1 строка в `media/src/lib.rs` |
| `rust/crates/discord-platform/src/{ffi_opus,opus}.rs`, `tests/opus_kat.rs` | **G5** | +2 строки в `platform/src/lib.rs` |
| `rust/crates/discord-platform/Cargo.toml` | **G5** + ack Lead | `[features] system-opus` (аддитивно, off) |
| `rust/crates/discord-voice/tests/voice_pipeline.rs` | **G1** | после T-OPUS-02/03 |
| `rust/crates/discord-voice/src/media/rtp.rs` | **G1** | G5 не правит; нужен только для T-OPUS-10 |
| `rust/Cargo.toml` (`discord-media` в `[workspace.dependencies]`) | **Lead** `[P]` | одна строка |
| `fixtures/MANIFEST.json`, `fixtures/README.md` | **Lead** `[P]` | G5 приносит готовые значения |
| `docs/rust-client/TEST-PLAN.md` (T-OPUS-01…10) | **Lead** `[P]` | серия не пересекается с T-VOICE |
| `docs/rust-client/UNSAFE-REGISTRY.md` (**создать**) | **Lead** `[P]` | G5 готовит 12 строк I-1…I-12 |
| `scripts/gate.mjs` (фаза `opus-kat`) | **Lead** `[P]` | §6.3 |
| `CONTRACTS/voice.md` (V-00) | **G1+G5 → Lead** `[P]` | PCM-раскладка/владение/ошибки |

Пересечений нет: G5 не трогает `discord-voice`, G1 не трогает `discord-media`/`discord-platform`.

## 8. Порядок работ и DoD

| Карточка | Содержание | Red-first | DoD |
|---|---|---|---|
| **OPUS-1** | генератор + 3 `.s16` + 2 `.okat` + `manifest.json` | генератор без файлов | 5 файлов, sha256 совпадает при повторной генерации на хосте, `packet_count==50`, Σ=48 000 |
| **OPUS-2** | `discord-media/src/opus.rs`: константы, `OpusError`, `OKAT`-парсер, TOC | T-OPUS-07 (panic/`todo!`) | T-OPUS-07 зелёный, `workspace-rust` без флагов |
| **OPUS-3** | `ffi_opus` + `opus` + feature + `opus_kat` | compile-red, затем T-OPUS-01/02 | T-OPUS-01/02/05 зелёные |
| **OPUS-4** | L2/L3 + DTX: T-OPUS-03/04/06 | пороги не достигаются на заглушке | зелёные, `KAT-OPUS.json` с фактическими значениями |
| **OPUS-5** | гейт-фаза, TEST-PLAN, UNSAFE-REGISTRY, `fixtures/MANIFEST.json` | — | фаза в `gate:worker` зелёная, `ignored` виден числом |
| **OPUS-6** | ALSA `null`/fake backend (S1/S2 из §5.1) | T-ALSA-01/02 | 960-аккумулятор, xrun-восстановление, `device-missing` |
| **OPUS-7** | G1: `voice_pipeline.rs` | T-OPUS-10 | `seq`/`ts`/`marker`, 0 plaintext |
| **OPUS-8** | live-окно W-1: `default`(=PipeWire) микрофон, mute, PTT (после P-1) | — | §10 |

DoD всей карточки (V-04 закрывается **офлайн**): T-OPUS-01…07 зелёные под `system-opus`,
T-OPUS-07 зелёный без флагов, `KAT-OPUS.json` с версией/libopus/lookahead/lag/SNR по 3 прогонам,
фикстуры + манифест зарегистрированы, SAFETY-строки в UNSAFE-REGISTRY, ревью G4 (независимость
оракула: L2 действительно пиновые чужие байты) и G4b (нет PCM/ключей в логах; ctl-типы).

## 9. Риски

| # | Риск | Митигация |
|---|---|---|
| R-1 | **«KAT ≠ совместимость с Discord»**: наш поток валиден по RFC, но bitrate/application/DTX выбраны не как у клиента | §10 явно; финальные настройки — только live; KAT не считается доказательством слышимости |
| R-2 | Вариадический `*_ctl`: неверная ширина/тип аргумента — UB без ошибки компиляции | один хелпер на запрос, белый список, meta-grep = 0, ревью G4b (I-6) |
| R-3 | `#[link(name="opus")]` без feature ломает сборку на чужом хосте | feature off by default, модуль под `#[cfg]`, фаза передаёт флаг явно |
| R-4 | Тесты под feature **не компилируются** в обычном прогоне и «незаметно отсутствуют» | hard-фаза в `commonPhases`; `failOnSkips`; `discord-media`-часть всегда в `workspace-rust` |
| R-5 | Нативный энкодер ffmpeg `exp` и `-opus_delay` default 360 мс → не 50 пакетов на 1 с | генератор **падает** при `packet_count != 50`; `-opus_delay 2.5`; версия и cmdline в манифесте; байты пинуются, ffmpeg в CI не участвует |
| R-6 | Порог SNR подобран под результат = скрытое ослабление | правило `max(кандидат, измеренное−3 дБ)` один раз, запись в `KAT-OPUS.json`, заморозка после D-113 |
| R-7 | Кросс-машинный хэш пакетов (RTCD/SIMD) даёт ложный красный | байтовое равенство только «в процессе» (hard) и `#[ignore]`-тест на каноническом хосте |
| R-8 | Задержка кодека без выравнивания → ложный провал на 1 кГц | lag-поиск, допуск ±1 к `OPUS_GET_LOOKAHEAD`, стабильность на 3 прогонах |
| R-9 | `opus_packet_get_nb_channels` без `len` → чтение за границей на пустом пакете | инвариант I-5 + fuzz-подобный T-OPUS-07 (пустой пакет) |
| R-10 | `null`-PCM «проходит», но реальный `default` (=PipeWire) ведёт себя иначе (частичные чтения, xrun) | fake-backend с частичными чтениями в гейте; `default` — только live-окно W-1 |
| R-11 | `hw:*` в обход PipeWire ломает чужие сессии/эксклюзивный захват | запрет в плане и в тестах: только `null` и `default` |
| R-12 | Portal не даёт согласия на микрофон (H-10) — «consent» может быть ошибочно ожидаем | зафиксировано: mic consent = явное действие UI + ACL; портал только экран/камера/PTT |
| R-13 | PTT нельзя закрыть до `Value::U64` (P-1) | P-1 отдельной карточкой **до** PTT; до этого PTT только UI-фейк |
| R-14 | `blocked` по фокусу окна непроверяем (F-4) | переопределить `blocked` = нет сессии/отказ/нет триггера; фокус — UI-фейк |
| R-15 | Приватность: имена устройств/`secret_key`/PCM в логах | I-11, meta-grep `ui/**`+`rust/**`, review G4b |
| R-16 | FEC-кейс может не дать 960 на первом пакете → «подгонка» | `#[ignore]` с причиной + обязательная запись `fec_available`, решение PM |
| R-17 | Дрейф доков: `lib.rs:9-10` voice до сих пор утверждает «сетевых операций нет» | читать код, не шапки; правка шапки — G1 `[P]` |
| R-18 | Дублирование PT-констант (960/48000/1275) в трёх крейтах | единственный источник — `discord-media::opus`; `discord-voice`/platform импортируют |

## 10. Blocked-by-live (окончательный список)

| # | Что | Почему нельзя офлайн | Разблокирует |
|---|---|---|---|
| B-1 | **UDP**: discovery, ответ сервера, NAT/порт, ротация сокета +60 с | нужен живой медиа-сервер; loopback не доказывает формат ответа | live-окно W-1 |
| B-2 | **SRTP**: схема KDF/nonce/AAD (B-08) + `secret_key` из op4 (вырезан) | ни ключа, ни схемы; текущая — изобретённая (`crypto/kdf.rs:1-10`) | live + RE ключа |
| B-3 | **Реальный голос**: слышимость, задержка, потери, джиттер | требует двух клиентов и согласия | live W-2 (2 клиента) |
| B-4 | **Opus-настройки Discord**: bitrate, `application`, DTX on/off, FEC, stereo/mono | в capture речи не было (только CN/DTX), настроек не видно | live W-2 + сравнение с эталоном |
| B-5 | **DAVE/MLS**: обязателен ли для слышимости | нет MLS-слоя и спецификации в репозитории | live W-1/W-2 |
| B-6 | **Микрофон/устройства**: права, consent, реальный capture, hot-plug | `null` не имеет ни прав, ни времени | live W-1 |
| B-7 | **PTT**: реальная клавиша, поведение бэкенда, «ограниченный режим» | keybind не наблюдался; нужен `Value::U64` + системный диалог | live W-1 (после P-1) |
| B-8 | **Ping/RTCP-статистика**: источник `93 ms`, потери | тела SR/RR шифрованы | live |
| B-9 | **Krisp/NS**: Linux-путь | только имя модели в логе | live/отдельная карточка |

Что **нельзя** утверждать после зелёного KAT: «голос работает», «Discord примет поток»,
«SRTP совместим», «DAVE не нужен», «PTT работает», «устройства выбираются как в Discord»,
«кросс-машинная побитовая воспроизводимость».

## 11. Вопросы PM (до кода)

| # | Вопрос | Варианты | Рекомендация |
|---|---|---|---|
| Q-1 | **D-113** отсутствует как решение, но на него ссылаются `TASKS:118-119`, `QUEUE:147`, `MASTER-PLAN:79` | создать запись решения / убрать ссылку | создать: mono, `OPUS_APPLICATION_VOIP`, bitrate default, DTX on, комплексность 10 |
| Q-2 | Порог KAT (SNR 20/12/10/8 дБ) | утвердить как контракт / калибровать до первого прогона | калибровка по правилу §3.7, заморозка после D-113 |
| Q-3 | Feature `system-opus` + фаза в `gate:worker` | hard-фаза / только release / ручной прогон | hard в `worker` (libopus есть; иначе гейт врёт) |
| Q-4 | `UNSAFE-REGISTRY.md` (файла нет) — кто заводит | Lead | Lead; G5 приносит 12 строк I-1…I-12 |
| Q-5 | `discord-media` в `[workspace.dependencies]` + зависимость `platform → media` | принять / держать типы в platform | принять: даёт always-on тесты без feature |
| Q-6 | ID тестов `T-OPUS-01…10` вместо занятых `T-VOICE-20…24` | новая серия / перенумеровать T-VOICE | новая серия (T-VOICE-20…23 уже заняты MLS/устойчивостью) |
| Q-7 | FEC-кейс (T-OPUS-08) обязателен в DoD? | да / `#[ignore]` до live | `#[ignore]` с причиной, обязательная запись `fec_available` |
| Q-8 | Стерео в MVP? | mono / stereo | mono до live: в capture речи не было, канал не подтверждён |
| Q-9 | «Канонический хост» для пинового хэша пакетов | этот хост / любой CI | этот хост + запись CPU/версии в `KAT-OPUS.json` |

## 12. Что этот документ не утверждает

KAT не запускался, libopus не линковался, фикстуры не создавались, `ffmpeg`-кодирование не
выполнялось, аудиоустройства не открывались, live/сеть/сборка не использовались. Ни одна строка
кода, тестов, `Cargo.*`, `fixtures/*`, `scripts/*` не изменена. Изменён только этот файл.

Отдельно: здесь нет утверждений о (а) wire-совместимости SRTP, (б) слышимости голоса,
(в) совпадении настроек кодека с Discord, (г) необходимости DAVE, (д) работоспособности
ALSA/PipeWire на реальных устройствах, (е) выполнении M3.
