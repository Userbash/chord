# ATTACH-RENDER-RE — схема вложения и правила рендера (AT-03)

Дата: 2026-10-30. Карточка: RE-AT03-SCHEMA. Ветка: `RW-40`.
Источник: read-only разбор `reference-oracle/` (web-бандл эталона + сохранённые ответы REST).
Продуктовый код, тесты и документы других карточек не менялись.

Метки: `[D]` — прямая цитата кода/дампа по смещению, `[S]` — следует из кода косвенно
(имя пропа, сигнатура, вывод из ветвлений), `[A]` — не подтверждено, gap.

## 0. Главное

| Вопрос AT-03 | Ответ | Метка |
|---|---|---|
| Из чего состоит объект `attachment` | `id, filename, size, url, proxy_url, content_type, content_scan_version`, опционально `width, height, placeholder, placeholder_version, original_content_type, duration_secs, waveform, flags, title` | `[D]` |
| Есть ли поле `ephemeral` у вложения | Нет. Эфемерность — флаг сообщения `EPHEMERAL=64` + CDN-путь `/ephemeral-attachments/` | `[D]` (отсутствие в модели и в 72 живых объектах) |
| Чем отличаются image/video/audio/file | Три разных механизма: (1) inline-медиа — по **расширению файла**, (2) gallery/embed — по **MIME**, (3) file-card — по MIME+имени через таблицу `klass` | `[D]` |
| Как отличить voice message | Флаг сообщения `IS_VOICE_MESSAGE=8192`, не поля вложения | `[D]` |
| Что делать с `flags` | Битовая маска: `IS_CLIP=1, IS_THUMBNAIL=2, IS_REMIX=4, IS_SPOILER=8, CONTAINS_EXPLICIT_MEDIA=16, IS_ANIMATED=32, CONTAINS_GORE_CONTENT=64, CONTAINS_SELF_HARM_CONTENT=128, HAS_TIMELINE_COMMENTS=256` | `[D]` |
| CDN-подписи | `url`/`proxy_url` уже содержат `?ex=&is=&hm=`; клиент сам продлевает через `POST /attachments/refresh-urls` | `[D]` |
| Blurhash | `placeholder` + `placeholder_version`, декод только при `placeholder_version === 1` | `[D]` |
| Чего не хватает | Разметка контейнера (`d2`/`d3`/`dJ` — ленивые чанки) и точные max-размеры сетки: `[A]` | gap |

Оговорка по версии: рендер снят с web-бандла `web.4d4b2f75fdea5470.js` (1.0.162, ключ `B162`
в `PARITY/FUNCTION-INVENTORY.md`). Desktop-часть репозитория подтверждена на 1.0.166, поэтому
это тот же `version-gap`, что уже зафиксирован в инвентаре: схема протокола от версии не зависит,
разметка компонентов — зависит.

## 1. Источники и как проверялось

| Артефакт | Что даёт | Размер / md5 |
|---|---|---|
| `decoded/4d2a64cb5df5fc4e_0` = `assets/web.4d4b2f75fdea5470.js` | модель, классификаторы, компоненты вложения (модуль `684519`), redaction, CDN-refresh | 12 410 287 / `934b1289b7ebf17bc0fd09569b23b438` |
| `decoded/48550b11930370fb_0` = `assets/52bdd7060b270ca4.js` | `renderAttachments()` рендерера сообщения | 260 303 / `af6237f3c02c5fa412c90d1146aedc21` |
| `decoded/e0f9541dbaca2408_0` = `assets/5a4ab6813e0e0256.js` | резолвер причины скрытия медиа (spoiler/explicit) | 2 391 / `0a56c5d5616e1c1c1cb35da5c0bd1cef` |
| `decoded/5c14801cf5025f71_0` = `assets/a2a9a9bfb278ea88.js` | сборщик элементов медиа-галереи (копия `256265`) | 3 762 / `d6e8d0cb61b8dc895302a5e3d2636535` |
| `decoded/510c13959205dd46_0` = `assets/865429.a8014fba4482a736.js` | карта CSS-классов иконок файлов (`document…unknown`) | 3 175 / `4ab61ee533750fac46a0431320ca6d34` |
| `decoded/1b4e8eb40ee58eb4_0` и ещё 15 файлов | реальные ответы `GET /api/v9/channels/{id}/messages` с 72 вложениями | см. §2 |

Статистика по живым объектам: **72 вложения в 16 снапшотах каналов**. Частота полей:
`id/filename/size/url/proxy_url/content_type/content_scan_version` — 72/72;
`width/height/placeholder/placeholder_version` — 68/72; `original_content_type` — 35/72;
`duration_secs` — 7/72; `flags` — 4/72 (все `32`); `title` — 1/72; `waveform` — 1/72.

## 2. Схема объекта `attachment` в сообщении

### 2.1 Поля, подтверждённые живыми ответами

| Поле | Тип | Обяз. | Смысл | Пример `[D]` |
|---|---|---|---|---|
| `id` | string (snowflake) | да | id вложения; в CDN-пути совпадает | `"1561379697533911060"` |
| `filename` | string | да | исходное имя, включая расширение | `"vkclips_20261015062453.mp4"` |
| `size` | int | да | размер в байтах | `9256495` |
| `url` | string | да | прямая ссылка CDN с подписью | `https://cdn.discordapp.com/attachments/…?ex=6ad58db7&is=6ad43c37&hm=…&` |
| `proxy_url` | string | да | прокси `media.discordapp.net` с той же подписью | `https://media.discordapp.net/attachments/…` |
| `content_type` | string | да | MIME, может содержать `; charset=utf-8` | `"text/plain; charset=utf-8"` |
| `content_scan_version` | int | да | версия сканера контента; `1` или `4` в дампе | `4` |
| `width` / `height` | int | нет | пиксели; есть у image/video, нет у audio/file | `720 / 1280` |
| `placeholder` | string | нет | blurhash-строка (base64) | `"FAgGDAJN+lpnaGOIqRgCdTVAWA=="` |
| `placeholder_version` | int | нет | версия формата placeholder; в дампе всегда `1` | `1` |
| `original_content_type` | string | нет | MIME до перекодирования сервером | `image/png` при `content_type: image/webp` |
| `duration_secs` | float | нет | длительность медиа, секунды (float, не int) | `36.0955924987793` |
| `waveform` | string | нет | base64 амплитуд, только у voice message | `"AAAAAAAAAAAAABwnJBArEiAn…"` |
| `flags` | int | нет | битовая маска, см. §3 | `32` у анимированных GIF |
| `title` | string | нет | заголовок клипа/записи | `"2025-06-05 12.17.00"` |
| `description` | string | нет | alt-текст, заданный автором | в выдаче не встретился, поле читается кодом `[D]` |

