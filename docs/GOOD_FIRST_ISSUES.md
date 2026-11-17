# Good First Issues for New Contributors

Welcome to **Chord**! If you want to contribute to a high-performance, open-source alternative Discord desktop client built with **Rust** and **Tauri 2**, this guide is for you.

Here are 5 beginner-friendly, isolated tasks designed to help you get familiar with the codebase. Each task has clear boundaries, pointers to exact files, and simple verification steps.

---

## 🚀 Getting Started in 2 Minutes

Before picking a task, make sure you can clone and verify the project:

```bash
# 1. Clone the repository
git clone https://github.com/Userbash/chord.git
cd chord

# 2. Checkout the active development branch
git checkout tauri

# 3. Run instant sanity precheck (< 1 second)
npm run precheck
```

If `npm run precheck` outputs `✅ [PRECHECK] Passed`, your environment is ready!

---

## 🎯 Curated Good First Issues

### Issue #1: Channel Topic Character Counter & Truncation
- **Domain:** Frontend / UI (`JavaScript`, `CSS`)
- **Difficulty:** Beginner (`~30 mins`)
- **Target Files:**
  - `rust/gui-spike/ui/app.js` (channel header rendering)
  - `rust/gui-spike/ui/style.css`
- **Objective:**
  When viewing a channel with a very long description (up to 1,024 characters), the text in the channel header should gracefully truncate with an ellipsis `...` and show the full topic on click or hover tooltip.
- **Expected Behavior:**
  - Topic header does not overflow or push action buttons out of the viewport.
  - Clean CSS `text-overflow: ellipsis; white-space: nowrap; overflow: hidden;`.
- **How to verify:**
  ```bash
  npm run precheck
  ```

---

### Issue #2: Graceful Error Handling on Cancelled PipeWire Screen Share
- **Domain:** Backend / Linux Desktop (`Rust`, `XDG Desktop Portal`)
- **Difficulty:** Beginner / Intermediate (`~45 mins`)
- **Target Files:**
  - `rust/gui-spike/src/main.rs`
  - `rust/crates/chord-platform/src/screencast.rs`
- **Objective:**
  When the user opens the "Share Screen" dialog but clicks **Cancel** in the system XDG Desktop Portal window, the application currently receives a portal rejection error.
- **Expected Behavior:**
  - Catch `ashpd::Error::Cancelled` (or user dismiss event).
  - Return a clean, non-fatal `{ status: "cancelled" }` response to the frontend rather than logging an error stack trace.
  - The UI closes the screen share modal cleanly without showing a red error alert.
- **How to verify:**
  ```bash
  cargo test -p chord-client
  npm run precheck
  ```

---

### Issue #3: Dynamic Volume Percentage Tooltip in Voice Stage
- **Domain:** Frontend / UI Component (`JavaScript`, `CSS`)
- **Difficulty:** Beginner (`~30 mins`)
- **Target Files:**
  - `rust/gui-spike/ui/features/voice-stage.js`
  - `rust/gui-spike/ui/style.css`
- **Objective:**
  While adjusting a participant's volume slider in the voice stage grid, show a floating badge/tooltip indicating the current percentage (e.g. `0%` to `200%`, with `100%` as neutral).
- **Expected Behavior:**
  - As the slider input event fires, update a small badge adjacent to the thumb showing `Math.round(val * 100) + '%'`.
  - Tooltip fades out smoothly 500ms after slider release.
- **How to verify:**
  ```bash
  npm run precheck
  ```

---

### Issue #4: Global Keyboard Shortcut `Ctrl+/` for Shortcuts Modal
- **Domain:** Frontend / Keyboard Navigation (`JavaScript`)
- **Difficulty:** Beginner (`~20 mins`)
- **Target Files:**
  - `rust/gui-spike/ui/app.js`
  - `rust/gui-spike/ui/features/settings.js`
- **Objective:**
  Discord power users expect `Ctrl+/` (or `Cmd+/` on macOS) to instantly pop up the Keyboard Shortcuts cheat-sheet.
- **Expected Behavior:**
  - In the global `keydown` event listener, intercept `Ctrl + /` (when not inside an active text input or textarea).
  - Toggle the keyboard shortcuts modal open/closed.
  - Pressing `Escape` closes the modal.
- **How to verify:**
  ```bash
  npm run precheck
  ```

---

### Issue #5: High-Contrast OLED / Midnight Dark Theme Preset
- **Domain:** Design / Styling (`CSS`, `Tokens`)
- **Difficulty:** Beginner (`~30 mins`)
- **Target Files:**
  - `rust/gui-spike/ui/tokens.css`
  - `rust/gui-spike/ui/features/settings.js`
- **Objective:**
  Add a true pitch-black `#000000` theme preset for OLED displays to maximize battery life on laptops and Steam Deck.
- **Expected Behavior:**
  - Add `[data-theme="midnight"]` CSS token overrides (pure black backgrounds, `#000000` chat container, `#0a0a0a` server rail).
  - Register "Midnight (OLED)" in the Appearance settings dropdown in `settings.js`.
- **How to verify:**
  ```bash
  node --test tests/meta/design-breakpoints.test.mjs
  npm run precheck
  ```

---

## 🛠️ Contribution Workflow (How to Submit Your PR)

1. **Fork the repo** and create a feature branch from `tauri`:
   ```bash
   git checkout -b feature/my-cool-fix
   ```
2. **Make your changes** adhering to clean code guidelines.
3. **Verify locally:**
   ```bash
   npm run precheck
   ```
4. **Commit with Conventional Commits:**
   ```bash
   git commit -m "feat(ui): add channel topic character counter"
   ```
5. **Open a Pull Request** against branch `tauri`. Our automated CI will run checks and give you instant feedback!

Need help? Feel free to open an issue or ask in our community channels!
