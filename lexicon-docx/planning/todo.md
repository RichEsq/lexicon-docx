# TODO

1. ~~Look into how the Parties identifiers in the YAML are/can be used~~ ✓
2. ~~Look into how the Annexures in the YAML are/can be used~~ ✓ (Renamed to `exhibits` — generates placeholder pages; file import staged as future work)
3. Look into how the version numbers in the YAML are/can be used
4. Investigate whether addenda are cross-referenceable
5. ~~TOC doesn't work~~ ✓
6. ~~TOML config items: reference in footer boolean, page number in footer boolean, version number in footer boolean (appended to reference, e.g. Ref: OK:RP:20260115v3)~~ ✓
7. ~~TOML style config option for schedule position: end of document (current) or after the TOC~~ ✓
8. ~~Exhibit file import — local files~~ ✓ (PNG, JPEG, PDF via `pdftoppm`). URL paths (Phase 3) still pending — spec now allows HTTP/HTTPS URLs in the `path` field; docx compiler needs to fetch and embed remote files.
9. ~~Signature pages~~ ✓ — configurable signature blocks via `[signatures]` TOML config + external `signatures.toml` definitions file. `entity_type` field on parties. Templates for AU, UK, US, NZ.
10. Placeholders for front-matter definitions — allow the contract body to reference front-matter values (e.g. party role, name, specifier) as inline placeholders that get resolved during processing
11. ~~Better fallback syntax for schedule references~~ ✓ (Replaced reference-link syntax with phrase-based detection from defined terms. Schedule items are now ordinary bold terms with natural prose — fully readable without a processor.)
12. Auto-inject definitions into the definitions clause — automatically add YAML party roles and schedule reference descriptions into the definitions list in the rendered output. Perhaps behind a TOML toggle. Requires a mechanism to identify which clause is "the definitions clause" (e.g. convention-based heading match, anchor name, or explicit front-matter/TOML reference). Tricky problem.
13. Auto-alphabetise the definitions clause — if we can identify the definitions clause (see #12), automatically sort its sub-clauses into alphabetical order in the rendered output, regardless of source order.
14. Comments — a syntax for comments in Lexicon Markdown that are visible to the drafter but stripped from the rendered output. Useful for drafting notes, review annotations, and internal instructions. Needs design: HTML comment syntax (`<!-- ... -->`) is the natural Markdown-compatible choice (already ignored by renderers), but comrak may or may not expose them in the AST. Alternatives: a custom syntax (e.g. `// comment` or `%% comment`), or a front-matter/TOML toggle to control whether HTML comments are stripped or rendered as annotations. Consider whether comments should support inline (within a clause) and block-level (between clauses) usage, and whether they could optionally render as Word comments/annotations rather than being fully stripped.
15. Configurable document element ordering — the order of document sections after the body is currently hardcoded: signature pages → addenda → exhibits → schedule (end). This should be configurable via a TOML list, e.g.:
    ```toml
    # Default order:
    element_order = ["signatures", "addenda", "exhibits", "schedule"]

    # Or for a contract where schedules come before exhibits:
    element_order = ["signatures", "schedules", "addenda", "exhibits"]

    # Or signatures at the very end:
    element_order = ["addenda", "exhibits", "schedules", "signatures"]
    ```
    This would replace the current `schedule_position` config (which only offers `end` vs `after_toc`) with a more general mechanism. Elements not listed are omitted. The `after_toc` schedule position would become a separate `pre_body_elements` list or similar. Needs design thought around: what happens when both `schedule_position` and `element_order` are set (backwards compat), whether TOC/cover/preamble should also be orderable, and how to handle the page-break logic between elements.
16. Watch mode (`--watch`) — automatically rebuild the .docx when the input `.md` file (or style/signatures config files) change on disk. Useful for iterative drafting: keep the `.docx` open in Word/preview and see changes reflected on save. Implementation: add `notify` crate for filesystem watching; on change, re-run the full pipeline and overwrite the output file. Debounce rapid saves (e.g. 200ms). Print a timestamp + status line on each rebuild. Consider also watching files referenced by `exhibits[].path` so exhibit updates trigger a rebuild too.
