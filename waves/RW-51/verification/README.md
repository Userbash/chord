# Verification evidence

Raw execution logs and native-provider response records are stored losslessly as `.gz` files. `ARCHIVE-MANIFEST.json` records the original bytes and SHA256; decompressing reproduces the immutable source artifact exactly. This retains native line endings and error-output whitespace without editing evidence. Source originals remain in `/root/projects/chord-modernization-verification`.

Later final runs supersede earlier green snapshots. Root-level `VALIDATION.md` describes the final 207-test meta run, native Windows board8/8 and actual ECC planning receipts. Earlier red/green and transport failures are intentionally retained.
