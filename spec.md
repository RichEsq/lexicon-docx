# Lexicon Markdown Spec

**Version**: 1.0-draft
**Date**: 2026-03-09

## 1. Overview

Lexicon Markdown is a contract drafting format that extends standard Markdown with conventions for legal documents. It is designed to be:

1. **Valid Markdown** — any Lexicon Markdown document renders correctly in any Markdown renderer (GitHub, Obsidian, Pandoc, VS Code, etc.);
2. **Legally structured** — supporting the hierarchical clause structure, defined terms, cross-references, schedule items, and meta properties that legal contracts require; and
3. **Tooling-friendly** — a processor can extract structured data, auto-resolve cross-references, generate glossaries, validate defined terms, and render to `.docx` or `.pdf`.

Without a processor, a Lexicon Markdown document reads and renders as a well-structured Markdown document. With a processor, it gains auto-numbering, cross-reference resolution, defined term validation, and schedule generation.

## 2. Document Meta Properties

Every Lexicon Markdown document begins with a YAML front-matter block. This block contains structured metadata about the contract that does not form part of the substantive terms but is used by a processor for rendering cover pages, headers, footers, party blocks, and exhibit pages.

### 2.1. Syntax

The front-matter block is enclosed by `---` delimiters and must appear at the very beginning of the document, before any other content.

```yaml
---
title: Deed of Release
short_title: Deed
date: 2017-11-05
ref: "VL:RP:20161021"
author: Richard Prangell (Viridian Lawyers)
status: draft
version: 2
parties:
  - name: Elliot Anderson
    specifier: 123 Forth St, New York
    role: Employee
  - name: ECorp Limited
    specifier: ACN 123 456 789
    role: Employer
exhibits:
  - title: Office Diagram
  - title: Employment Obligations
---
```

### 2.2. Fields

#### 2.2.1. `title` (required)

The plain-language title of the document. This will appear as the document title in any rendering, and as the content of the `# Heading` in the output.

```yaml
title: Contract for the Sale of Business
```

#### 2.2.2. `date` (required)

The effective date of the contract. Must be in `YYYY-MM-DD` format. A processor must validate this format.

```yaml
date: 2026-01-15
```

#### 2.2.3. `ref` (optional)

The drafter's reference number. Multiple references are separated by commas within the string.

```yaml
ref: "VL:RP:20161021, ASH/JN/123456"
```

#### 2.2.4. `author` (optional)

The drafter or authoring firm. Free text.

```yaml
author: Richard Prangell (Viridian Lawyers)
```

#### 2.2.5. `status` (optional)

The current status of the document. Must be one of the following values:

| Value      | Description                                                              |
| ---------- | ------------------------------------------------------------------------ |
| `draft`    | The document is a working draft, not yet agreed by the parties.          |
| `final`    | The document has been agreed but not yet executed.                       |
| `executed` | The document has been executed (signed) by the parties.                  |

If omitted, no status is assumed. A processor may use this field to render a watermark (e.g., "DRAFT") or status indicator on the output document.

```yaml
status: draft
```

#### 2.2.6. `short_title` (optional)

An abbreviated name for the document, used to refer to the document throughout the contract body (e.g., in a parties preamble or recitals). Defaults to `"Agreement"` if not specified.

The `short_title` is automatically treated as a defined term. A processor should include it alongside party roles in the list of automatic definitions.

Common values: `"Agreement"`, `"Deed"`, `"NDA"`, `"Addendum"`, `"Contract"`.

```yaml
short_title: Deed
```

#### 2.2.7. `version` (optional)

The version number of the document, expressed as a positive integer. This tracks iterations during negotiation and review. A processor may render the version number in the document header or footer.

```yaml
version: 2
```

#### 2.2.8. `parties` (required)

A list of parties to the contract. Each party has three sub-fields:

| Sub-field   | Required | Description                                                        |
| ----------- | -------- | ------------------------------------------------------------------ |
| `name`      | Yes      | The legal name of the party.                                       |
| `specifier` | No       | Identifying detail (address, ACN, ABN, registration number, etc.). |
| `role`      | Yes      | The drafting reference used throughout the contract (e.g., "Employer", "Contractor"). |

```yaml
parties:
  - name: Elliot Anderson
    specifier: 123 Forth St, New York
    role: Employee
  - name: ECorp Limited
    specifier: ACN 123 456 789
    role: Employer
```