Полный живой объект (видео) `[D] decoded/2c1c34dc2c6678f0_0`:

```json
{"id":"1560312729041113179","filename":"vkclips_20261015062453.mp4","size":9256495,
 "url":"https://cdn.discordapp.com/attachments/1058348665342808104/1560312729041113179/vkclips_20261015062453.mp4?ex=6ad58db7&is=6ad43c37&hm=33319bbe…&",
 "proxy_url":"https://media.discordapp.net/attachments/1058348665342808104/1560312729041113179/vkclips_20261015062453.mp4?ex=6ad58db7&is=6ad43c37&hm=33319bbe…&",
 "width":720,"height":1280,"duration_secs":36.0955924987793,
 "content_type":"video/mp4","original_content_type":"video/mp4",
 "content_scan_version":4,"placeholder":"FAgGDAJN+lpnaGOIqRgCdTVAWA==","placeholder_version":1}
```

Голосовое сообщение `[D] decoded/50f2a9f76680271b_0` — без `width/height/placeholder`,
зато с `duration_secs` и `waveform`:

```json
{"id":"1359580913469751459","filename":"voice-message.ogg","size":26921,
 "duration_secs":8.119976997375488,
 "waveform":"AAAAAAAAAAAAABwnJBArEiAnJQAAACQTDBkAEg4DJycgIBsDDgMBAQAADBsOExkDIBUeDhwOGxAQEwcSEgUAAAAAAAAAAAAAAAAAAAAAAAAA",
 "content_type":"audio/ogg","content_scan_version":1}
```

Файл без медиа-полей `[D] decoded/861c87b0200e0786_0`:

```json
{"id":"1434205937689952266","filename":"vbs.bat","size":2563,
 "url":"…/vbs.bat?ex=6ad5919d&is=6ad4401d&hm=…&","proxy_url":"…",
 "content_type":"application/x-msdos-program; charset=utf-8","content_scan_version":4}
```

### 2.2 Диапазоны значений, которые реально приходят

* `content_type`: `image/png` (39), `image/jpeg` (9), `video/mp4` (8), `image/webp` (6),
  `image/gif` (4), `video/quicktime` (2), `audio/ogg` (1), `application/x-msdos-program; charset=utf-8` (1).
* `original_content_type`: `image/png` (22), `video/mp4` (7), `image/gif` (4), `text/plain` (1),
  `application/x-zip-compressed` (1). Поле есть только когда сервер перекодировал файл
  (например, `image/png` → `image/webp`).
* `content_scan_version`: `4` (70 объектов), `1` (2 объекта).
* `flags`: единственное ненулевое значение в дампе — `32` (`IS_ANIMATED`) на `image/gif`.

Следствие для парсера: `content_type` может содержать параметры (`; charset=utf-8`), поэтому
сравнение на равенство ломается. Клиент сравнивает первый сегмент до `/` `[D] @6056280`:

```js
function a(e,t){if(null==e)return!1;let[n,i]=e.split("/");return n===t}
```

### 2.3 Как объект попадает в клиент

Модель сообщения сохраняет массив как есть, без нормализации в camelCase `[D] @8920304`:

```js
this.attachments=e.attachments??[]
```

REST-снапшоты и gateway идут одним и тем же путём: `MESSAGE_CREATE` кладёт `message.attachments`
в запись, `MESSAGE_UPDATE` замещает массив целиком (в `shouldComponentUpdate` сравнение идёт
по ссылке: `o.attachments!==s.attachments` `[D] 48550@219809`).

Отдельно: в gateway-дампе этой волны `attachments` был `null` во всех 10 событиях
(`docs/re/gateway-schema.md:438`), поэтому схема восстановлена по REST-ответам, а не по шлюзу.

### 2.4 Клиентские поля, которых нет в ответе сервера

| Поле | Откуда | Метка |
|---|---|---|
| `loading_state` | ставит клиент при оптимистичной отправке (`LOADING`/`LOADED_SUCCESS`), читается как `e.loading_state` в `Uv` | `[D] @5917760` |
| `content_scan_metadata: {version, flags}` | ставит клиент (компоненты медиа, оптимистичная отправка); в ответах сервера не встречается | `[D] @5917760` |
| `content_scan_version: -1` | служебный сентинел «не просканировано»; клиент проставляет его принудительно в `resetContentScan` | `[D] @4564304`, `[D] @4544787` (`Ae=-1`) |

### 2.5 `ephemeral`, `is_spoiler`, `uploaded_filename`

* Поля `ephemeral` у вложения в этой версии нет: ни в модели, ни в 72 живых объектах.
  Эфемерность приходит флагом сообщения `EPHEMERAL=64` `[D] @254936`, а CDN-путь такого
  вложения — `/ephemeral-attachments/…`, который клиент явно перечисляет `[D] @6046114`:

  ```js
  let i=new Set(["/attachments/","/ephemeral-attachments/"])
  ```

* `is_spoiler` — это **исходящее** имя. Сервер отдаёт бит `IS_SPOILER=8` в `flags`, а клиент
  при `PATCH` сообщения отправляет `{attachments:[{id, description, is_spoiler}]}`
  `[D] 48550@227557` (`patchMessageAttachments`):

  ```js
  patchMessageAttachments:async(e,t,n)=>(await em.A.unarchiveThreadIfNecessary(e),
    s.Bo.patch({url:D.Rsh.MESSAGE(e,t),body:{attachments:n},…}))
  ```

