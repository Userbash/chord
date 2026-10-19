#!/usr/bin/env python3
# Ревью RW-02: применение заглушек/мутаций к КОПИИ src/zstd.rs (ws/) из zstd.rs.orig.
# Каждая замена проверяется на точное совпадение — если сниппет не найден, скрипт падает.
import sys, pathlib

src_dir = pathlib.Path("/tmp/opencode/review-rw-02")
orig = (src_dir / "zstd.rs.orig").read_text()
dst = src_dir / "ws/crates/discord-codec/src/zstd.rs"

PUSH_HEAD = "    pub fn push(&mut self, frame: &[u8]) -> Result<Vec<u8>, ZstdError> {"
PUSH_TAIL_MARK = "\n    /// Сколько сжатых байт потока уже разобрано"

def replace_push_body(text: str, body: str) -> str:
    start = text.index(PUSH_HEAD) + len(PUSH_HEAD)
    end = text.index(PUSH_TAIL_MARK, start)
    return text[:start] + "\n" + body + "\n    }\n" + text[end:]

STUB_A = '''        // ЗАГЛУШКА R-02 (вариант A): декодирования нет вовсе.
        self.pending.extend_from_slice(frame);
        Ok(Vec::new())'''

STUB_B = '''        // ЗАГЛУШКА R-02 (вариант B): проверка магии есть, декодирования нет.
        self.pending.extend_from_slice(frame);
        if !self.magic_checked {
            if self.pending.len() < crate::ZSTD_FRAME_MAGIC.len() {
                return Ok(Vec::new());
            }
            if self.pending[..crate::ZSTD_FRAME_MAGIC.len()] != crate::ZSTD_FRAME_MAGIC {
                return Err(ZstdError::MissingMagic);
            }
            self.magic_checked = true;
        }
        Ok(Vec::new())'''

def apply(name: str, text: str) -> str:
    if name == "stub_a":
        return replace_push_body(text, STUB_A)
    if name == "stub_b":
        return replace_push_body(text, STUB_B)
    if name == "mut1_nomagic":
        old = '''            if self.pending[..crate::ZSTD_FRAME_MAGIC.len()] != crate::ZSTD_FRAME_MAGIC {
                return Err(ZstdError::MissingMagic);
            }'''
        assert text.count(old) == 1
        return text.replace(old, "            // МУТАЦИЯ R-05.1: проверка магии снята (всегда считаем валидной)")
    if name == "mut2_break_on_empty":
        old = "        let mut out = Vec::new();\n        loop {\n"
        assert text.count(old) == 1
        return text.replace(
            old,
            "        let mut out = Vec::new();\n        loop {\n            if self.pending.is_empty() {\n                break; // МУТАЦИЯ R-05.2: выход при пустом pending\n            }\n",
        )
    if name == "mut3_no_trailing":
        old = '''        let initialized = self.decoder.bytes_read_from_source() > 0;
        if initialized && self.decoder.is_finished() && !self.pending.is_empty() {
            return Err(ZstdError::TrailingData {
                bytes: self.pending.len(),
            });
        }'''
        assert text.count(old) == 1
        return text.replace(old, "        // МУТАЦИЯ R-05.3: проверка хвостовых данных снята")
    if name == "mut4_advanced_minus_one":
        old = '''            let advanced = usize::try_from(delta)
                .unwrap_or(usize::MAX)
                .min(self.pending.len());'''
        assert text.count(old) == 1
        return text.replace(
            old,
            '''            let advanced = usize::try_from(delta)
                .unwrap_or(usize::MAX)
                .min(self.pending.len())
                .saturating_sub(1); // МУТАЦИЯ R-05.4''',
        )
    if name == "restore":
        return orig
    raise SystemExit(f"неизвестный вариант: {name}")

if __name__ == "__main__":
    variant = sys.argv[1]
    if variant == "orig":
        dst.write_text(orig)
        print(f"{dst}: восстановлен из orig")
    else:
        dst.write_text(apply(variant, orig))
        print(f"{dst}: применён вариант {variant}")
