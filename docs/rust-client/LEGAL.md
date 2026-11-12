# Legal Framework for the Rust Client (P0-07)

Status: Project internal working document, not formal legal advice. The project owner makes decisions regarding deployment and distribution; in case of conflict with Discord Terms of Service (ToS), Discord ToS takes precedence.

## 1. Project Boundaries

- This is an **unofficial** client; it is not affiliated with Discord Inc. and does not impersonate an official client.
- Reverse engineering and client modification are subject to Discord ToS. Analysis is conducted solely on locally installed clients for interoperability and cleanroom research within applicable law.
- It is strictly prohibited to use this project to bypass paid features, DRM, bans, anti-abuse mechanisms, cheats, mass mailing, spamming, scraping third-party data, or distributing modified proprietary binaries.
- User account automation (messages, reactions, voice) must be triggered solely by explicit user interaction; background botting and "selfbots" are strictly outside the project scope.
- Discord API rate limits must be respected: the client must never retry more aggressively than server `Retry-After` headers, never attempt to bypass HTTP 429 errors, and never use multi-accounting to circumvent quotas.

## 2. Telemetry and Networking

- **Telemetry is disabled by default**: `POST /science`, `POST /beaker`, and similar analytics events are not transmitted. This is enforced by `T-REST-*`/telemetry-off tests.
- Outbound network traffic is limited strictly to user-initiated actions: WebSocket gateway, REST API, voice signaling/media transport. There is no hidden analytical, advertisement, or model training traffic.
- **Redaction Policy:** Local logs must never contain session tokens, `analytics_token`, `qos_token`, public IP addresses, or raw binary payloads. Redaction is enforced across `Debug`/CLI representations (`REDACTED`, `<redacted>`); comprehensive log auditing is tracked under P1-10/T-SEC.
- Packet captures and test fixtures are kept locally; publication of raw data is allowed only after thorough sanitization and provenance verification (`fixtures/MANIFEST.json`).

## 3. Honest Client Fingerprint

- The client does not attempt to spoof the official Discord desktop client or promise "invisibility" against anti-abuse systems.
- `X-Super-Properties` and User-Agent headers are derived from cleanroom-documented fields; the policy is an **honest native fingerprint** without forged third-party client versions, without artificial jitter/noise to bypass detection, and without undocumented fields.
- Fingerprint parameters are configurable for automated tests and local debugging, but default values are transparent and reproducible.
- Ethical and operational risk assessment is detailed in `CONCEPT.md` §7.

## 4. Accounts, Tokens, and User Data

- **Credential Storage:** Authentication tokens must be securely stored in the system keyring (libsecret/Secret Service on Linux, Keychain on macOS, Credential Manager on Windows); log files and test fixtures must only contain `REDACTED` strings.
- Passwords and MFA codes are never logged; authentication error responses never reveal secrets.
- User data (messages, server rosters, user settings) remains strictly local; export operations require explicit user interaction.
- Profile and cache deletion routines must permanently purge authentication tokens and sensitive logs.

## 5. Assets and Licensing

- **Asset Policy:** Proprietary Discord fonts, icons, sound effects, emojis, and brand assets are not distributed without an explicit license. The client utilizes original or permissively licensed open-source assets with clear attribution.
- Project code is dual-licensed under MIT OR Apache-2.0 (as specified in `rust/Cargo.toml`); third-party dependencies adhere to their respective permissive licenses. Automated license auditing and SBOM generation are enforced via `cargo-deny`.
- Discord trademarks and logos are not used as project branding; documentation includes standard unofficial client disclaimers.

## 6. Live Operations and Operator Approval

Live operations require explicit human operator approval and are never executed automatically: connecting to live Discord production endpoints, spawning processes, active voice streaming, sending live messages or reactions, and GUI screen capture. Until approved, development relies exclusively on sanitized local fixtures, mock servers, and offline unit/contract tests.

## 7. P0-07 Checklist

Status legend: `[x]` verified by current code/process, `[~]` policy or partial implementation, `[ ]` backlog.

- [x] README and documentation contain unofficial client disclaimers and ToS notices.
- [x] Telemetry disabled by default and verified by `rest_telemetry` automated tests.
- [x] Honest fingerprint policy without anti-abuse spoofing or deceptive noise.
- [x] Secrets in `Debug` and CLI representations are redacted; comprehensive sanitization enforced.
- [x] Third-party proprietary assets excluded; license compliance tracked.
- [x] Live operations restricted to explicit human operator approval.