* `uploaded_filename` живёт только в запросе `POST /channels/{id}/messages`; в ответе и в
  событиях его нет (подробности отправки — `ATTACH-RE.md §2`, `reference-oracle-REA.md §1.7`).

## 3. `flags` — полная маска

`[D] @255684` (enum `sbO`, он же `AttachmentFlags`):

```js
nl=((v={})[v.IS_CLIP=1]="IS_CLIP",v[v.IS_THUMBNAIL=2]="IS_THUMBNAIL",v[v.IS_REMIX=4]="IS_REMIX",
 v[v.IS_SPOILER=8]="IS_SPOILER",v[v.CONTAINS_EXPLICIT_MEDIA=16]="CONTAINS_EXPLICIT_MEDIA",
 v[v.IS_ANIMATED=32]="IS_ANIMATED",v[v.CONTAINS_GORE_CONTENT=64]="CONTAINS_GORE_CONTENT",
 v[v.CONTAINS_SELF_HARM_CONTENT=128]="CONTAINS_SELF_HARM_CONTENT",
 v[v.HAS_TIMELINE_COMMENTS=256]="HAS_TIMELINE_COMMENTS",v)
```

| Бит | Имя | Где используется в рендере |
|---|---|---|
| 1 | `IS_CLIP` | клип-медиа: `F(e)` отбирает вложения с этим битом, а клип-инфо строится из `duration_secs`+`clip_sync_timestamp` (`application.id` обязателен) `[D] @3643916` |
| 2 | `IS_THUMBNAIL` | такие вложения **исключаются** из inline-списка (`filter(e=>null==e.flags||!IS_THUMBNAIL(e.flags))`) `[D] 48550@223939`; в медиа-канале, наоборот, выбирается как превью `[D] 5c14801@2773` |
| 4 | `IS_REMIX` | объявлен; потребителя в бандле нет `[D]` |
| 8 | `IS_SPOILER` | причина скрытия `SPOILER` `[D] 48550@223939`, `[D] e0f9541@296` |
| 16 | `CONTAINS_EXPLICIT_MEDIA` | redaction: `attachmentFlag` для harm-типа `explicit` `[D] @4552599` |
| 32 | `IS_ANIMATED` | автоплей GIF/анимированного webp/avif: `srcIsAnimated` `[D] @5918150` |
| 64 | `CONTAINS_GORE_CONTENT` | redaction: harm-тип `gore` `[D] @4552599` |
| 128 | `CONTAINS_SELF_HARM_CONTENT` | redaction: harm-тип `self_harm` `[D] @4552599` |
| 256 | `HAS_TIMELINE_COMMENTS` | комментарии к таймлайну клипа; потребителя в бандле не нашли `[D]` |

Рядом лежат два «подмножества» той же маски: `no` — только `IS_ANIMATED=32` (для embed-медиа),
`nd` — `16/32/64/128` (для сканированного контента) `[D] @256080`.

Флаг сообщения, который влияет на вложения: `IS_VOICE_MESSAGE=8192` `[D] @254936`.

## 4. Классификация image / video / audio / file

В клиенте три независимых классификатора — их нельзя путать, они дают разные ответы на одних
и тех же данных.

### 4.1 Inline-медиа: только по расширению файла

`[D] @5918776` (модуль `731068`, функция `Rr`):

```js
function I(e,t){return{...h(e),
  type:(0,o.u)(e.filename)?"IMAGE":(0,o.AE)(e.filename)?"VIDEO":"INVALID",
  alt:e.description,
  sourceMetadata:{message:t,identifier:{type:"attachment",attachmentId:e.id,
    filename:e.filename,title:e.title,size:e.size}}}}
```

`o.u` / `o.AE` — из модуля `59318` `[D] @6056405`:

```js
let s=/\.(png|jpe?g|jfif|webp|gif|heic|heif|dng|avif)$/i;
function l(e){return r(e,s)}          // Zj — картинка (с отбрасыванием ?query)
function o(e){return null!=e&&s.test(e)}   // u — картинка по сырому имени
…
h=(isIOS)?/\.(mp4|mov|qt)$/i:(isAndroid)(),/\.(mp4|webm|mov|qt)$/i;
function f(e){return r(e,h)||!1}      // r1 — видео (с отбрасыванием ?query)
function I(e){return null!=e&&(h.test(e)||!1)}  // AE — видео по сырому имени
```

Важно: inline-путь смотрит **только на `filename`**. Вложение `image/png` без расширения
попадёт в `INVALID` и не отрисуется как картинка, хотя `content_type` корректный.

### 4.2 Тип по MIME (галерея, embed, компоненты)

`[D] @5918631` (`FE`):

```js
function f(e){return(0,o.tT)(e.contentType)?"IMAGE"
  :(0,o.XB)(e.contentType)&&null!=e.proxyUrl&&null!=d.A.toURLSafe(e.proxyUrl)?"VIDEO"
  :"INVALID"}
```

где `tT(e)= a(e,"image")`, `XB(e)= a(e,"video")` — сравнение первого сегмента MIME `[D] @6056280`.
Видео дополнительно требует парсящийся `proxy_url` (постер грузится с прокси).

### 4.3 Что считается медиа-вложением вообще (гейт галереи)

`[D] 5c14801@390` (модуль `256265`, копия в основном бандле `@4675094`):

```js
function S(e){if(null==e)return!1;let{filename:n,height:t,width:l}=e;
  return(0,o.u)(n)&&null!=t&&t>0&&null!=l&&l>0}          // картинка + размеры
function M(e){return null!=e&&null!=e&&(0,o.AE)(e.filename)&&null!=e.proxy_url}  // видео + proxy
function V(e){return S(e)||M(e)}
```

То есть для галереи картинка без `width/height > 0` отбрасывается, а inline-путь таких
требований не ставит `[D] 48550@223939`.

### 4.4 File-card: таблица `klass` по MIME и имени

`[D] @9704193` (модуль `453771`, `TX` = `classifyFile(name, type)`):

