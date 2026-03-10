# Configurable Cover Page

## Status: Implemented

The cover page is now fully configurable via the `[cover]` section in the style TOML file.

## Configuration

```toml
[cover]
enabled = true                # false: inline title block instead of full cover page
title_size = 20.0             # cover page title font size in points
date_format = "%e %B %Y"     # chrono format string
between_label = "BETWEEN"    # heading above parties block
party_format = "name_spec_role" # "name_spec_role", "name_role", or "name_only"
show_ref = true               # show reference number on cover
show_author = true            # show author/firm on cover
show_status = true            # show status and version on cover
```

All fields are optional with sensible defaults matching the original hardcoded values.

## Party format options

| Value            | Renders as                                          |
|------------------|-----------------------------------------------------|
| `name_spec_role` | **Name** (specifier) + (the "Role") — default       |
| `name_role`      | **Name** + (the "Role") — no specifier              |
| `name_only`      | **Name** only — no specifier or role line            |

## Design decision

Cover page and TOC toggles live in the style TOML rather than the YAML front-matter, because they are rendering/output concerns — the front-matter contains contract identity (title, parties, date, status), while the TOML controls how that identity is presented in Word output. This allows the same contract to be rendered differently for different output contexts.
