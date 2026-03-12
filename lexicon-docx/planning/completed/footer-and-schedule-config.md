# Footer Config & Schedule Position

## Status: Implemented

## Feature 1: Footer Config — `[footer]` TOML section

### Goal

Make footer content configurable via the style TOML.

### Configuration

```toml
[footer]
show_ref = true               # show reference number in footer (default true)
show_page_number = true        # show "Page X of Y" in footer (default true)
show_version = false           # append version to ref, e.g. "Ref: OK:RP:20260115v3" (default false)
```

### Behaviour

- `show_ref = true`: renders "Ref: {ref}" on the left side of the footer
- `show_version = true`: appends version to ref string: "Ref: {ref}v{version}". Only applies when `show_ref` is also true.
- `show_page_number = true`: renders "Page X of Y" on the right side
- Tab separator between ref and page number only added when both are shown
- If both are false, an empty footer paragraph is still emitted (required for first-page suppression logic)

### Implementation

1. Add `FooterConfig` struct to `style.rs` with 3 bool fields
2. Add `footer: FooterConfig` field to `StyleConfig`
3. Modify footer rendering in `render/docx.rs` (lines 50-73) to be conditional on `style.footer.*`
4. Update `style.example.toml`

---

## Feature 2: Schedule Position — `schedule_position` top-level TOML field

### Goal

Allow the schedule to appear either at the end of the document (after all addenda) or before the contract body (after the TOC).

### Configuration

```toml
schedule_position = "end"      # "end" (after addenda, default) or "after_toc" (before contract body)
```

### Behaviour

- `end`: schedule renders after all addenda (current behaviour)
- `after_toc`: schedule renders before the contract body, regardless of whether TOC is enabled

### Implementation

1. Add `SchedulePosition` enum (`End`, `AfterToc`) to `style.rs`
2. Add `schedule_position: SchedulePosition` to `StyleConfig` (top-level, not nested)
3. In `render/docx.rs`, insert schedule rendering at the position-dependent location:
   - `AfterToc`: render schedule right before the body loop
   - `End`: render schedule after addenda (existing location)
   - Schedule renders in exactly one location, never both
4. Update `style.example.toml`