```js
let I=[{reType:/^image\/vnd.adobe.photoshop/,klass:"photoshop"},
 {reType:/^image\/svg\+xml/,klass:"webcode"},
 {reType:/^image\//,klass:"image"},
 {reType:/^video\//,klass:"video"},
 {reName:/\.pdf$/,klass:"acrobat"}, … {reName:/\.ae/,klass:"ae"},
 {reName:/\.sketch$/,klass:"sketch"}, {reName:/\.ai$/,klass:"ai"},
 {reName:/\.(?:rar|zip|7z|tar|tar\.gz)$/,klass:"archive"},
 {reName:/\.(?:c\+\+|cpp|cc|c|h|hpp|mm|m|json|js|ts|…|sh|ml|actionscript)$/,klass:"code"},
 {reName:/\.(?:txt|rtf|doc|docx|md|pages|ppt|pptx|pptm|key|log)$/,klass:"document"},
 {reName:/\.(?:xls|xlsx|numbers|csv)$/,klass:"spreadsheet"},
 {reName:/\.(?:html|xhtml|htm|xml|xsd|css|styl)$/,klass:"webcode"},
 {reName:/\.(?:mp3|ogg|opus|wav|aiff|flac)$/,klass:"audio"}];
function p(e){return T(e.name,e.type)}
function T(e,t){e=e?.toLowerCase()??"";let n=s().find(I,n=>null!=n.reType&&null!=t?n.reType.test(t)
  :null!=n.reName&&""!==e&&n.reName.test(e));return null!=n?n.klass:"unknown"}
```

Набор `klass` совпадает с картой CSS-классов иконок `[D] 510c13959205dd46_0@124`:
`document, image, code, video, acrobat, ae, sketch, ai, archive, photoshop, ps, spreadsheet,
webcode, audio, unknown`. Иконка файла грузится как `./icon-file-${klass}.svg` `[D] @3542276`.

Тонкости: SVG и Photoshop классифицируются **не** как `image`, а как `webcode`/`photoshop`;
аудио определяется только по расширению (MIME `audio/*` в таблице отсутствует).

### 4.5 Сводка решений

| Данные | Inline-медиа (`Rr`) | Галерея (`S`/`M`) | File-card (`TX`) | Аудио-плеер |
|---|---|---|---|---|
| `image/png`, `cat.png` | IMAGE | медиа | `image` | — |
| `image/png`, `cat` (без расширения) | INVALID → не рисуется | не медиа | `image` | — |
| `image/svg+xml`, `logo.svg` | INVALID | не медиа | `webcode` | — |
| `video/mp4`, `a.mp4` + proxy | VIDEO | медиа | `video` | — |
| `video/mp4`, без расширения | INVALID | не медиа | `video` | — |
| `audio/ogg` + `waveform` + флаг сообщения 8192 | INVALID (не медиа) | не медиа | `audio` | voice-плеер `[S]` |
| `audio/mpeg`, `track.mp3` | INVALID | не медиа | `audio` | обычный аудио-плеер `[S]` |
| `text/plain`, `message.txt` | INVALID | не медиа | `document` | превью текста `[S]` |
| `application/zip`, `x.zip` | INVALID | не медиа | `archive` | file-card |

## 5. Пайплайн inline-рендера (точка входа AT-03)

`[D] 48550@223939`, метод `renderAttachments(e)`:

```js
renderAttachments(e){let{channel:t,gifAutoPlay:n,canDeleteAttachments:i,inlineAttachmentMedia:l,
  onMediaItemContextMenu:r,enabledContentHarmTypeFlags:a,shouldHideMediaOptions:o}=this.props,
  {attachments:d,flags:u}=e;
  if(0===d.length||e.isPoll()||(0,r1._c)(e))return null;
  let m=d.filter(e=>null==e.flags||!(0,c.Lt)(e.flags,O.sbO.IS_THUMBNAIL));
  if(0===m.length)return null;
  let h=m.map(t=>({...(0,l_.Rr)(t,e),original:t.url,
      srcIsAnimated:(0,c.Lt)(t.flags??0,O.sbO.IS_ANIMATED)}))
    .filter(e=>"INVALID"!==e.type),
    {srcToOnClickOverride:g,srcToHandlePreloadImage:p,srcToSeekOverride:A}
      =(0,lJ.o)(h,{enabledContentHarmTypeFlags:a,shouldHideMediaOptions:o},"Media Mosaic"),
    x=h.length>1,
    f=(0,c.Lt)(u,O.pr7.IS_VOICE_MESSAGE);
  function I(e,n){let i=(0,c.Lt)(e.originalItem.flags??0,O.sbO.IS_SPOILER)||(0,sz.BV)(t);
    return(0,r9.iW)(e.originalItem,n,i)}
  let E=m.map(s=>{let a=(0,l_.aG)(s),o={message:e,item:(0,d3.rC)(s,l),autoPlayGif:n,
    canRemoveItem:i&&(m.length>1||""!==e.content),onRemoveItem:this.handleRemoveAttachment,
    canEditItem:i,onEditItem:this.handleEditAttachment,getObscureReason:I,
    onContextMenu:…,renderAudioComponent:f?dJ.SX:dJ.Nj,renderImageComponent:dJ.Cr,
    renderVideoComponent:dJ.I1,renderPlaintextFilePreview:dJ.R6,
    renderGenericFileComponent:dJ.UB,onPlay:…,gifFavoriteButton:cs(a),
    allowFullScreen:!this.props.disableComponentInteractivity},d=(0,r3.E)({proxyURL:s.proxy_url,url:s.url});
    … return o});
  return(0,s.jsx)(d2.A,{items:E})}
```

Что это даёт для контракта:

1. Вложения рендерятся, только если массив непустой, сообщение не опрос и не подпадает под
   `_c` (модуль `383233`; в дампе его нет, точный набор условий — `[A]`).
2. `IS_THUMBNAIL`-вложения выпадают из списка.
3. В список медиа-элементов (`h`, он же определяет `x=h.length>1` — одиночное вложение или
   сетка) попадают только `type !== "INVALID"`, т.е. IMAGE/VIDEO по **расширению**.
4. Все вложения (и медиа, и файлы) получают общий набор render-пропов; конкретный компонент
   выбирается по `item` внутри `d2` — это `[S]`, сам контейнер в дампе отсутствует.
