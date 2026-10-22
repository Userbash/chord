# RE-REGIONS — регионы и latency-пробы для P4-11 (B07/RW-26)

Seat: RE-1. Дата: 2026-10-22. Метод: статический разбор DWARF/символов
`discord_voice.node` (Discord 1.0.165), офлайн-дампы. Живой процесс не подключался.

## 1. Подтверждённые данные

### 1.1. Список регионов и IP приходит в native как JSON

Файл `/home/sanya/re-deep/decoded/efc4f1b3263c1c0d_0` (716 Б, sha256
`79a49aaf…12d0a5`) — точный массив, который получает `RegionLatencyTester::PerformTest`:

```json
[{"region":"frankfurt","ips":["104.29.146.223:19308","104.29.147.107:19322","104.29.147.224:19341","104.29.147.94:19341","104.29.147.162:19338"]},
 {"region":"brussels","ips":["104.29.159.158:19306","104.29.154.190:19301","104.29.154.187:19335","104.29.154.193:19295","104.29.154.192:19329"]},
 {"region":"rotterdam","ips":["104.29.153.173:19315","104.29.153.114:19295","104.29.153.195:19310","104.29.157.194:19313","104.29.153.123:19333"]},
 {"region":"paris","ips":["104.29.151.114:19302","104.29.159.102:19314","104.29.151.87:19311","104.29.151.46:19324","104.29.144.5:19336"]},
 {"region":"milan","ips":["104.29.146.176:19336","104.29.146.158:19302","104.29.146.194:19317","104.29.146.45:19332","104.29.146.160:19317"]}]
```

Копия для тестов: `waves/RW-26/research/fixtures/regions-rtc.json`
(md5 `ffe52792025bf702696cfa38db06995f`). Ровно 5 регионов × 5 IP:port.

### 1.2. Географический порядок из READY

`/home/sanya/re-deep/gateway-run1/sanitized/by-type/002-READY.json`:

```json
"geo_ordered_rtc_regions": ["frankfurt", "brussels", "rotterdam", "paris", "milan"]
```

### 1.3. Реальный результат ранжирования (engine log)

`/home/sanya/re-deep/voice/engine-log-session1.txt:2`:

```text
[RTCLatencyTestManager] RTC region latency test completed, ranked regions are:
  frankfurt,rotterdam,paris,milan,brussels
```

Отличие от geo-порядка: `brussels` опущен с 2-го на 5-е место, `rotterdam` поднят на 2-е.
Это оракул для алгоритма ранжирования: **измеренная задержка важнее geo-порядка**.

### 1.4. Типы native (DWARF, read-only)

```cpp
struct discord::RtcRegionWithIps { string region; vector<string> ips; };

class discord::media::RegionLatencyTester {
  bool pendingFailure_;
  function<void(vector<string>)> completionCallback_;
  unique_ptr<uv::ThreadedEventLoop> networkLoop_;
  shared_ptr<uv::ThreadedEventLoop> workerLoop_;
  uv_udp_t* socket_;
  static const uint16_t kRtcWorkerPingPortRangeStart;   // значение не извлечено
  static const uint16_t kRtcWorkerPingPortRangeEnd;
  mt19937 randomGenerator_;
  uniform_int_distribution<int> randomPortGenerator_;
  vector<RegionWithRtts> regionsRtts_;
  vector<PingInfo> sentPings_;
  int remainingPings_;
  UvTimer pingTimeoutTimer_;
  ScopedTaskCancellation cancelFlag_;
};
struct PingInfo { int regionIndex; uint64_t sendMs; bool handled; };
struct RegionWithRtts { string region; vector<uint64_t> rtts; };
using LatencyRankedRegions = vector<string>;
```

Вывод: проба — UDP-датаграмма на `ip:port` региона; исходный порт выбирается случайно из
диапазона `kRtcWorkerPingPortRange*`; RTT считается по времени отправки и прихода ответа;
на регион копится **вектор** RTT; в callback уходит ранжированный список строк.

## 2. Чего в дампе нет (честные UNKNOWN)

1. **Payload latency-пробы** и точный размер датаграммы — в strace нет привязки
   `sendto` к `RegionLatencyTester`; константы портов не извлечены.
2. **Метрика ранжирования** (min / median / среднее / число успешных проб) — тип хранит
   `vector<uint64_t>`, но компаратор в дампе не восстановлен.
3. **Источник JSON `region→ips`** (какой REST/IPC-вызов) — известно только, что это
   массив на входе native-функции.
4. Соответствие `region` ↔ префикс хоста `c-<prefix><N>-<hash>.discord.media`
   (`frankfurt` ↔ `c-fra10`/`c-fra19` — совпадение по смыслу, не по подтверждённой таблице).

## 3. Решение для headless-модели (P4-11)

Так как payload пробы не подтверждён, карточка **не изобретает wire-формат**. Реализуется:

- строгий парсер `[{region, ips[]}]` (реальный fixture) и `geo_ordered_rtc_regions`;
- модель проб с инъекцией часов и транспорта: `RegionProbe` фиксирует `sent_at_ms`,
  `rtt_ms`; таймаут выкидывает пробу;
- ранжирование: **медиана успешных RTT**, затем geo-порядок, затем лексикографический
  порядок — self-consistent, помечено «не подтверждено capture»;
- `select_region` (P4-01) получает измеренные пары `(region, latency)` и manual override.

Wire-отправка UDP-пробы — отдельная карточка после capture с подтверждённым payload.
