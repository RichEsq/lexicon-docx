# Lexicon

Lexicon is a plain-text legal contract format built on standard Markdown, plus tooling to convert Lexicon Markdown contracts into formatted documents.

**Write contracts in Markdown. Get professionally formatted Word documents.**

## What is Lexicon Markdown?

Lexicon Markdown extends standard Markdown with conventions for legal documents. A Lexicon document is valid Markdown that renders correctly in any Markdown viewer (GitHub, Obsidian, VS Code, etc.), but when processed by Lexicon tooling, gains:

- **Legal clause numbering** — `1.`, `1.1`, `(a)`, `(i)`
- **Cross-reference resolution** — anchors like `{#termination}` and links like `[clause 3](#termination)` are auto-resolved to correct clause numbers
- **Defined term validation** — `**Confidential Information** means ...` defines a term; the processor warns if it's never used
- **Cover pages, TOC, headers/footers** — generated from YAML front-matter metadata
- **Schedule generation** — variable values collected into a schedule annexure
- **Draft watermarks** — automatic "DRAFT" watermark when `status: draft`

Without a processor, Lexicon Markdown reads as a clean, well-structured document. With a processor, it becomes a production-ready legal contract.

## Quick Example

```markdown
---
title: Deed of Release
date: 2026-01-15
status: draft
parties:
  - name: Alice Smith
    role: Employee
  - name: Acme Corp
    specifier: ACN 123 456 789
    role: Employer
---

1. ## Definitions {#definitions}

    1. **Claim** means any and all claims, demands, or causes of action.

    1. **Confidential Information** means all information disclosed
       by one party to the other.

2. ## Release {#release}

    1. The **Employee** releases the **Employer** from all Claims.

    1. The obligations in [clause 1](#definitions) survive termination.

        1. This includes any Confidential Information held by the Employee.
```

## Repository Structure

```
spec.md           # The Lexicon Markdown specification (v1.0-draft)
example.md        # A real-world Data Processing Addendum in Lexicon format
lexicon-docx/     # Rust CLI — converts Lexicon Markdown to .docx
```

## Lexicon DOCX Processor

The `lexicon-docx` CLI converts Lexicon Markdown files into formatted Word documents with legal numbering, cover pages, tables of contents, and more.

### Requirements

- [Rust](https://rustup.rs/) (2024 edition)

### Build

```bash
cd lexicon-docx
cargo build
```

### Usage

```bash
# Build a .docx from a Lexicon contract
cargo run -- build ../example.md -o output.docx

# Validate without generating output
cargo run -- validate ../example.md

# Use a custom style configuration
cargo run -- build ../example.md -o output.docx --style style.toml

# Fail on warnings
cargo run -- build ../example.md --strict
```

### Features

| Feature | Description |
|---------|-------------|
| Cover page | Title, parties, date, status, version, author, ref |
| Table of contents | Auto-generated from clause headings |
| Legal numbering | Native Word numbering: `1.`, `1.1`, `(a)`, `(i)` |
| Cross-references | `{#id}` anchors resolved to clause numbers |
| Defined terms | Bold terms validated for usage; warnings for unused terms |
| Schedule annexures | Reference-link items collected into a schedule table |
| Draft watermark | Diagonal "DRAFT" watermark when `status: draft` |
| Headers/footers | Document ref and page numbering on all pages |
| Configurable layout | `cover_page` and `toc` booleans; TOML style overrides |

## Front-Matter Fields

```yaml
---
title: Contract Title          # required
date: 2026-01-15               # required, YYYY-MM-DD
ref: "ABC:123"                 # optional, drafter's reference
author: Jane Doe (Law Firm)    # optional
status: draft                  # optional: draft | final | executed
version: 2                     # optional, positive integer
cover_page: true               # optional, default true
toc: true                      # optional, default true
parties:                       # required
  - name: Party Name
    specifier: ACN 123 456 789 # optional
    role: Buyer                # used as a defined term
annexures:                     # optional
  - Annexure Title
---
```

## Specification

The full Lexicon Markdown specification is in [`spec.md`](spec.md). It covers:

- Document structure and clause hierarchy
- Defined terms and term validation rules
- Cross-reference anchors and resolution
- Schedule items and reference-link syntax
- Annexure declarations and content
- Processor capabilities and validation requirements

## License

MIT