5. Для аудио компонент выбирается **флагом сообщения** `IS_VOICE_MESSAGE`: `dJ.SX` (voice,
   с waveform) против `dJ.Nj` (обычный плеер).
6. Причина скрытия: `IS_SPOILER(flags) || BV(channel)` и дальше `r9.iW` — см. §7.
7. `inlineAttachmentMedia=false` уходит в `d3.rC(s, l)` — режим «только ссылка» `[S]`
   (имя пропа и передача настройки видны, сама ветка в ленивом чанке).

Модули `d2` (`394839`), `d3` (`96782`), `dJ` (`706403`), `r1` (`383233`) в дампе отсутствуют —
это и есть единственный существенный gap по AT-03.

## 6. Leaf-компоненты (есть в бандле, модуль `684519`)

| Роль | Функция | Ключевое | Метка |
|---|---|---|---|
| Картинка | `tx` (`LL`) | LazyImage + кнопка alt-текста, `mediaLayoutType` (`STATIC`/`RESPONSIVE`/`MOSAIC`), `hiddenSpoilers` | `[D] @6144343` |
| Видео | `j` (`248643`) | постер через LazyImage, `forceExternal` при сторонах > `6016×3384` | `[D] @5929320` |
| Аудио | `tw` (`gL`) | MediaPlayer `type: AUDIO`, `autoMute:()=>!1`, ссылка на скачивание скрыта | `[D] @6143840` |
| Voice message | `tG` (`P$`) | плеер с canvas-waveform, `durationSecs`, `playbackCacheKey` | `[D] @6144184`, canvas `@6133343` |
| Текст/код | `tF` (`_d`) | превью `Range: bytes=0-50000`, разметка, язык, «Развернуть» | `[D] @6145355` |
| Файл-карточка | `tB` (`Dk`) | `G.A` (`564771`): иконка+имя+размер | `[D] @3543521` |
| Строка «скрыто» | `tz` (`yR`) | заголовок + «Нажмите, чтобы узнать больше» (`515vjG`) | `[D] @6157702` |

Файл-карточка `[D] @3543521`:

```js
let O=function(e){let{className:t,url:n,fileName:r,fileSize:a,onClick:l,onContextMenu:d,
  renderAdjacentContent:c}=e;return … <N filename={r}/>
  … <Anchor className={S.AD} href={n} onClick={l} onContextMenu={d}>{r}</Anchor>
  … <Text variant="text-xs/normal" color="text-muted">{filesize(a)}</Text> …}
```

Иконка `[D] @3542276`: `r=(0,m.GD)(t); require(\`./icon-file-${r}.svg\`)`, где `GD` — тот же
`classifyFile`. Размер в карточке — библиотека `filesize` `[D] @9705272`; в других местах
(заголовок текстового превью, плеер) — локализованный форматтер `up`: `< 1 МБ` →
`ceil(bytes/1000)` КБ, иначе `round(10*bytes/1e6)/10` МБ `[D] @9703523`.

Текстовое превью `[D] @6125741`:

```js
let n=await fetch(e,{headers:{Range:"bytes=0-50000",Accept:"text/plain"}}), …
  new TextDecoder(charset) … // charset из content_type, дефолт utf-8
… s((0,ed.sJ)(u)), o(c)       // остаток байт = total - content-length
```

Показывается 6 строк в свёрнутом виде, до 100 в развёрнутом, для `.md` доступна отрисовка
markdown; кнопка «Загрузить» (`1WjMbC`) — обычная ссылка на CDN.

## 7. Spoiler, explicit media и причина скрытия

### 7.1 Резолвер причины

`[D] e0f9541dbaca2408_0@296` (модуль `294520`):

```js
function d(e,t){let n=arguments.length>2&&void 0!==arguments[2]&&arguments[2],
  c=(0,E.b)({type:r.D.Attachment,media:e},t),
  O=(0,E.$V)({type:r.D.Attachment,media:e},t);
  return c.length>0?c[0]:O?i.Oc.POTENTIAL_EXPLICIT_CONTENT:n?i.Oc.SPOILER:null}
```

Приоритет: конкретный harm-флаг → `potential_explicit_content` (скан неполный) → `spoiler`.

Перечисление причин `[D] @4555078`:

```js
r=((i={}).SPOILER="spoiler",i.EXPLICIT_CONTENT="explicit_content",
  i.POTENTIAL_EXPLICIT_CONTENT="potential_explicit_content",
  i.GORE_CONTENT="gore_content",i.SELF_HARM_CONTENT="self_harm_content",i)
```

Текст для оверлея `[D] e0f9541dbaca2408_0@1816`: `explicit/gore/self_harm` → `SEgHFh`
(«Предупреждение о контенте…»), `spoiler` → `XpfDH+` («спойлер, скрыто»).

### 7.2 Какие вложения скрываются

`[D] @4557887` (модуль `282108`, `y` = «потенциально скрываемое»):

```js
function y(e,t){ …
  switch(e.type){
   case E.D.Attachment:
     return a=e.media, 0!==(l=c).length && 0!==l.filter(e=>!D(e,{type:E.D.Attachment,media:a})).length
       && P(a.content_scan_version??a.contentScanVersion,l);
   …}}
function D(e,t){ … case E.D.Attachment: return(0,i.Lt)(t.media.flags??0,n.attachmentFlag); …}
function P(e,t){let n=A.A.validContentScanVersion;
  return e!==I.Ae&&(t.includes(h.kn.GORE)||t.includes(h.kn.SELF_HARM)?null==e||e<n:null==e)}
```

Три функции делят работу так:

* `D(e,t)` — «у вложения стоит бит этого harm-типа» (`16/64/128`).
* `y(e,t)` — «есть подозрение, что вложение скрывать нужно, но скан не даёт ответа»:
  берутся включённые harm-типы, и проверка срабатывает, если хотя бы у одного **нет** своего
  бита (`filter(e=>!D(...))`), а версия скана при этом недостаточна по `P`.