Party roles are automatically treated as defined terms. A processor should include them in any generated glossary and may validate that the role appears in the contract body.

#### 2.2.9. `exhibits` (optional)

A list of external documents to be exhibited (attached for reference) to the contract. Each entry is an object with a `title` field. Exhibits are pre-existing documents included for reference that do not contain legal terms — for example, a property map exhibited to a lease, or a technical diagram.

```yaml
exhibits:
  - title: Plumbing Diagram
  - title: Site Plan
```

A processor generates an exhibit placeholder page for each entry, with the exhibit number and title centred on the page (e.g., "EXHIBIT 1 - Plumbing Diagram"). Future versions may support a `path` field for importing external files directly into the output document.

### 2.3. Markdown Compatibility

YAML front-matter is widely supported. Renderers that do not support it will display the raw YAML at the top of the document, which is acceptable for plain-text readability. Renderers that do support it (Pandoc, Obsidian, Hugo, GitHub Pages) will parse it as metadata.

## 3. Document Structure

### 3.1. Document Title

The document title is specified in the `title` field of the front-matter (see section 2.2.1). It is not repeated as a `# Heading` in the document body. A processor will render the title from the front-matter.

If the document is being used without a processor, the author may optionally include a `# Title` heading at the top of the body for readability, but a processor should ignore it in favour of the front-matter `title`.

### 3.2. Top-Level Clauses

A top-level clause is represented by an ordered list item with a `##` heading:

```markdown
1. ## Definitions
```

This produces a numbered top-level clause with a heading that will appear in any table of contents.

### 3.3. Sub-clauses

Sub-clauses are represented by indented ordered list items. Each level of nesting uses 4 spaces of indentation:

```markdown
1. ## Termination

    1. A party may terminate this Agreement with immediate effect by giving written notice to the other if:

        1. the other party breaches any provision of this Agreement and fails to remedy the breach within 30 days after receiving notice requiring it to do so; or

        2. the other party undergoes an **Insolvency Event**.
```

The hierarchy is:

| Level         | Syntax                   | Rendered as (by processor) |
| ------------- | ------------------------ | -------------------------- |
| Top-level     | `1. ## Heading`          | `1. Heading`               |
| Clause        | `    1. Text`            | `1.1.` or `1.1`           |
| Sub-clause    | `        1. Text`        | `(a)`                      |
| Sub-sub-clause| `            1. Text`    | `(i)`                      |

Without a processor, standard Markdown renderers will display these as nested numbered lists (`1.`, `1.`, `1.`, `1.`). With a processor (e.g., Pandoc with a custom filter, or a dedicated tool), the numbering is transformed to the legal convention shown in the table above.

### 3.4. Multiple Paragraphs Within a Clause

A clause may contain multiple paragraphs. Subsequent paragraphs are indented to the same level as their parent clause and separated by a blank line:

```markdown
1. ## Termination

    1. A party may terminate this Agreement with immediate effect.

        Nothing in this clause shall allow a Party to act unreasonably.

    2. At the end of this Agreement, each party must return all **Confidential Information**.
```

### 3.5. Blockquotes

Blockquotes are used for material that does not form a structural part of the clause hierarchy — formulae, examples, or quoted text:

```markdown
1. ## Termination Fee

    1. The Lessee will pay a termination fee calculated as follows:

        > (2 × A) − C
        > Where "A" has the value of 1 month's rent
        > Where "C" has the value of the Rental Bond
```

### 3.6. Sub-headings Within a Clause

Where a clause requires a sub-heading (e.g., for grouped terms within a miscellaneous clause), use a `###` heading inside the list item:

```markdown
14. ## General Terms

    1. ### Governing Law and Jurisdiction

        1. This Addendum is governed by the laws of New South Wales.

    2. ### Severance

        1. If any provision of this Addendum is invalid, it will be severed.
```

### 3.7. Superscript

Superscript text is marked with `^` delimiters:

```markdown
^2^
```

This renders as superscript in the output document. Useful for footnote markers, ordinals, or mathematical notation.

### 3.8. Prose Sections

Some parts of a contract are not structured as numbered clauses (e.g., recitals, addendum content, signature blocks). These are written as standard Markdown paragraphs, headings, lists, and tables outside of the numbered outline structure.

