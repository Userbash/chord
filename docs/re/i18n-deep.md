# Discord 1.0.162 — i18n: Runtime, Pluralization, Rich Format

Closing the pluralization and rich-text format technical gap based on local Discord Desktop 1.0.162 artifacts (Linux, Flatpak).
All findings are derived from decompiled bundles and catalogs; every citation is verified by `i18n-deep-build.mjs`. Machine-readable artifact: `i18n-deep.json`.

## 0. Sources and Delta Against `docs/`

| Item | Value |
|---|---|
| Bundles | `docs/re/out/bundles` — 3,628 files, 61 MB |
| i18n Runtime + simple-markdown | `discord.com_assets_501428.98d57a9bd8cbb604.js` (12.7 MB; modules 237341/624067/668317/446274/618830/239127/835751) |
| Discord Message Rules | `discord.com_assets_web.4d4b2f75fdea5470.js` (12.4 MB; modules 652215/999915/247186/379418/113051/7584/873879/223637/683412/704726) |
| Catalogs | `ru-catalog.json` — 23,836 keys / 2,443,841 B; `en-catalog.json` — 23,903 / 1,427,037 B |

Previous analyses noted that localization catalogs exist but left pluralization mechanics unexplored.
This document specifies: Runtime architecture, AST node structures, pluralization engine, message markdown grammar and regex rules, argument interpolation, locale negotiation, and fallback chains.

## 1. Runtime: Custom AST, Not IntlMessageFormat

Implementation is **custom** (`FormatBuilder` + `bindFormatValues`), layered over standard `Intl.*` APIs (not formatjs/ICU parser).
Key citations (file:offset — offset in bytes from start of bundle):

- `239127(e,t,r){"use strict";var n=this&&this.__awaiter` — `MessageLoader` module (`501428.98d57a9bd8cbb604.js:17458`), `_loadLocale(e){...}` (:19797), `localeImportMap[e]()` (:20259).
- `668317(e,t,r){...t.FormatBuilder=void 0` (:5284) — `bindFormatValues`/`FormatBuilder`.
- `835751(...)t.IntlManager=t.DEFAULT_LOCALE=void 0` (:15943) and `t.DEFAULT_LOCALE="en-US"`.
- `dataFormatterCache=new class{...this.dateTime=new Map` (:1995, module 237341) — Cache for `Intl.DateTimeFormat / DurationFormat / ListFormat / NumberFormat / PluralRules / RelativeTimeFormat`.
- `formatNumber(r,a){let s=...resolveFormatConfigOptions)(t.number,a);...getNumberFormatter` (:4369, module 624067).

Lifecycle: `createLoader(localeImportMap, defaultLocale)` -> `supportedLocales = Object.keys(map)` (:18297) ->
on locale lookup, `_loadLocale` performs dynamic `import()` of corresponding chunk (`localeImportMap[e]()`, :20259) -> `this.messages[e] = (yield o).default` -> value wrapped in `InternalIntlMessage(ast, locale)`, parsed once and cached per locale.

**Message Keys — Hashed, 6 characters** (`446274`, :12888):
`runtimeHashMessageKey(s)` = `base64(xxh64(s, seed=0) in LE order)[0..6]`. Verified 14/14 against client modules (`ru -> +irF5O`, `en-US -> +dXJII`, `AKA -> l1QVfj`, `OK -> TyCVIq`, …);
xxh64 itself (module `618830`) verified against `xxhsum -H1` across 81 test inputs — 0 discrepancies.
Important: The key is hashed from the **internal ID**, not English text — out of 22,803 en strings, hash(text) matched the key only 5 times (AKA/GIF/OK/PS4/PS5); SCREAMING_SNAKE IDs accounted for 1,246 (5.46%).

AST structure — **keyless JSON**, node types (`524897`):
`0 Literal, 1 Argument, 2 Number, 3 Date, 4 Time, 5 Select, 6 Plural, 7 Pound (#), 8 Tag`.
Examples: `[[2,"size"]," MB"]`, `[3,"timestamp","medium"]`.
Standalone `FORMAT_JS_POUND=Object.freeze({type:7})`.

Formatters for numbers/dates/times sourced from `DEFAULT_FORMAT_CONFIG` (module `896745`):
`number:{integer:{maximumFractionDigits:0},currency:{style:"currency"},percent:{style:"percent"}}`, `date.short/medium/long/full`, `time.short/medium/long/full`.

## 2. Pluralization