* `P(e,t)` — версия скана: `-1` отсекается сразу как «не сканировано вручную»; для `gore`
  и `self_harm` недостаточной считается отсутствующая версия или версия ниже
  `validContentScanVersion`, для `explicit` — только отсутствующая.

Поэтому `y` даёт ровно тот случай, который показывается как `POTENTIAL_EXPLICIT_CONTENT`:
включённый harm-тип не подтверждён свежим сканом, и клиент перестраховывается.

`validContentScanVersion` `[D] @4551612`:

```js
validContentScanVersion(){return o.getConfig({location:"ExplicitMediaStore.validContentScanVersion"})
  .enabled? i??5 : Math.min(i??4,4)}
```

где `i` приходит из `CONNECTION_OPEN` (`explicitContentScanVersion` из READY).
Сентинел «не сканировано» — `-1` `[D] @4544787`.

### 7.3 Настройки и контекст

Конфиг harm-типов `[D] @4552599`:

| harm | `attachmentFlag` | `bitmask` | Настройки пользователя |
|---|---|---|---|
| `explicit` | `CONTAINS_EXPLICIT_MEDIA=16` | 1 | `explicitContentGuilds/FriendDm/NonFriendDm` |
| `gore` | `CONTAINS_GORE_CONTENT=64` | 2 | `goreContent*` |
| `self_harm` | `CONTAINS_SELF_HARM_CONTENT=128` | 4 | `selfHarmContent*` (по умолчанию `UNSET_EXPLICIT_CONTENT_REDACTION`) |

Контекст определяется автором и каналом `[D] @4559606` (`282108`, `M`): свой автор → `NONE`,
DM с другом → `friend_dm`, DM с не-другом → `non_friend_dm`, иначе `guild`.

Дополнительно канал может быть «спойлерным»: `BV(channel)` `[D] @7747374` возвращает `true`,
если канал (или родитель ветки) помечен spoiler и пользователь ещё не согласился его раскрыть.
Именно поэтому `renderAttachments` скрывает все вложения в таком канале.

### 7.4 Реакции UI

* Плашка «скрыто» с текстом причины и подсказкой «Нажмите, чтобы узнать больше» (`515vjG`),
  компонент `tz` `[D] @6157702`.
* Для файлов отдельные строки: `3Gc2XP` «Файл скрыт под спойлером», `sb2W2J`/`ehBaMc` —
  изображение/видео, `C8ci33` — блок, `cuurzA` — «Вложение-спойлер» `[D] ru-catalog.json`.
* Кнопка alt-текста: `fSiQ3A` (aria), `jCV1Tz` («ПОДСКАЗКА»), бейдж `QEW81z` («Alt»)
  `[D] @6142284` (`tM`), `[D] @6144343` (`tx`).
* Жалоба «это не explicit»: `POST /attachments/report-false-positive`
  `{channel_id, message_id, attachment_ids, embed_ids}` и
  `POST /attachments/sender-report-false-positive` `{…, filenames}`
  `[D] @4544038`; повторный скан — `PATCH /channels/{id}/explicit-media` `{message_ids}`,
  `PATCH /messages/explicit-media` `{messages:[{channel_id,message_id}]}` `[D] @4544038`.

## 8. CDN, прокси, refresh и placeholder

### 8.1 Две ссылки

`url` → `cdn.discordapp.com`, `proxy_url` → `media.discordapp.net`. Обе приходят уже
подписанными: `?ex=<hex>&is=<hex>&hm=<hex>&` (см. живые объекты §2.1). Клиент берёт строки
из ответа и правит только параметры.

| Правило | Код | Метка |
|---|---|---|
| Видео-постер — `proxy_url` + `format=webp` | `e.searchParams.append("format","webp")` | `[D] 5c14801@696`, `[D] @4675551` |
| Для Discord CDN параметры ресайза не добавляются | `if(t.startsWith("data:image")||u.A.isDiscordCdnUrl(t))return t;` | `[D] @5728479` |
| Для не-CDN — `width/height/format/quality/animated`, ограничение стороны `4096` | `(0,d.Uj)({width,height,maxWidth:A.uJv,maxHeight:A.uJv})`, `uJv=4096` | `[D] @5728479`, `[D] @531318` |
| Снятие подписи | `w6` удаляет `ex`, `is`, `hm` | `[D] @6046342` |
| Просрочка | `ex` парсится как hex-секунды; refresh нужен, если `ex <= now + 1 час` | `[D] @6046342` (`f`) |
| Продление | `POST /attachments/refresh-urls` `{attachment_urls:[url]}` → `body.refreshed_urls[0].refreshed` | `[D] @6047505`, `[D] @308839` |
| Эфемерные вложения | путь `/ephemeral-attachments/` входит в тот же набор для refresh | `[D] @6046114` |

Проверка устроена в два уровня: `S(e)` смотрит одну ссылку и продлевает её, если `ex` пуст
или истекает в пределах часа; `m(e)` отвечает на вопрос «есть ли в сообщении хоть одна такая
ссылка во вложениях или embeds» и используется как быстрый гейт `[D] @6047505`.

### 8.2 Placeholder (blurhash)

`[D] @841846`:

```js
let i = n && 1===t && null!=e;   // n = загрузка, t = placeholderVersion, e = placeholder
… new Promise(t=>{ … d.postMessage({id:n,placeholderData:e}) }) … .then(png => …)
```

То есть `placeholder` — blurhash, декодируется воркером (`n.u("314250")`) в PNG и показывается
подложкой, пока грузится настоящее изображение. Условие — строго `placeholder_version === 1`;
при других версиях поле игнорируется.

### 8.3 Загрузка изображения

`[D] @5920611` (модуль `619517`, класс LazyImage):

* ленивая загрузка через `IntersectionObserver` (`rootMargin: "300px"`);
* `isAnimated` = флаг `animated` либо расширение `.gif`, либо `srcIsAnimated` + `.webp/.avif`;
* формат: если платформа умеет webp (`canUseWebp()`), не-анимированные картинки просятся
  как `format=webp`, PNG/AVIF — с `quality=lossless`; «замороженная» картинка — `format=png`;