## 4. Defined Terms

### 4.1. Overview

Defined terms are marked with **bold** formatting (`**Term**`) at the point of definition only. After a term is defined, all subsequent references to it appear as ordinary text. Bold must not be used for emphasis. For emphasis, use *italics*.

### 4.2. Formal Definitions

A formal definition is a bold term followed by the word "means" (or "has the meaning"):

```markdown
1. **Applicable Laws** means European Union or Member State laws with respect to any
   Merchant Personal Data in respect of which any Merchant Group Member is subject
   to EU Data Protection Laws.
```

The pattern is:

```
**Term** means ...
```

or:

```
**Term** has the meaning given to it in clause X.
```

A processor identifies formal definitions by matching this pattern. The bold text is the canonical defined term.

### 4.3. Inline Definitions

A term can be defined contextually within a clause, typically using a parenthetical:

```markdown
1. The Employer agrees to pay (the "**Payment**") to the Employee.
```

or:

```markdown
1. The parties to this agreement ("**Parties**") agree as follows.
```

The pattern is:

```
("**Term**")
```

or:

```
(the "**Term**")
```

A processor identifies inline definitions by matching bold text within parentheses and quotation marks.

### 4.4. References to Defined Terms

After a term is defined, all subsequent uses appear as ordinary (unformatted) text:

```markdown
1. The Employee must return all Confidential Information to the Employer.
```

A processor should:

1. collect all defined terms from bold text (formal definitions, inline definitions, and party roles);
2. scan the document text for occurrences of each defined term; and
3. warn if a defined term is never used in the document.

### 4.5. Party Roles as Defined Terms

Party roles declared in the front-matter `parties[].role` field are automatically defined terms. They do not need a separate formal definition in the body, though the author may include one. When used in the body, they appear as ordinary text:

```markdown
1. The Employer agrees to the following terms.
```

### 4.6. Generated Glossary

A processor may generate a definitions schedule or glossary by collecting all defined terms, their definitions, and their locations. This is optional and at the processor's discretion.

## 5. Cross-References

### 5.1. Overview

Cross-references allow one clause to refer to another by a stable identifier, rather than by a hard-coded clause number. This means clause numbers can change (due to insertions, deletions, or reordering) without breaking references.

### 5.2. Anchors

An anchor marks a location in the document that can be referenced from elsewhere.

#### 5.2.1. Heading-Level Anchors

Top-level clause headings receive anchors using Pandoc's attribute syntax, appended to the heading:

```markdown
1. ## Termination {#termination}
```

In standard Markdown renderers, `{#termination}` may render as literal text (harmless). In Pandoc and many other processors, it creates an HTML `id` attribute on the heading.

#### 5.2.2. Clause-Level Anchors

To anchor a specific sub-clause (not a heading), append the attribute syntax to the end of the clause text:

```markdown
    2. The Employer shall pay the Payment within the Timeframe. {#payment-timeframe}
```

In standard Markdown, this renders as literal text at the end of the clause. A processor strips the anchor from the rendered output and records it for cross-reference resolution.

### 5.3. References

A reference to an anchored clause uses standard Markdown link syntax:

```markdown
Notwithstanding the provisions of [clause 2.2](#payment-timeframe), payment is
subject to the following conditions.
```

The format is:

```
[display text](#anchor-id)
```

Where:

- `display text` is what appears in the rendered document (e.g., `clause 2.2`, `clause 5`, `this clause`).
- `anchor-id` is the identifier from the anchor.

#### 5.3.1. Without a Processor

In plain Markdown, these render as clickable hyperlinks (linking to `#payment-timeframe`). The display text is static and must be manually maintained. This is the same as the current system's behaviour.

#### 5.3.2. With a Processor

A processor:

1. resolves each anchor to its clause number (e.g., `#payment-timeframe` → `clause 1.2`);
2. replaces the display text with the resolved reference (e.g., `[clause 2.2](#payment-timeframe)` → `clause 1.2`);
3. warns if any reference points to a non-existent anchor.

The display text in the source serves as a fallback and a hint to the author. The processor overwrites it with the correct value.

### 5.4. Self-Referential Anchors

A clause may reference itself:

```markdown
    3. For the avoidance of doubt, nothing in [this clause](#avoidance-of-doubt) prevents
       the Consultant from working with Pearson Hardman. {#avoidance-of-doubt}
```

