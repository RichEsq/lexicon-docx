# TODO

1. ~~Look into how the Parties identifiers in the YAML are/can be used~~ ✓
2. ~~Look into how the Annexures in the YAML are/can be used~~ ✓ (Renamed to `exhibits` — generates placeholder pages; file import staged as future work)
3. Look into how the version numbers in the YAML are/can be used
4. Investigate whether addenda are cross-referenceable
5. ~~TOC doesn't work~~ ✓
6. ~~TOML config items: reference in footer boolean, page number in footer boolean, version number in footer boolean (appended to reference, e.g. Ref: OK:RP:20260115v3)~~ ✓
7. ~~TOML style config option for schedule position: end of document (current) or after the TOC~~ ✓
8. ~~Exhibit file import — local files~~ ✓ (PNG, JPEG, PDF via `pdftoppm`). URL paths (Phase 3) still pending — spec now allows HTTP/HTTPS URLs in the `path` field; docx compiler needs to fetch and embed remote files.
9. Signature pages — a `# SIGNATURE` top-level heading (or similar) for rendering signature blocks with party names, signature lines, witness fields, etc.
10. Placeholders for front-matter definitions — allow the contract body to reference front-matter values (e.g. party role, name, specifier) as inline placeholders that get resolved during processing
11. ~~Better fallback syntax for schedule references~~ ✓ (Replaced reference-link syntax with phrase-based detection from defined terms. Schedule items are now ordinary bold terms with natural prose — fully readable without a processor.)
12. Auto-inject definitions into the definitions clause — automatically add YAML party roles and schedule reference descriptions into the definitions list in the rendered output. Perhaps behind a TOML toggle. Requires a mechanism to identify which clause is "the definitions clause" (e.g. convention-based heading match, anchor name, or explicit front-matter/TOML reference). Tricky problem.
13. Auto-alphabetise the definitions clause — if we can identify the definitions clause (see #12), automatically sort its sub-clauses into alphabetical order in the rendered output, regardless of source order.