* битые картинки: текст `sAfT1K` «Не удалось загрузить изображение.», при отсутствии alt —
  `X4IxWL` «Изображение».

## 9. Аудио, voice message, видео

### 9.1 Voice message

Флаг `IS_VOICE_MESSAGE=8192` у сообщения → компонент `dJ.SX` (`tG` → `tm`) `[D] 48550@223939`.
Плеер строит waveform из `waveform` `[D] @6133343`:

```js
function(e){let t;if(null==e)return;try{t=window.atob(e)}catch(e){return}
  let n=[];for(let e=0;e<t.length;e++)n[e]=t.charCodeAt(e)/255;return n}
```

Байты base64 → амплитуды `0..1`, число столбцов = `floor((width+4)/6)`, лишние сэмплы
усредняются, недостающие добиваются нулями. Таймер: `mm:ss`, при отсутствии длительности
`--:--`; формат `[D] @5935271`:

```js
function z(e){let t=0|e,n=t%60;return`${(t-n)/60}:${String(n).padStart(2,"0")}`}
```

### 9.2 Обычное аудио

MediaPlayer `type: AUDIO` `[D] @5954877`:

```js
renderMetadata(){let{fileName:e,fileSize:t,src:n,type:r,playable:a,mimeType:s}=this.props;
  return null==e||null==t?null:r===K.AUDIO
    ?<Z fileName={e} fileSize={t} src={n} disabled={!a} mimeType={s} hideDownloadButton={!0}/>
    :null}
```

То есть у аудио в плеере показываются имя и размер, кнопка скачивания скрыта; для видео
метаданных нет, зато есть `fileName/fileSize/mimeType/downloadable` на самом плеере.
Плеер выбирает `<audio>`/OGV: `""!==new Audio().canPlayType("audio/ogg; codecs=opus")`
`[D] @7656699`.

### 9.3 Видео

`[D] @5929320`: если пропорции не укладываются в `6016×3384` ни в одной ориентации —
`forceExternal`, т.е. вместо встроенного плеера открывается внешняя ссылка. Минимальные
размеры плеера — `static minWidth=150`, `static minHeight=110` `[D] @5943565`.
Постер — `proxy_url` + `format=webp`, `width/height` берутся из вложения.

## 10. Что это значит для AT-03 (контракт и оракулы)

1. Парсер сообщения обязан сохранять вложение как «сырой» объект: snake_case, без потери
   неизвестных полей. Нормализация допустима только на уровне представления.
2. Классификация для inline-ленты — по `filename` (расширение), а не по `content_type`.
   `content_type` нужен для галереи, file-card и выбора иконки.
3. `flags` — единственный источник `spoiler`/`thumbnail`/`animated`/`clip`/harm-битов.
   `is_spoiler` в JSON бывает только в исходящем `PATCH`.
4. Аудио-ветка зависит от флага **сообщения**, а не вложения.
5. CDN-ссылки не собираются заново: берём `url`/`proxy_url`, при истечении `ex`
   дергаем `POST /attachments/refresh-urls` и подменяем строку.
6. `placeholder` рендерим только при `placeholder_version === 1`; иначе — пустая подложка.
7. Что можно проверить тестом без сети (оракулы):
   * фикстуры из §2.1 (три живых объекта) → ожидаемые `type`, `spoiler`, размеры;
   * таблица `flags`: `32` на GIF → «анимированный», `8` → spoiler, `2` → не рендерим inline;
   * `width/height` отсутствуют у audio/file → галерея их пропускает, inline-путь — нет;
   * `content_type` c `; charset=utf-8` → классификация по первому сегменту;
   * `placeholder_version != 1` → blurhash не декодируется;
   * истёкший `ex` (раньше `now+1h`) → один POST refresh на URL.

## 11. Остаточные gap

| Пункт | Статус | Что нужно |
|---|---|---|
| Контейнер `d2` (`394839`), item-props `d3` (`96782`), карта компонентов `dJ` (`706403`) | `[A]` | ленивый чанк рендерера сообщений; в дампе 1.0.162 отсутствует, в live 1.0.167 тоже не снимался |
| Точные max-размеры сетки вложений и правило «одно вложение на всю ширину» | `[A]` | те же чанки |
| Предикат `_c(e)` (какие сообщения не рисуют вложения) | `[A]` | модуль `383233` |
| Поведение при `inlineAttachmentMedia=false` | `[S]` (по имени пропа) | визуальная проверка |
| Разметка DOM для inline-медиа | `[A]` | в `ui8/*` вложений в сообщениях нет, только inline-ссылки `attachmentLink` |
| `title`/`description` в UI помимо alt/clip | `[D]` частично | alt-текст подтверждён кодом, остальное — в ленивом чанке |

## 12. Артефакты и смещения цитат

Все смещения — байтовые от начала файла в `decoded/`. Короткие имена в цитатах:
`48550` = `48550b11930370fb_0`, `e0f9541` = `e0f9541dbaca2408_0`,
`5c14801` = `5c14801cf5025f71_0`, `510c` = `510c13959205dd46_0`;
остальные `@NNNN` без префикса — смещения в `4d2a64cb5df5fc4e_0`.

