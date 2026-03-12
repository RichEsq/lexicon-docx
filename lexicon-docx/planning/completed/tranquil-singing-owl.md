# Schedule Refactor: Phrase-Based Detection

## Context

The current schedule system uses Markdown reference-link syntax (`[display][ref-id]` + `[ref-id]: #schedule "value"`), which is a clever hack but has poor cross-compilation compatibility — the value is hidden in the link title attribute and invisible in plain Markdown, Pandoc HTML output, and other renderers. This refactor replaces it with a phrase-based system where defined terms whose definition text references a schedule are automatically collected into schedule pages.

## How It Works

1. Author declares schedules in front-matter YAML: `schedule: [{title: "Schedule"}]`
2. Author writes defined terms with schedule-referencing phrases: `**Objection Period** has the meaning given by the Schedule.`
3. The compiler pattern-matches built-in phrases (case-insensitive), collects the terms, and generates a schedule page with the term names and blank lines for completion
4. Pre-filled values are dropped — schedules are always blank for completion at execution
5. Multiple schedules supported — each with its own title matched in phrases

## Built-In Phrases (case-insensitive, `{title}` = schedule title)

- "given by the {title}"
- "set out in the {title}"
- "specified in the {title}"
- "described in the {title}"
- "defined in the {title}"
- "provided in the {title}"
- "contained in the {title}"
- "stated in the {title}"
- "referred to in the {title}"
- "as per the {title}"
- "in accordance with the {title}"
- "pursuant to the {title}"
- "detailed in the {title}"

## Implementation Steps

### 1. Model changes (`src/model.rs`)

- Add `ScheduleConfig { title: String }` struct (with Deserialize)
- Add `schedule: Vec<ScheduleConfig>` to `DocumentMeta` (serde default empty)
- Update `ScheduleItem`: replace `{ description, value }` with `{ term: String, schedule_index: usize }`
- Remove `InlineContent::ScheduleRef` variant entirely (and its `as_plain_text()` arm)

### 2. Style changes (`src/style.rs`)

- Add `ScheduleOrder` enum: `Document` (default), `Alphabetical`
- Add `schedule_order: ScheduleOrder` to `StyleConfig`

### 3. Parser changes (`src/parser/clause.rs`)

- Remove the `if link_url == "#schedule"` branch (~line 302). Links to `#schedule` will fall through to `CrossRef` handling and produce an unresolved-anchor warning (natural migration signal).

### 4. Resolve changes (`src/resolve.rs`) — core logic

- Add `build_schedule_phrase_patterns(titles: &[ScheduleConfig]) -> Vec<(usize, Regex)>`: for each schedule, compile one regex with all 13 phrases as alternations (case-insensitive), with `{title}` replaced by the schedule's title
- Add `TermKind::ScheduleDefinition(usize)` variant (usize = schedule index)
- Update `classify_term()` to accept schedule patterns and check text following a bold term for schedule phrases. Concatenate all inline text after the bold term's position to handle split inlines.
- Rewrite `collect_schedule_items()`: walk document bold terms using the same traversal as term validation, classify each, collect `ScheduleDefinition` terms into `Document.schedule_items`
- Merge schedule collection and term validation into a single traversal pass (classify each bold term once)
- Add warnings: term references undeclared schedule title; declared schedule has no referencing terms
- Remove old `collect_clause_schedule_items()`, `collect_inline_schedule_items()`, `collect_addendum_schedule_items()` functions
- Schedule terms are still added to the definitions list (they ARE defined terms)
- Update `collect_inlines_text()` to remove `ScheduleRef` match arm

### 5. Render changes (`src/render/docx.rs`)

- Remove `InlineContent::ScheduleRef` match arm in inline rendering (~line 375)
- Rewrite `render_schedule()` signature: `(docx, schedule_configs, schedule_items, style) -> Docx`
  - Iterate over each `ScheduleConfig` by index
  - Filter items for that schedule index
  - Apply `schedule_order` (alphabetical sort if configured)
  - Render page break + centred bold heading (the schedule title from YAML)
  - Render two-column table: "Item" | blank column for completion
  - Skip rendering schedules with no items (emit warning only)
- Update call sites (~lines 174 and 214) to pass `&doc.meta.schedule`

### 6. Update `example.md`

- Add `schedule: [{title: "Schedule"}]` to front-matter YAML
- Remove `<!-- Schedule References -->` and `[ref-1]: #schedule "7 days"` at bottom
- Replace inline schedule references with phrase-based definitions (e.g. add `**Objection Period** has the meaning given by the Schedule.` to the definitions clause)
- Update any body text that used `[the Objection Period][ref-1]` to plain text `the Objection Period`

### 7. Rewrite `spec.md` section 6

- Replace reference-link schedule syntax with phrase-based system
- Document the `schedule` front-matter field (list of `{title}` objects)
- List all 13 built-in phrases
- Document multiple schedules
- Update section 2.2 to add `schedule` field docs
- Update section 9 complete example
- Update section 11 summary table (remove reference-link row, add schedule phrase row)
- Update section 10 processor capabilities

### 8. Tests

- Unit test `build_schedule_phrase_patterns()` — each phrase matches, case-insensitive, non-matching text rejected
- Unit test `classify_term()` with schedule phrases
- Integration test: parse doc with schedule YAML + phrase definitions → verify `schedule_items` populated
- Test multiple schedules with different titles
- Test warning on undeclared schedule title
- Test warning on unreferenced schedule declaration
- `cargo test` — no regressions

### 9. Update planning/meta files

- `implementation-status.md` — move to "Recently completed", note refactor
- `CLAUDE.md` — update model description, remove `ScheduleRef` references
- `todo.md` — update schedule-related items
- `MEMORY.md` — update attachment terminology section

## Critical Files

| File | Change |
|------|--------|
| `src/model.rs` | Add ScheduleConfig, update ScheduleItem, remove ScheduleRef |
| `src/style.rs` | Add ScheduleOrder enum + field |
| `src/parser/clause.rs` | Remove `#schedule` detection |
| `src/resolve.rs` | Phrase patterns, classify_term update, rewrite collection, merge passes |
| `src/render/docx.rs` | Multi-schedule rendering, remove ScheduleRef inline, update call sites |
| `example.md` | New syntax, remove reference links |
| `spec.md` | Rewrite section 6, update sections 2, 9, 10, 11 |

## Verification

1. `cargo build` — compiles cleanly
2. `cargo test` — all tests pass
3. `cargo run -- build ../example.md -o output.docx` — produces valid .docx with schedule page
4. `cargo run -- validate ../example.md` — no unexpected warnings
5. Open output.docx in Word — schedule page renders correctly with title and blank table
