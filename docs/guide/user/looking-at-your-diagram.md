# Looking at your diagram

An SVG file is text, so you cannot judge it by reading it. `svgdog preview` draws it into a PNG picture that you can open in any viewer or hand to an AI assistant. This page takes about five minutes.

`preview` writes a file and finishes. It does not open a window, run a server or refresh on its own: run it again after each change.

## Make a picture

```bash
svgdog --format text preview hello.svg -o hello.png
```

```text
Wrote hello.png (1200 x 191 px, 15710 bytes).
```

The picture is 1200 px wide by default. SVGDog draws it with the fonts built into the program, so the result is the same on every computer and needs no browser.

## Options

| Option | What it does |
|---|---|
| `--width N` | Picture width in pixels. Default 1200; the largest is 2000 and the smallest is 16 |
| `--background light` or `dark` | The page colour behind a transparent drawing. Default `light` |
| `--annotate` | Draws a box around every problem SVGDog found and labels it with the rule name |
| `--crop '#id'` | Zooms in on one element (or elements matching a selector), with a little padding |
| `--force` | Replace a picture that already exists |

If you ask for more than 2000 px, SVGDog gives you 2000 and says so: `--width 5000 is over the limit; the image is 2000 px wide.`

## Find problems visually

Run `preview` with `--annotate` on a file that has findings. On the broken example, 8 boxes are outlined:

```bash
svgdog --format text preview tests/fixtures/broken.svg -o broken.png --annotate
```

```text
Wrote broken.png (1200 x 600 px, 30212 bytes).
8 finding boxes outlined.
```

The findings are listed below that message, the same as `validate`. Open the picture and match each outlined box to a rule name.

## Zoom in

To check a detail, crop to one element. Element ids in a picture SVGDog drew are `node-<id>`, `edge-<from>--<to>`, `group-<id>` and `note-<number>`:

```bash
svgdog --format text preview hello.svg -o api.png --crop '#node-api'
```

```text
Wrote api.png (1200 x 564 px, 18066 bytes).
Shows the region 170.88 0 170.25 80 of the viewBox.
```

If the selector matches nothing, SVGDog says `error[no-match]` and exits with code 3.

## Transparent drawings and the dark page

Diagrams that SVGDog draws carry their own background colour, so `--background` makes no difference to them. It matters for SVG files from other tools that have a transparent background: preview those on both `light` and `dark` to see whether they stay readable on both. To draw a diagram with a dark look, see [Themes and settings](themes-and-settings.md).

## Overwrite protection

If the picture already exists, SVGDog refuses and exits with code 4 unless you add `--force`.
