# Configurable Defined Term Rendering Style

## Context

In legal drafting, defined terms at their definition site can be styled differently depending on convention:
- **Bold only** — `**Term**` renders as **Term** (current Lexicon behaviour)
- **Quoted only** — `**Term**` renders as "Term" (no bold)
- **Bold and quoted** — `**Term**` renders as "**Term**"

The Markdown spec stays unchanged (bold = definition), but the .docx rendering can vary via a TOML style option.

## TOML Config

New top-level field in `StyleConfig`:

```toml
# How defined terms are rendered at their definition site
defined_term_style = "bold"   # "bold", "quoted", or "bold_quoted"
```

## Files to Change

### 1. `src/style.rs`
- Add `DefinedTermStyle` enum: `Bold`, `Quoted`, `BoldQuoted`
- Default: `Bold` (preserves current behaviour)
- Add `pub defined_term_style: DefinedTermStyle` to `StyleConfig`

### 2. `src/render/docx.rs`

**Body text rendering** — `render_inline` / `render_inlines_paragraph` where `InlineContent::Bold` is matched (~line 326):
- `Bold`: current behaviour — `.bold()` run
- `Quoted`: regular run with `"` wrapping the text
- `BoldQuoted`: `.bold()` run with `"` wrapping

**Preamble party roles** — in Simple (~line 826) and Prose (~line 888) styles:
- Currently hardcoded as `("role")` with role bold
- Apply same style: `Bold` → `("**role**")`, `Quoted` → `("role")` (already quoted, no bold), `BoldQuoted` → `("**role**")` (already quoted + bold)

**Preamble short_title** — in Simple (~line 793) and Prose (~line 857):
- Currently hardcoded as `("short_title")` with short_title bold
- Same logic as roles

**Custom template `render_template_paragraph`** (~line 991):
- `**text**` markers currently always render bold
- Apply defined_term_style: `Bold` → bold only, `Quoted` → wrap in `"`, `BoldQuoted` → bold + wrap in `"`

### 3. `style.example.toml`
- Add `defined_term_style = "bold"` in typography section with comment

### 4. Post-work checklist
- Update `planning/implementation-status.md`
- Run `cargo test`
- Commit and push

## Verification
1. `cargo build` — clean
2. `cargo test` — all pass
3. Build example with each style and inspect .docx output:
   - Default (bold) — terms render bold, no quotes
   - `defined_term_style = "quoted"` — terms render with quotes, no bold
   - `defined_term_style = "bold_quoted"` — terms render bold with quotes
4. Test preamble rendering with each style