## 6. Schedule Items

### 6.1. Overview

Schedule items are variable values within a contract that are either:

1. pre-filled at drafting time; or
2. left blank for completion at the time of execution.

They appear inline in the contract body and are collected into a schedule (or schedules) by a processor.

### 6.2. Syntax

A schedule item is represented using Markdown's reference-link syntax:

```markdown
The Employer agrees to pay to the Employee [the Payment][ref-1].
```

The reference is defined at the end of the document (or at the end of the relevant section) as:

```markdown
[ref-1]: #schedule "AU $10,000"
```

### 6.3. Reference Definition Format

A schedule item reference definition takes the form:

```
[ref-id]: #schedule "value"
```

Where:

| Component  | Description                                                                |
| ---------- | -------------------------------------------------------------------------- |
| `ref-id`   | A unique identifier for the schedule item (e.g., `ref-1`, `ref-a`).      |
| `#schedule`| A fixed URL fragment that identifies this as a schedule item (not a real link). |
| `"value"`  | The pre-filled value, in quotes. An empty string (`""`) indicates a value to be completed at execution. |

Examples:

```markdown
[ref-1]: #schedule "AU $10,000"
[ref-2]: #schedule ""
```

### 6.4. Behaviour

#### 6.4.1. Without a Processor

In standard Markdown, reference links render the link text as a clickable link. The text `[the Payment][ref-1]` renders as a hyperlink with the display text "the Payment" pointing to `#schedule`. This is not ideal but is functional and readable.

#### 6.4.2. With a Processor

A processor:

1. collects all reference definitions with the `#schedule` URL;
2. extracts the ref-id, the inline display text, and the pre-filled value (or marks it as blank);
3. in the body, renders the inline text with the value (e.g., "the Payment (AU $10,000)") or a blank line for hand completion;
4. generates a schedule page listing all schedule items with their ref-ids, descriptions, and values.

### 6.5. Multiple Schedules

Where a contract requires multiple schedules, use a naming convention in the ref-id:

```markdown
[s1-ref-1]: #schedule "AU $10,000"
[s1-ref-2]: #schedule "14 days from the date of invoice"
[s2-ref-1]: #schedule ""
```

The prefix (`s1-`, `s2-`) groups items into Schedule 1, Schedule 2, etc. A processor uses these prefixes to generate separate schedules.

### 6.6. Schedule Reference Definitions Placement

All schedule reference definitions should be placed at the end of the document, grouped under a comment or heading for readability:

```markdown
<!-- Schedule References -->
[ref-1]: #schedule "AU $10,000"
[ref-2]: #schedule ""
```

Or under a heading (which a processor may strip from the final output):

```markdown
## Schedule Values

[ref-1]: #schedule "AU $10,000"
[ref-2]: #schedule ""
```

## 7. Tables

Standard Markdown tables are used where a contract requires tabular data:

```markdown
| Item        | Rate         | Frequency |
| ----------- | ------------ | --------- |
| Rent        | $1,000/month | Monthly   |
| Maintenance | $200/quarter | Quarterly |
```

A processor renders these as formatted tables in the output document.

## 8. Addenda and Exhibits

Lexicon distinguishes between three types of attachments to a contract:

| Type | Purpose | Where declared | Content |
|------|---------|----------------|---------|
| **Schedule** | Variable commercial terms (see section 6) | Inline reference links | Values collected into a schedule page |
| **Addendum** | Substantive attached sections that supplement the body | `# ADDENDUM` headings in the document body | Free-form Markdown |
| **Exhibit** | Pre-existing external documents included for reference | Front-matter `exhibits` field | Placeholder pages (future: file import) |

### 8.1. Addenda

An addendum is an attached part of the contract that supplements but does not form part of the main body. Addenda may contain substantive terms, amend provisions in the body, or provide additional detail. If an addendum is removed, the contract body remains fully functional.

#### 8.1.1. Syntax

Addendum content appears after the main contract body under a top-level heading beginning with `ADDENDUM` (case-insensitive):

```markdown
# ADDENDUM - Details of Processing

This addendum includes details of processing...
```

The heading may optionally include a number (`# ADDENDUM 1 - Title`), but a processor auto-numbers addenda sequentially regardless of any number in the source heading. The title text is extracted from after the dash separator (any of `-`, `–`, `—`).