| Файл | Смещение | Что там |
|---|---|---|
| `4d2a64cb5df5fc4e_0` | 254936 | флаги сообщения (`EPHEMERAL=64`, `IS_VOICE_MESSAGE=8192`) |
| `4d2a64cb5df5fc4e_0` | 255684 | `AttachmentFlags` (`IS_CLIP…HAS_TIMELINE_COMMENTS`) |
| `4d2a64cb5df5fc4e_0` | 308839 | `ATTACHMENTS_REFRESH_URLS:"/attachments/refresh-urls"` |
| `4d2a64cb5df5fc4e_0` | 531318 | `uJv=4096` (лимит стороны для imgproxy-параметров) |
| `4d2a64cb5df5fc4e_0` | 841846 | условие `placeholderVersion===1` + воркер blurhash |
| `4d2a64cb5df5fc4e_0` | 3542276 | `icon-file-${klass}.svg` |
| `4d2a64cb5df5fc4e_0` | 3543521 | файл-карточка (иконка, имя-ссылка, `filesize`) |
| `4d2a64cb5df5fc4e_0` | 3643916 | клип: `duration_secs` + `clip_sync_timestamp` |
| `4d2a64cb5df5fc4e_0` | 4544038 | `POST /attachments/report-false-positive`, `PATCH …/explicit-media` |
| `4d2a64cb5df5fc4e_0` | 4544787 | `Ae=-1` (сентинел «не сканировано») |
| `4d2a64cb5df5fc4e_0` | 4551612 | `validContentScanVersion` |
| `4d2a64cb5df5fc4e_0` | 4552599 | конфиг harm-типов (`attachmentFlag`, `bitmask`) |
| `4d2a64cb5df5fc4e_0` | 4555078 | enum причин (`spoiler…self_harm_content`) |
| `4d2a64cb5df5fc4e_0` | 4557887 | `y()` — проверка «потенциально скрываемое» |
| `4d2a64cb5df5fc4e_0` | 4559891 | `P()` — сверка `content_scan_version` с `validContentScanVersion` |
| `4d2a64cb5df5fc4e_0` | 4564304 | `content_scan_version=-1` при сбросе скана |
| `4d2a64cb5df5fc4e_0` | 4675094 | `isMediaAttachment` (галерея, копия `256265`) |
| `4d2a64cb5df5fc4e_0` | 4675551 | сборщик элементов галереи, `format=webp` для постера |
| `4d2a64cb5df5fc4e_0` | 5728479 | сборка imgproxy-URL, `format/quality/animated/width/height` |
| `4d2a64cb5df5fc4e_0` | 5917760 | `Uv` — attachment → media item |
| `4d2a64cb5df5fc4e_0` | 5918150 | `aG` — media item + `contentScanMetadata` из флагов |
| `4d2a64cb5df5fc4e_0` | 5918631 | `FE` — тип по MIME |
| `4d2a64cb5df5fc4e_0` | 5918776 | `Rr` — тип по расширению |
| `4d2a64cb5df5fc4e_0` | 5920611 | LazyImage: `isAnimated`, webp/quality |
| `4d2a64cb5df5fc4e_0` | 5929320 | `forceExternal` при 6016×3384 |
| `4d2a64cb5df5fc4e_0` | 5935271 | формат `mm:ss` |
| `4d2a64cb5df5fc4e_0` | 5954877 | медиаплеер: AUDIO-метаданные, `hideDownloadButton` |
| `4d2a64cb5df5fc4e_0` | 6046114 | `/attachments/` + `/ephemeral-attachments/` |
| `4d2a64cb5df5fc4e_0` | 6046342 | проверка подписи (`ex/is/hm`), `CDN_HOST`, `MEDIA_PROXY_ENDPOINT` |
| `4d2a64cb5df5fc4e_0` | 6047505 | refresh-urls: запрос и разбор `refreshed_urls[0].refreshed` |
| `4d2a64cb5df5fc4e_0` | 6056280 | `a(e,t)` — сравнение первого сегмента MIME |
| `4d2a64cb5df5fc4e_0` | 6056405 | регэкспы картинок/аудио/видео (`59318`) |
| `4d2a64cb5df5fc4e_0` | 6116710 | начало модуля `684519` (leaf-компоненты вложений) |
| `4d2a64cb5df5fc4e_0` | 6125741 | текстовое превью: `Range: bytes=0-50000`, charset |
| `4d2a64cb5df5fc4e_0` | 6133343 | декод waveform и canvas |
| `4d2a64cb5df5fc4e_0` | 6142284 | `tM` — кнопка alt-текста |
| `4d2a64cb5df5fc4e_0` | 6143840 | `tw` — аудио-плеер вложения |
| `4d2a64cb5df5fc4e_0` | 6144184 | `tG` — voice message плеер |
| `4d2a64cb5df5fc4e_0` | 6144343 | `tx` — картинка вложения |
| `4d2a64cb5df5fc4e_0` | 6145355 | `tF` — текстовый файл |
| `4d2a64cb5df5fc4e_0` | 6157702 | `tz` — строка «скрыто / нажмите, чтобы узнать больше» |
| `4d2a64cb5df5fc4e_0` | 7656699 | выбор `<audio>` vs OGV (`536184`) |
| `4d2a64cb5df5fc4e_0` | 9703523 | `up` — локализованный размер файла |
| `4d2a64cb5df5fc4e_0` | 7747374 | `BV(channel)` — spoiler-канал |
| `4d2a64cb5df5fc4e_0` | 8920304 | `this.attachments=e.attachments??[]` |
| `4d2a64cb5df5fc4e_0` | 9704193 | `classifyFile` (klass по MIME/имени) |
| `4d2a64cb5df5fc4e_0` | 9705272 | `filesize()` для карточки файла |
| `48550b11930370fb_0` | 223939 | `renderAttachments()` |
| `48550b11930370fb_0` | 227557 | `patchMessageAttachments` (PATCH `{attachments:[…]}`) |
| `48550b11930370fb_0` | 233262 | `renderMediaObscureNotice` |
| `e0f9541dbaca2408_0` | 296 | резолвер причины скрытия (`d`) |
| `e0f9541dbaca2408_0` | 1816 | тексты причин (`SEgHFh`, `XpfDH+`) |
| `5c14801cf5025f71_0` | 390 | `S()`/`M()`/`V()` — критерии медиа для галереи |
| `5c14801cf5025f71_0` | 696 | сборщик элементов галереи (`L`) |
| `510c13959205dd46_0` | 124 | карта CSS-классов иконок файлов |

Живые объекты для фикстур: `decoded/1b4e8eb40ee58eb4_0` (png/webp/gif),
`decoded/2c1c34dc2c6678f0_0` (video/mp4), `decoded/8b3a3d3deafc4cc6_0` (video/quicktime),
`decoded/50f2a9f76680271b_0` (audio/ogg + waveform, video с `title`),
`decoded/a0470793b01b89c7_0` (text/plain с charset), `decoded/861c87b0200e0786_0`
(application/x-msdos-program), `decoded/d4cf5ae1da1b6c4e_0` (zip с `original_content_type`).
