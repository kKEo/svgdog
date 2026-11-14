# Making your own theme

This page shows how to give your diagrams your own colours, font, text sizes and line style. It takes about fifteen minutes. You need to know how to [choose a theme](themes-and-settings.md#themes) first.

A theme of your own is a folder that holds one file, `theme.toml`. You keep the folder in your project, next to your diagrams, and point SVGDog at it.

## 1. Start from a built-in theme

Pick the built-in theme closest to what you want (the [Theme gallery](theme-gallery.md) shows them all) and copy it:

```bash
svgdog --format text theme init themes/acme --from paper
```

```text
Wrote themes/acme/theme.toml (705 bytes), a copy of 'paper' named 'acme'.
Change it, then check it with: svgdog theme check themes/acme
Use it with: --theme themes/acme
```

`--from` takes `docs`, `blueprint`, `paper`, `bold` or `cards`; without it you get a copy of `docs`. The command does not replace a file that is already there unless you add `--force`.

## 2. Change the file

Open `themes/acme/theme.toml`. It has four parts:

```toml
version = 1
name = "acme"
description = "Based on paper"

[font]
family = ["Inter", "sans-serif"]
size = 14          # labels in boxes
small = 12         # arrow labels, notes, legend, steps and caption

[line]
width = 1.5        # boxes and arrows
thin = 1           # frames, notes and guide lines
corner = 12        # rounded kinds (service, client, state) and frames; notes use half
box_corner = 6     # the plain box

[light]
background = "#fbf7ef"
text = "#3b342c"
muted = "#6f6457"
node_fill = "#f1e6d3"
node_stroke = "#6b5d4d"
accent_fill = "#a84a24"
accent_text = "#ffffff"
note_fill = "#fffdf8"
edge = "#5a4e41"

[dark]
background = "#26221e"
# ... the same nine colours for the dark variant
```

### The nine colours

| Key | Where it is used |
|---|---|
| `background` | The page behind the drawing |
| `text` | Labels and other text |
| `muted` | Frames, the outline of notes, captions, and everything that is out of focus |
| `node_fill` | The inside of a box |
| `node_stroke` | The outline of a box |
| `accent_fill` | The inside of a highlighted box (`emphasis: true`), and step badges |
| `accent_text` | Text on `accent_fill` |
| `note_fill` | The inside of a note |
| `edge` | Arrows, and the axis of a timeline |

Write each colour as `#rrggbb`. `[light]` is required and needs all nine. `[dark]` is optional: leave it out and your theme has a light variant only.

### Font, sizes and lines

Everything in `[font]` and `[line]` is optional. A key you leave out takes the value of the `docs` theme.

| Key | Allowed | Default | What it does |
|---|---|---|---|
| `font.family` | list of font names | `["Inter", "sans-serif"]` | The fonts written into the drawing, first choice first |
| `font.size` | 10 to 24 | 14 | Text size of labels in boxes, in px |
| `font.small` | 8 to 20, not above `size` | 12 | Text size of arrow labels, notes, legend, steps and caption |
| `line.width` | 0.5 to 4 | 1.5 | Line width of boxes and arrows |
| `line.thin` | 0.5 to 4 | 1 | Line width of frames, notes and guide lines |
| `line.corner` | 0 to 16 | 8 | Corner radius of the rounded kinds and of frames. States use 1.5 times it; notes and `external` boxes use half |
| `line.box_corner` | 0 to 16 | 0 | Corner radius of the plain `box` |

Two things to know about fonts:

- SVGDog carries two fonts, Inter and JetBrains Mono, and measures text with one of them: JetBrains Mono when the first name in `family` is a monospace font, Inter otherwise. If you name another font, boxes are sized for the stand-in, and the reader's device draws your font if it has it. End the list with `sans-serif` or `monospace` so there is always a fallback.
- For the exact same letters everywhere, render with `--text-as-paths` (see [Text as outlines](themes-and-settings.md#text-as-outlines)).

## 3. Check it

```bash
svgdog --format text theme check themes/acme
```

```text
Theme 'acme' in themes/acme: light and dark colours.
Drew 27 example drawings with it.
27 files checked: 0 errors, 0 warnings, 0 info, 0 fixable
```

The check does three things:

1. It reads the file and reports a misspelled key, a value out of range or a colour it cannot read.
2. It compares the colours that sit on each other, such as `text` on `node_fill`, and reports any pair that is too close to read.
3. It draws every bundled example with each variant of your theme and runs the usual checks on the results.

It exits with 1 when it finds a problem. A line that starts with `Not drawn:` means an example became too wide with your text sizes; that is worth knowing, and is not counted as an error.

## 4. Use it

A theme folder goes wherever a theme name goes. SVGDog treats a value as a folder when it starts with `.` or contains `/`.

On the command line, the path is relative to where you run the command:

```bash
svgdog --format text render fig/orders.yaml -o fig/orders.svg --theme ./themes/acme
```

In a spec, the path is relative to the spec file (in a Markdown page, to the page):

```yaml
theme: ../themes/acme
```

In `svgdog.toml`, the path is relative to that file, and the theme becomes the project default:

```toml
[style]
theme = "themes/acme"
```

For the dark or the adaptive variant, add `#dark` or `#auto` after the path:

```bash
svgdog --format text render fig/orders.yaml -o fig/orders.svg --theme ./themes/acme#auto
```

## Keeping pictures up to date

A theme is part of what a picture is made from. When you change `theme.toml`, the pictures drawn with it are out of date, and `svgdog render --check` and `svgdog docs --check` report them. Render again to update them.

## Sharing a theme

A theme is one small text file, so commit the folder with your project. To use the same theme in several projects, copy the folder into each one. SVGDog does not download themes: it reads them from your disk only (see [Trust and privacy](trust-and-privacy.md)).

## When something goes wrong

| Message | What to do |
|---|---|
| `theme-not-found` | The folder does not exist or has no `theme.toml`. Check the path, and where it is relative to |
| `invalid-theme` | The file has a mistake. The message names the key, for example `light.node_fill` |
| `theme-no-dark` | You asked for `#dark` or `#auto`, and the theme has no `[dark]` table |
| `unknown-theme` | A name that is not a built-in theme. For a folder, start the value with `./` |
| `too-wide` | Larger text made the diagram wider than `max_width`. Shorten labels, use `direction: down` or raise `max_width` |