Any top-level heading (`#`) that does not begin with `ADDENDUM` will generate a warning. Only `ADDENDUM` headings are parsed as addendum content.

#### 8.1.2. Content

Addendum content is free-form Markdown (paragraphs, lists, tables, headings). It may include numbered clause lists, but does not follow the numbered clause structure of the main body unless appropriate.

### 8.2. Exhibits

An exhibit is a pre-existing document included for reference that does not contain legal terms — for example, a property map, a technical diagram, or an organisational chart.

#### 8.2.1. Declaration

Exhibits are declared in the front-matter `exhibits` field (see section 2.2.9). Each entry has a `title` field.

#### 8.2.2. Rendering

A processor generates a placeholder page for each exhibit, with the exhibit number and title centred on the page (e.g., "EXHIBIT 1 - Property Map"). The physical document can then be inserted manually when printing or assembling the final contract.

#### 8.2.3. Future: File Import

A future version of the specification may support a `path` field on exhibit entries, allowing the processor to import the referenced file directly into the output document.

## 9. Complete Example

```markdown
---
title: Deed of Release
short_title: Deed
date: 2017-11-05
ref: "VL:RP:20161021"
author: Richard Prangell (Viridian Lawyers)
status: final
version: 3
parties:
  - name: Elliot Anderson
    specifier: 123 Forth St, New York
    role: Employee
  - name: ECorp Limited
    specifier: ACN 123 456 789
    role: Employer
exhibits: []
---

1. ## Definitions {#definitions}

    1. **Claim** means any and all claims, investigations, complaints, enquiries, demands, suits, causes of action, damages, debts, costs, proceedings, whether at law or in equity or under any statute (except workers compensation legislation).

    2. **Person** means a natural person, company, partnership, association, joint venture, or any other business or any other organisation, of any description.

2. ## Employer's Obligations {#employer-obligations}

    1. The Employer, without admission of any liability whatsoever, agrees to pay to the Employee [the Payment][ref-1].

    2. The Employer shall pay the Payment to the Employee, within [the Timeframe][ref-2] of an executed counterpart of this Deed being received by the Employer, from the Employee. {#payment-timeframe}

3. ## Employee's Obligations {#employee-obligations}

    1. The Employee releases, discharges the Employer against all Claims which the Employee has, or which, but for this Deed, could, would or might at any time have or have had against the Employer.

    2. Notwithstanding the provisions of [clause 2.2](#payment-timeframe), payment of the Payment is subject to the Employee:

        1. completing a handover of the Employee's duties in good faith; and

        2. the Employee returning all Employer property, information, information technology access details and passwords and intellectual property in the Employee's possession, custody or control within 7 days.

4. ## Deed to Remain Confidential {#confidentiality}

    1. The parties to this Deed agree that they will not disclose the existence of this Deed or the terms of this Deed, to any person, directly or indirectly, without the other party's prior written approval, except:

        1. to their legal or financial advisers, in the course of obtaining advice;

        2. as may be required by law; or

        3. to treating medical practitioners, in the course of obtaining treatment.

    2. The parties acknowledge that their obligations under [this clause](#confidentiality) are on-going.

5. ## Warranties {#warranties}

    1. The Employee warrants and agrees that:

        1. the Employee remains under an ongoing duty not to use or disclose any confidential information belonging to the Employer for as long as that information is not available in the public domain (other than by breach of the terms of this Deed);

        2. prior to entering into this Deed, the Employee was given a reasonable opportunity to obtain any advice (legal, financial or otherwise) about the Deed and the obligations contained in it;

        3. the Employee has had sufficient time to consider the terms of this Deed, its implications and any advice given to the Employee in respect of it;

        4. the Employee has acted honestly and in good faith and has disclosed all matters to the Employer that may affect the terms of this Deed and the Employer's discretion to enter into this Deed;

        5. the Employee has read this Deed and agrees that its terms are fair and reasonable in the circumstances;

        6. the Employee has entered into this Deed voluntarily and of the Employee's own free will, without duress, coercion, undue influence or pressure from either the Employer or any other Person; and

        7. the Employer is relying upon these warranties in executing this Deed.

6. ## Miscellaneous {#miscellaneous}

    1. This Deed binds each of the parties and anyone who claims through a party.

    2. This Deed may be pleaded as a full and complete defence by the Employer, including as a bar, to any Claims commenced, continued or taken by or on behalf of the Employee in connection with any of the matters referred to in this Deed.

    3. The provisions of this Deed contain the entire understanding and agreement between the parties as to the subject matter of this Deed.

    4. All previous negotiations, understandings, representations, warranties, memoranda or commitments in relation to, or in any way affecting, the subject matter of this Deed are merged in and superseded by this Deed, and are of no force or effect whatever and no party will be liable to any other party in respect of those matters.

    5. If any provision of this Deed at any time is or becomes void, voidable or unenforceable, it will be severable and the remaining provisions of this Deed shall continue to be in full force and effect.

    6. This Deed is governed by and is to be construed in accordance with the laws in force in NSW.

    7. The parties will bear their own costs in connection with the preparation and execution of this Deed.

    8. This Deed may consist of a number of counterparts and if so, the counterparts taken together constitute one and the same Deed.

7. ## Non-Disparagement {#non-disparagement}

    1. The Employer undertakes not to make any statement or intimations derogatory of the Employee, and agrees not to discredit or disparage the Employee.

    2. The Employee undertakes not to make any statement or intimations derogatory of the Employer, and agrees not to discredit or disparage the Employer.

<!-- Schedule References -->
[ref-1]: #schedule "AU $10,000"
[ref-2]: #schedule ""
```

