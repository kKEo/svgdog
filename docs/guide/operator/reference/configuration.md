# Configuration

SVGDog reads an optional `svgdog.toml` that sets the rule profile, the target the drawings are checked against, the house style, per-rule severities and thresholds, and per-file overrides. Without a file, the `docs` profile and the catalogue defaults apply. A single element can also opt out of a rule with a `svgdog-disable` comment.

## Discovery

For each input file, SVGDog looks for `svgdog.toml` in the file's directory, then in each parent directory, and uses the first one found. Rules:

- `--config PATH` uses that file for every input and skips discovery. `--no-config` ignores all config files. The two conflict.
- For stdin (`-`) the search starts in the working directory.
- Under `svgdog mcp`, the search stops at the server's root directory.
- Each config that is read is logged to stderr as `svgdog: config <path>`; `--quiet` hides it.
- Only `validate`, `render`, `edit`, `fix` and `preview` read a config. `inspect`, `schema` and `rules` do not.
- Overrides match paths relative to the directory holding the config file.

A config is parsed strictly. An unknown key is exit 3 with a did-you-mean hint (`unknown-field`), an unknown rule is `unknown-rule`, an unknown profile is `unknown-profile`, and any other invalid value is `invalid-config`.

```bash
$ svgdog validate orders.svg --config bad.toml --format json
{"tool":"svgdog","version":"1.0.0","command":"validate","ok":false,"error":{"code":"unknown-field","message":"bad.toml: unknown key 'profle'.","path":"profle","suggestion":"Did you mean 'profile'?"}}
```

## Keys

| Key | Type | Default | Meaning |
|---|---|---|---|
| `version` | integer | | If present, must be `1` |
| `profile` | string | `docs` | `minimal`, `docs` or `strict`; `--profile` overrides it |
| `[target] render_width` | number | 720 | Pixel width the drawing is shown at; must be positive. The viewBox is scaled to it before size rules (`font-too-small`, `stroke-too-thin`, contrast of large text) measure |
| `[target] backgrounds` | list of strings | `["light", "dark"]` | Page backgrounds the drawing must be legible on: `light`, `dark` or a colour such as `#f5f5f5`; at least one. The first is the page for `low-contrast`; `background-missing` runs when a dark page is listed |
| `[style] fonts` | list of strings | none | Font-family list `render` writes into the SVG |
| `[style] palette` | list of colours | none (unchecked) | Allowed fill and stroke colours for the `palette` rule |
| `[style] stroke_widths` | list of numbers | none (unchecked) | Allowed stroke widths for `stroke-width-style` |
| `[style] grid` | number | none (unchecked) | Grid size in viewBox units for `off-grid`; must be positive |
| `[rules]` | table | | Per-rule severity and thresholds |
| `[[overrides]]` | array of tables | | Per-file rule settings |

## Profiles

A profile sets the starting severity of every rule before `[rules]`, overrides and flags are applied.

| Profile | Effect |
|---|---|
| `minimal` | Only the structure rules, plus `script-content` and `external-resource`, at their catalogue severity; everything else is off |
| `docs` (default) | Every rule at its catalogue severity |
| `strict` | Every rule on; catalogue `info` becomes `warning`, and `warning` and `error` become `error` |

## The `[rules]` table

Each entry is a rule id. The value is either a severity string, or a table with an optional `severity` and the rule's thresholds.

```toml
[rules]
excess-margin = "off"                                  # off, info, warning or error
text-overflow = { severity = "warning", padding = 6 }  # severity plus a threshold
```

- A severity is `off`, `info`, `warning` or `error`.
- Threshold keys are specific to each rule and must be non-negative numbers; they are listed per rule in the [Rules catalogue](rules.md) and by `svgdog rules --explain ID`. An unknown threshold is `unknown-field`.

## Overrides

`[[overrides]]` blocks change rule settings for files whose path matches a glob. Each block has `files` (a list of globs) and `rules` (same form as `[rules]`).

```toml
[[overrides]]
files = ["legacy/*.svg"]
rules = { palette = "off", text-overflow = "warning" }
```

Globs are case sensitive, `*` does not cross `/`, and the path is relative to the config's directory. When several blocks match, later blocks win.

## Precedence

From highest to lowest, for each rule on each file:

1. Command-line selection: `--rules` (runs only those; turns a listed rule on at its catalogue severity if it was off) and `--skip` (turns a rule off)
2. `[[overrides]]` matching the file, later blocks winning
3. `[rules]`
4. The profile (`--profile`, else the config's `profile`, else `docs`)

## A complete example

```toml
version = 1
profile = "strict"

[target]
render_width = 640
backgrounds = ["light", "dark", "#f5f5f5"]

[style]
fonts = ["Inter", "sans-serif"]
palette = ["#2563eb", "#1f2937"]
stroke_widths = [1, 1.5]
grid = 8

[rules]
excess-margin = "off"
text-overflow = { severity = "warning", padding = 6 }

[[overrides]]
files = ["sub/*.svg"]
rules = { palette = "off" }
```

The schema for this file is `svgdog schema config`.

## Suppressing a rule on one element

A comment placed directly before an element silences the named rules for that element and everything inside it:

```xml
<!-- svgdog-disable text-overlap: label sits on purpose over the legend swatch -->
<text x="40" y="20">Legend</text>
```

Format: `svgdog-disable <rule>[, <rule>...]: <reason>`. The reason is mandatory. Anything wrong with the comment is reported as an `invalid-suppression` error: no `:` or an empty reason, an unknown rule id (with a did-you-mean hint), no rule named, or no element directly after the comment (only whitespace may sit between them). `invalid-suppression` is not a catalogue rule, so it cannot be configured or suppressed.

Findings from geometry rules on an SVG that SVGDog did not draw are marked `approximate` in `measured` and are capped at `warning`, because positions in foreign files are estimated.
