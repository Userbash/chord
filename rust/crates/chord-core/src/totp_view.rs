//! TOTP 2FA Modal Dialog (`TotpDialog`) with 6 separate digit input cells (`TASK-1009A`).
//!
//! Oracle: Discord 2FA entry dialog (`#1e1f22` digit cells, auto-advance focus, Backspace return, Ctrl+V paste).

#![forbid(unsafe_code)]
#![allow(dead_code)]

pub const TOTP_DIGIT_COUNT: usize = 6;
pub const CELL_BG_COLOR: &str = "#1e1f22";
pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_DIGIT_FIELD_BG: &str = "#1e1f22";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TotpViewError {
    NonNumericCharacter(char),
    IncompleteCode { len: usize, expected: usize },
    EmptyClipboard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigitCell {
    pub index: usize,
    pub value: Option<char>,
    pub bg_color: &'static str,
    pub focused: bool,
    pub hovered: bool,
}

impl DigitCell {
    #[must_use]
    pub const fn new(index: usize, focused: bool) -> Self {
        Self {
            index,
            value: None,
            bg_color: TOKEN_DIGIT_FIELD_BG,
            focused,
            hovered: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpDialog {
    pub cells: [DigitCell; TOTP_DIGIT_COUNT],
    pub active_index: usize,
    pub modal_bg: &'static str,
    pub border_color: &'static str,
    pub accent_color: &'static str,
    pub text_color: &'static str,
}

impl Default for TotpDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl TotpDialog {
    #[must_use]
    pub fn new() -> Self {
        let cells = std::array::from_fn(|i| DigitCell::new(i, i == 0));
        Self {
            cells,
            active_index: 0,
            modal_bg: TOKEN_BACKGROUND,
            border_color: TOKEN_BORDER,
            accent_color: TOKEN_ACCENT,
            text_color: TOKEN_TEXT_PRIMARY,
        }
    }

    fn set_focus(&mut self, next_index: usize) {
        let clamped = next_index.min(TOTP_DIGIT_COUNT - 1);
        for (idx, cell) in self.cells.iter_mut().enumerate() {
            cell.focused = idx == clamped;
        }
        self.active_index = clamped;
    }

    /// Handles single-character input in the active cell and auto-advances focus.
    pub fn on_digit_input(&mut self, ch: char) -> Result<(), TotpViewError> {
        if !ch.is_ascii_digit() {
            return Err(TotpViewError::NonNumericCharacter(ch));
        }
        self.cells[self.active_index].value = Some(ch);
        if self.active_index + 1 < TOTP_DIGIT_COUNT {
            self.set_focus(self.active_index + 1);
        }
        Ok(())
    }

    /// Handles Backspace: clears current cell if non-empty, or moves focus back and clears previous cell.
    pub fn on_backspace(&mut self) {
        if self.cells[self.active_index].value.is_some() {
            self.cells[self.active_index].value = None;
            return;
        }
        if self.active_index > 0 {
            let prev = self.active_index - 1;
            self.cells[prev].value = None;
            self.set_focus(prev);
        }
    }

    /// Handles `Ctrl+V` clipboard paste of a full 6-digit TOTP code.
    pub fn on_paste(&mut self, clipboard: &str) -> Result<(), TotpViewError> {
        let trimmed = clipboard.trim();
        if trimmed.is_empty() {
            return Err(TotpViewError::EmptyClipboard);
        }
        for ch in trimmed.chars() {
            if !ch.is_ascii_digit() {
                return Err(TotpViewError::NonNumericCharacter(ch));
            }
        }
        let digits: Vec<char> = trimmed.chars().take(TOTP_DIGIT_COUNT).collect();
        if digits.len() != TOTP_DIGIT_COUNT {
            return Err(TotpViewError::IncompleteCode {
                len: digits.len(),
                expected: TOTP_DIGIT_COUNT,
            });
        }
        for (idx, digit) in digits.into_iter().enumerate() {
            self.cells[idx].value = Some(digit);
        }
        self.set_focus(TOTP_DIGIT_COUNT - 1);
        Ok(())
    }

    /// Collects and validates the 6-digit code before submission.
    pub fn submit(&self) -> Result<String, TotpViewError> {
        let mut code = String::with_capacity(TOTP_DIGIT_COUNT);
        for cell in &self.cells {
            if let Some(ch) = cell.value {
                if !ch.is_ascii_digit() {
                    return Err(TotpViewError::NonNumericCharacter(ch));
                }
                code.push(ch);
            }
        }
        if code.len() != TOTP_DIGIT_COUNT {
            return Err(TotpViewError::IncompleteCode {
                len: code.len(),
                expected: TOTP_DIGIT_COUNT,
            });
        }
        Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_six_cells_with_1e1f22_and_focus_on_first() {
        let dialog = TotpDialog::new();
        assert_eq!(dialog.cells.len(), 6);
        assert_eq!(dialog.active_index, 0);
        for (i, cell) in dialog.cells.iter().enumerate() {
            assert_eq!(cell.bg_color, "#1e1f22");
            assert_eq!(cell.focused, i == 0);
            assert_eq!(cell.value, None);
        }
    }

    #[test]
    fn auto_advances_focus_on_digit_and_returns_on_backspace() {
        let mut dialog = TotpDialog::new();
        dialog.on_digit_input('1').unwrap();
        assert_eq!(dialog.active_index, 1);
        dialog.on_digit_input('2').unwrap();
        assert_eq!(dialog.active_index, 2);

        // Backspace on empty cell 2 moves focus back to cell 1 and clears it
        dialog.on_backspace();
        assert_eq!(dialog.active_index, 1);
        assert_eq!(dialog.cells[1].value, None);
    }

    #[test]
    fn ctrl_v_paste_fills_all_six_cells() {
        let mut dialog = TotpDialog::new();
        dialog.on_paste("654321").unwrap();
        assert_eq!(dialog.submit().unwrap(), "654321");
        assert_eq!(dialog.active_index, 5);
    }

    #[test]
    fn rejects_non_numeric_and_incomplete_codes() {
        let mut dialog = TotpDialog::new();
        assert_eq!(
            dialog.on_digit_input('a'),
            Err(TotpViewError::NonNumericCharacter('a'))
        );
        dialog.on_digit_input('1').unwrap();
        assert_eq!(
            dialog.submit(),
            Err(TotpViewError::IncompleteCode {
                len: 1,
                expected: 6
            })
        );
        assert_eq!(
            dialog.on_paste("12ab56"),
            Err(TotpViewError::NonNumericCharacter('a'))
        );
    }
}
