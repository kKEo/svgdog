# Themes and settings

This page shows how to change how diagrams look and how to set house rules for a whole project. It takes about ten minutes.

## Light and dark themes

SVGDog has three built-in themes:

- `docs-light`: dark text on a white page. This is the default.
- `docs-dark`: light text on a dark page (`#0d1117`).
- `docs-auto`: one file that follows the reader. It is the light drawing, with a rule inside that switches to the dark colours when the reader's system is in dark mode.

Choose one in the spec:

```yaml
theme: docs-dark
```

or for a single run, on the command line (the command line wins over the spec):

```bash
svgdog --format text render hello.yaml -o hello-dark.svg --theme docs-dark
```

Use `docs-auto` when the same page is read in light and dark mode, for example a README. It works wherever a browser shows the SVG, including as a plain image. A viewer that does not understand the rule shows the light version, and so does `svgdog preview`. To see the dark version, render once with `--theme docs-dark`.

All themes use the same sizes: text at 14 px (12 px for arrow labels), lines 1.5 px thick (1 px for the thinner ones).

## Text as outlines

By default the picture keeps its text as real text, so it can be searched, selected and read by screen readers. If you need the picture to look exactly the same in programs that lack the Inter font, ask for outlines:

```bash
svgdog --format text render hello.yaml -o hello.svg --text-as-paths
```

The result looks the same everywhere but the words can no longer be searched or read aloud, so prefer the default.

## House rules: svgdog.toml

A project can keep its settings in one file called `svgdog.toml`. SVGDog looks for it in the folder of the file you check, then in each folder above it, and uses the nearest one. When it finds one, it prints a line such as `svgdog: config /path/to/svgdog.toml` to the error output.

```toml
version = 1
profile = "docs"                 # minimal, docs or strict

[target]
render_width = 720               # px the diagram is shown at
backgrounds = ["light", "dark"]

[style]
palette = ["#1f2937", "#ffffff", "#2563eb", "#dbeafe", "#6b7280"]
stroke_widths = [1, 1.5, 2]
grid = 8

[rules]
excess-margin = "off"
font-too-small = { severity = "error", min_px = 10 }
```

What the parts mean:

- `profile`: how strict the checks are (see below).
- `[target]`: how big the diagram will be shown, and on which page colours. Rules about text size and contrast use these numbers.
- `[style]`: the colours, line widths and grid your project allows. Colours outside the palette are reported.
- `[rules]`: switch a rule `"off"`, or change its severity or limits.

To ignore the file for one run, add `--no-config`. To use a specific file, add `--config path/to/svgdog.toml`. Every setting is described in the [Operator guide](../operator/).

## Profiles: how strict to be

| Profile | What it checks | When to use it |
|---|---|---|
| `minimal` | Only whether the file is valid and safe (structure rules, scripts, external resources) | Pictures from other tools that you cannot change |
| `docs` | Everything, at each rule's normal severity | The default; good for documentation |
| `strict` | Everything, with every level raised one step: info becomes warning, warning becomes error | Releases and shared style libraries |

Set a profile in `svgdog.toml` or per run with `--profile strict`. On the broken example file the three profiles report 3 errors (`minimal`), 7 errors (`docs`) and 10 errors (`strict`).