## 10. Processor Capabilities

A Lexicon Markdown processor should implement the following capabilities:

### 10.1. Parsing

1. Parse YAML front-matter and extract all meta properties.
2. Parse the document structure into a clause tree with depth levels.
3. Identify all anchors (heading-level and clause-level).
4. Identify all cross-references.
5. Identify all defined terms (formal, inline, and party-role).
6. Identify all schedule items and their reference definitions.

### 10.2. Validation

1. Validate that `date` is in `YYYY-MM-DD` format.
2. Validate that all cross-references point to existing anchors.
3. Warn on defined terms that are never used in the document text.
4. Validate that all schedule item references have a corresponding reference definition.

### 10.3. Transformation

1. Auto-resolve cross-references: replace display text with correct clause numbers.
2. Transform numbering to legal convention (`1.1`, `(a)`, `(i)`).
3. Strip anchor syntax from rendered output.
4. Generate a definitions glossary / schedule.
5. Generate a schedule of completion items from schedule references.
6. Render to `.docx` (via Pandoc with custom filters or templates) or `.pdf`.

### 10.4. Output Formats

At minimum, a processor should support:

1. **Markdown** — a "resolved" Markdown file with cross-references updated, anchors stripped, and schedule values interpolated.
2. **DOCX** — a Word document with legal numbering, formatted parties block, cover page, addendum pages, and exhibit pages.
3. **PDF** — equivalent to the DOCX output.

## 11. Summary of Syntax

| Feature              | Syntax                                          | Markdown Compatible |
| -------------------- | ----------------------------------------------- | ------------------- |
| Meta properties      | YAML front-matter (`---`)                       | Yes (widely supported) |
| Top-level clause     | `1. ## Heading`                                 | Yes |
| Sub-clauses          | Indented ordered lists (4 spaces per level)     | Yes |
| Multiple paragraphs  | Blank line + indented continuation              | Yes |
| Blockquotes          | `>`                                             | Yes |
| Sub-headings         | `### Heading` inside list item                  | Yes |
| Superscript          | `^text^`                                        | Partial (some renderers support it) |
| Formal definition    | `**Term** means ...`                            | Yes (renders as bold) |
| Inline definition    | `("**Term**")`                                  | Yes (renders as bold) |
| Term reference       | Plain text (no markup)                          | Yes |
| Heading anchor       | `## Heading {#id}`                              | Partial (Pandoc yes, others show literal text) |
| Clause anchor        | `Text {#id}`                                    | Partial (shows literal text without processor) |
| Cross-reference      | `[clause X](#id)`                               | Yes (renders as link) |
| Schedule item        | `[display text][ref-id]` + `[ref-id]: #schedule "value"` | Yes (renders as link) |
| Tables               | Standard Markdown tables                        | Yes |
| Exhibit declaration  | Front-matter `exhibits` field                   | Yes |
| Addendum content     | `# ADDENDUM` headings after main body           | Yes |