Engine mechanism (module `668317`): Node `[6,"count",{branches},offset,"cardinal"|"ordinal"]`;
first matches exact key `=${value}`, otherwise `Intl.PluralRules(type).select(value - offset)`,
then `.other`; if no branch matches, raises exception:
`"${b} is not a known option for plural value ... Valid options are ..."` (:8001).
`#` (Pound, `[7]`) inside branch interpolated via `Intl.NumberFormat` (`formatNumber(h)`).

Categories in complex Slavic rules (`Intl.PluralRules("ru")`, CLDR): **one** (1, 21, 31…), **few** (2–4, 22–24…),
**many** (0, 5–20, 25–30…), **other** (fractions); plus exact selectors `=N`.
English uses strictly `one` and `other`.

Observed plural keys: ru 911 (3.82%), en 932. Plural AST nodes in ru catalog: 965.

Examples:

| Key | Template Pattern | 1 | 2 | 5 | 11 | 21 | 22 |
|---|---|---|---|---|---|---|---|
| `+yF0ds` | `{count, plural, =0{None} one{# recipient} few{# recipients} many{# recipients} other{...}}` | 1 recipient (one) | 2 recipients (few) | 5 recipients (many) | 11 recipients | 21 recipients (one) | 22 recipients (few) |
| `r/EUap` | `[6,"count",{one:"# game",few:"# games",many:"# games",other:"# games"}]` | 1 game | 2 games | 5 games | 11 games (many) | 21 games | 22 games |
| `+sThav` | `[6,"hours",{one:"# hour",few:"# hours",many:"# hours",other:"# hours"}]` | 1 hour | 2 hours | 5 hours | 11 hours | 21 hours | 22 hours |
| `pAQJsJ` | `[6,"count",{one:"# member",few:"# members",other:"# members"}]` | 1 member | 2 members | 5 members | 11 members | 21 members | 22 members |
| `C4WXvc` | `[6,"memberCount",{one:"# person",other:"# people"}]` | 1 person | 2 people | 5 people | 11 people | 21 people | 22 people |

Implementation edge cases:
- `other` is mandatory in plural AST nodes and acts as safety fallback;
- Exact selectors extend beyond `=0/=1` (e.g. `d8pkUP` contains `=2` and `=3`);
- Nested plural blocks bind `#` to innermost `currentPluralValue`;
- Missing plural branch throws an error rather than silently rendering empty string.

## 3. Message Rich Format (Markdown)

Engine: **simple-markdown** (core engine in `501428`, Discord grammar overrides in `web` bundle), output targets React nodes rather than raw HTML. Complete rules grammar and regex definitions:

| Syntax | Description | Regex / Rule Definition |
|---|---|---|
| `**text**` | Bold | `/^\*\*((?:\\[\s\S]|[^\\])+?)\*\*(?!\*)/` (501428:824318) |
| `__text__` | Underline | `/^__((?:\\[\s\S]|[^\\])+?)__(?!_)/` (501428:824595) |
| `~~text~~` | Strikethrough | Discord override: `/^~~([\s\S]+?)~~(?!_)/` -> `<s>` (web:5891002) |
| `*text*`, `_text_` | Italics | `em` with `quality = length+0.2` (501428:823844) |
| `` `code` `` | Inline code | `/^(`+)([\s\S]*?[^`])\1(?!`)/` (501428:825091) |
| ` ```lang … ``` ` | Code block | `/^```(?:([a-z0-9_+\-.#]+?)\n)?\n*([^\n][^]*?)\n*```/i` (+ hljs, `dangerouslySetInnerHTML`) (web:5887232) |
| `\|\|text\|\|` | Spoiler | `/^\|\|([\s\S]+?)\|\|/` (require first char `\|`) (web:5891144) |
| `> text` | Blockquote | `/^( *>[^\n]+(\n[^
]+)*\n*)+/` (501428:820468) |
| `>>> text` | Multi-line blockquote | Discord rule: `/^>>>\s+([\s\S]+)/` (greedy capture to EOF) (web:5887012) |
| `# `, `## `, `### ` | Headings (levels 1-3) | `/^(#{1,3}) +([^\n]+?) *#* *(?:\n+|$)/` (web:5894028) |
| `-# text` | Subtext | Subtext heading: `/^-# +([^\n]+?) *(?:\n+|$)/` (web:5894028) |
| `-`, `*`, `1.` | Lists (depth <= 11) | `/ *-0123456789/`, `t._listLevel>=11 -> null` (web:5870466) |
| `@silent` | Silent notification | `/^(@silent(?![^\s]))/` (web:5889046) |
| `<@123>`, `<@!123>`, `@everyone`, `@here` | Mentions | `/^<@!?(\d+)>|^(@(?:everyone|here))/` (web:5888290) |
| `<@&123>` | Role mention | `/^<@&(\d+)>/` (web:5887472) |
| `<#123>` | Channel mention | `/^<#(\d+)>/` (web:5882377) |
| `</cmd opt:123>` | Slash command | `/^<\/((?:[-_\p{Letter}\p{Number}\p{sc=Deva}\p{sc=Thai}]{1,32})…):(\d+)>/u` (web:5889412) |
| `<a:name:123>`, `<:name:123>` | Custom emoji | `/^<(a)?:(\w+):(\d+)>/` (valid Snowflake id -> `\d{17,19}`) (web:5883764) |
| `:name:`, `:name::skin-tone-3:` | Emoji by name | `EMOJI_NAME_RE=/^:([^\s:]+?(?:::skin-tone-\d)?):/` (web:4453876) |
| `<t:1712345678:R>` | Timestamp | `^<t:(-?\d{1,17})(?::(t|T|d|D|f|F|s|S|R))?>` (web:5903524) |
| `<sound:guildId:soundId>` | Soundboard sound | `/^<sound:(\d+):(\d+)>/` (web:6975997) |
| `<id:home>`, `<id:browse:123>` | Navigation target | `/^<id:(home|browse|customize|guide|linked-roles)(?::(\d+))?>/` (web:314350) |
| `https://…`, `<https://…>`, `[t](u)` | Links | `/^(https?:\/\/[^\s<]+[^<.,:;"')\]\s])/`, `<([^: >]+:\/[^ >]+)>` (501428:822642) |
| `\* \_ \~ \| \\` | Escape | `/^\\([^0-9A-Za-z\s])/` -> literal (501428:821885) |

Rule execution order (`order` attribute; engine sorts by `order`, breaks ties by `quality`):
`heading 0 -> nptable -> lheading -> hr -> codeBlock(indent) -> fence -> blockQuote 6 -> list 7 -> def -> table -> newline 10 -> paragraph 11 -> escape 12 -> tableSeparator -> autolink 14 -> mailto -> url 16 -> link 17 -> image…refimage -> em 21 -> strong 22 -> u 23 -> del 24 -> inlineCode 25 -> br 26 -> text 27`.
Discord-specific rules (mention/emoji/spoiler/timestamp/…) receive text-level order and trigger via `requiredFirstCharacters` (e.g. `["<"]`), matching with higher precision.

**Escaping Pipeline:**
1. HTML escaping is NOT performed prior to markdown parsing: Parser evaluates raw text and renders React virtual DOM nodes.
   `sanitizeText` (`E={"<":"&lt;",">":"&gt;","&":"&amp;"…}`, `501428:815303`) applies only to engine raw HTML fallback (`defaultHtmlOutput`).
2. Markdown escape `\X` operates as distinct early rule (order 12, ahead of em/strong/u/del/inlineCode).
3. URL sanitizer: `sanitizeUrl` rejects dangerous schemes `javascript:`, `vbscript:`, `data:` (`501428`).
4. Code blocks dispatch to hljs and mount via `dangerouslySetInnerHTML`, delegating escaping to hljs syntax formatting.

Slate editor syntax set (`web`): `A=new Set(["*","_","~","|","\\"])`; pairs: `strong **`, `u __`, `s ~~`, `spoiler ||`, `em *`, `inlineCode ` (single backtick), `subtext "-# "`, `autolink/mailto/tel "<…>"`, `soundboard "<sound:…>"`, `staticRouteLink "<id:…>"`, `silentPrefix "@silent "`, `escape "\"`.

## 4. Rich Tags in i18n Strings (Distinct from Message Markdown)

Within catalog payloads, formatting tags are represented by node `[8,"$b",children,control]`; standard 6-tag suite (`DEFAULT_REACT_RICH_TEXT_ELEMENTS`, `501428`):

| Tag | React Target | Markdown Rendering | AST Node Type |
|---|---|---|---|
| `$b` | `strong` | `**…**` | `strong` |
| `$i` | `em` | `*…*` | `em` |
| `$del` | `del` | `~~…~~` | `s` |
| `$code` | `code` | `` `…` `` | `inlineCode` |
| `$link` | `a[href]` | `[text](url)` | `link{target}` |
| `$p` | `p` | `text\n\n` | `paragraph` |

Unrecognized tag raises exception: `${e} is not a known rich text formatting tag`.
Tags without `$` prefix represent **hook functions**: receive `(nodes, keyPrefix)` and return transformed nodes (e.g. `F6Hook`, `tabHook`, `usernameHook`); `$link` includes control nodes for callbacks (`[[1,"onSettingsClick"]]`, `[[1,"helpCenterLink"]]`).

## 5. Arguments and Data Formatting

- Argument substitution: `{name}` maps to node `[1,"name"]`. Missing variable raises **exception**:
  `No value for variable '<v>' was provided for the localized message '<m>'` (structured class with `variableName/originalMessage/nodeType`); zero silent fallback.
- `select`: `[5,"var",{...}]` — direct equality match; missing key without `other` throws `"… is not a known option for select value …"`.
- `#` (Pound): `[7]` -> `Intl.NumberFormat` applied to active plural count.
- Numbers: `[2,"name"]` -> `Intl.NumberFormat` with config; number skeletons are absent in catalog (all Number nodes 2-element: `[2,"size"]`, `[2,"percent"]`, `[2,"minutes"]`), units remain literal strings.
- Dates/Time: `[3,"name","medium"]` -> `Intl.DateTimeFormat`; standard skeletons: `short`, `medium`, `long`, custom `M/d` and `::MMMMd`; time: `short`. Relative timestamps use `<t:…:R>` format.
- Lists: Formatted using `Intl.ListFormat`.
- Durations: Handled via `Intl.DurationFormat`.

## 6. Units and Counters

- File sizes: Number + localized unit suffix: en `bTzRR6 [[2,"size"]," KB"]`, `pIn7Af " MB"`, `yhEXX7 " GB"`; transfer rates: `"kbps"`, `"mbps"`.
  **Byte-to-unit unit conversion is executed by caller code**; i18n layer purely formats numbers.
- Quotas and limits: Arguments pre-scaled to megabytes or kilobytes.
- Percentages: `[2,"percent"]` + literal `"%"`.
- Number formatting: Grouping separators match active locale rules.

## 7. Locales, Language Negotiation, Fallback

- Available languages in client asset (36 codes in `lang-list.json`): `en-US en-GB zh-CN zh-TW cs da nl fr de el hu it ja ko pl pt-PT pt-BR ru sk es-419 es-ES sv-SE tr bg uk fi no hr ro lt th vi hi he ar id`.
- Active enabled languages: **31** codes. Disabled: `pt-PT, sk, he, ar, id`.
- Fallback resolution (module `239127`): Requested locale -> `DEFAULT_LOCALE` (`en-US`) -> `fallbackWith(...)` cycle protection -> warning:
  `Requested message <debugKey> (<key>) does not have a value in the requested locale <ru> nor the default locale <en-US>` -> empty message (`fallbackMessage`).
- `IntlManager` initializes formatters over locale pair `[currentLocale, defaultLocale]`.

## 8. Filtered Scope and Boundaries

- Complete multi-megabyte string catalogs omitted from repository markdown; maintained in JSON artifacts.
- Analysis focuses specifically on runtime mechanics and simple-markdown grammar implementations.
- Cross-validation executed against primary `en-US` and localized catalogs.

## 9. Unknowns and Limitations

1. Hashed message keys are non-reversible (ID != text): string relocation must be tracked via AST shape.
2. Code cache contains fragmented historical catalogs for secondary locales.
3. Message highlight rule disabled in renderer (`match:()=>null`), active only in editor.
4. Pre-render bulk loading triggers vary between platform targets.

## 10. Architectural Implications for Rust Client

1. **Format:** Keyless JSON-AST with 9 node types + 6 rich formatting tags. Transparent keys internally (`channel.members.count`) with compatibility hashing layer (`base64(xxh64(id))` — 6 chars).
2. **Pluralization:** Full CLDR plural rule evaluation (`one`, `few`, `many`, `other`) + exact `=N` selectors; `#` formatted via locale number rules.
3. **Markdown Pipeline:** Match simple-markdown rule order (escape 12 -> inline styles 21–25 -> text 27), observe `requiredFirstCharacters`, Discord specific rules (`#` 1–3, list depth <= 11, `-#` subtext, `@silent`). Keep user markdown isolated from i18n rich tags `$b/$link`.
4. **Escaping:** Avoid pre-parsing HTML sanitization when targeting virtual DOM / UI tree; sanitize URL schemes against dangerous protocols (`javascript:`, `data:`).
5. **Data Formatting:** Cache locale formatters; keep unit conversions in business logic; handle relative time via periodic refresh timers.
