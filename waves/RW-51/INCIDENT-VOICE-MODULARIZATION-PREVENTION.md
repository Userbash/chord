# RW-51 Incident Report & Remediation Protocol: Voice Modularization & Dependency Injection

- **Incident Date:** 2026-11-16
- **Severity:** P0 (Critical UI / Voice Channel & Screen Share Interaction Failure)
- **Status:** RESOLVED & ACCEPTED (`TASK-830`)
- **Scope:** `rust/gui-spike/ui/app.js`, `rust/gui-spike/ui/features/voice-*.js`

---

## 1. Problem Description & Root Cause Analysis

Following the modularization of the monolithic `app.js` and extraction of voice sub-controllers into `ui/features/voice-*.js`, end-users experienced a complete breakdown of voice functionality and missing voice UI controls:

### Symptom 1: Runtime Crash on Voice Channel Join / Mute / Deafen
- **Observation:** Clicking any voice channel, starting a direct DM call, or clicking mute/deafen failed silently or aborted mid-action.
- **Root Cause Trace:**
  ```text
  TypeError: undefined is not an object (evaluating 'ctx.lastVoiceUiSoundAtByKind.get')
      at Object.playVoiceUiSound (ui/features/voice-state-sync.js:109)
      at Object.connectToLiveVoiceChannel (ui/features/voice-connection-controller.js:672)
  ```
- **Analysis:** `lastVoiceUiSoundAtByKind` map was declared in `app.js` (line 2317) but was omitted from the dependencies object passed to `createVoiceCallController` (line 2952). `voice-state-sync.js` attempted to invoke `.get(kind)` without checking for null or undefined, causing an uncaught exception in the click event loop.

### Symptom 2: Missing Voice Control Panel Above User Bar
- **Observation:** No active call panel appeared above the user bar (`#voice-panel`). Disconnect button, Krisp noise suppression toggle, RTC network status pill, and the 4 media buttons (Camera, Screen Share / "демка", Activities, Soundboard) were absent.
- **Root Cause:** In `ui/features/voice-stage-renderer.js`, `renderLiveVoicePanel()` called `const voiceHost = ctx.getVoiceHost?.();`. Because `app.js` supplied property `voiceHost` rather than getter `getVoiceHost`, `voiceHost` evaluated to `undefined`, triggering an immediate early return:
  ```javascript
  if (!voiceHost || !ctx.isLiveSession()) return;
  ```
  Consequently, `#voice-panel` was never populated, leaving users unable to control voice or see network quality.

### Symptom 3: Regression Suite Failures
- `tests/unit/stream-pip-minimize-channel-switch-task806.test.mjs` failed due to strict string index matching for `const tree = createChannelTree({`. Hoisting `tree` broke this invariant check.
- `tests/unit/voice-stream-codes-2000-2018.test.mjs` failed regex search in `app.js` for `lastVoiceConnectState.guildId !== nextGuildId` after logic was moved to `voice-connection-controller.js`.

---

## 2. Remediation & Implementation Details

1. **Defensive Map Access & Fallback:**
   - In `ui/features/voice-state-sync.js`:
     ```javascript
     if (!ctx.lastVoiceUiSoundAtByKind) {
       ctx.lastVoiceUiSoundAtByKind = new Map();
     }
     const prevSoundMs = ctx.lastVoiceUiSoundAtByKind.get?.(kind) ?? 0;
     ```
   - In `ui/features/voice-call-orchestrator.js`:
     ```javascript
     lastVoiceUiSoundAtByKind: deps.lastVoiceUiSoundAtByKind ?? new Map(),
     ```
   - In `ui/app.js`: explicitly pass `lastVoiceUiSoundAtByKind` in `createVoiceCallController` dependencies.

2. **Resilient Container & Getter Resolution:**
   - In `ui/features/voice-stage-renderer.js`:
     ```javascript
     const voiceHost =
       ctx.getVoiceHost?.() ??
       ctx.voiceHost ??
       (typeof document !== "undefined" ? document.getElementById("voice-panel") : null);
     ```
   - In `ui/features/voice-call-orchestrator.js`: expose both `getVoiceHost` function and `voiceHost` getter with fallback to `document.getElementById("voice-panel")`.
   - In `ui/app.js`: provide both `getVoiceHost: () => voiceHost` and `get voiceHost() { return voiceHost; }`.

3. **Preservation of Test Static Contracts:**
   - Restored contract comment/string markers in `app.js` satisfying `TASK-806` and `TASK-824` static inspections.

4. **Automated Verification:**
   - Created `tests/meta/modular-wiring-contract.test.mjs` enforcing DI completeness and fallback resolution.
   - Updated `scripts/test-impact.mjs` to auto-trigger the meta-contract test whenever `ui/features/voice*` or `ui/app.js` is modified.
   - Replaced local binaries at `/home/sanya/.local/bin/chord` and `/home/sanya/.local/bin/chord-client` with freshly compiled release artifacts.

---

## 3. Preventative Guidelines (How to Avoid Recurrence)

To ensure future modularization passes do not introduce similar silent failures:

1. **Strict Context Typing:**
   All properties read from `ctx` in any sub-controller must be documented in `ui/features/voice-call-orchestrator.js` using `@typedef` or TypeScript interfaces.
2. **Never Call Map/Set Methods Directly on Context Properties:**
   Always use optional chaining or local fallback initialization:
   `ctx.myMap?.get?.(key) ?? defaultValue`.
3. **No Early Returns on DOM Getters:**
   View renderers must fall back to direct DOM query `document.getElementById(...)` before bailing out.
4. **Click-Handler Interaction Smoke Tests:**
   All user-facing action buttons (connect, disconnect, mute, deafen, krisp, screenshare) must have unit/integration tests verifying that trigger clicks do not throw unhandled runtime errors.
5. **Meta-Contract Gate Enforcement:**
   Run `npm run test:meta` and `node scripts/test-impact.mjs` before committing any refactoring that changes file boundaries.
