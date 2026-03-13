# Library Extraction

## Goal

Extract the core processing logic into a standalone `lexicon-core` crate so it can be used as a Rust library, embedded in other tools, or compiled to WASM.

## Current state

`main.rs` is already a thin CLI wrapper. All logic lives behind `lib.rs` which exposes:
- `parse(input) -> Document`
- `resolve(doc)`
- `render_docx(doc, style) -> Vec<u8>`
- `process(input, style) -> (Vec<u8>, Vec<Diagnostic>)`

## Steps

1. Convert to a Cargo workspace with two crates:
   - `lexicon-core` — everything currently in `src/` except `main.rs`
   - `lexicon` (CLI) — `main.rs` only, depends on `lexicon-core`
2. Publish `lexicon-core` to crates.io with a stable public API
3. Consider WASM target for browser-based rendering
