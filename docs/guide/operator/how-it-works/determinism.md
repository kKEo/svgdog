# Determinism and output ids

SVGDog promises that the same input, configuration and version give byte-identical output, on every operating system. This chapter says how that is achieved, how it is checked, and which ids in the output you can depend on when you edit or select elements.

## What makes the output repeatable

- **No external inputs.** Rendering reads only the spec. There are no timestamps, random numbers or environment lookups in the render path.
- **Bundled fonts.** Text is measured and rasterised with Inter and JetBrains Mono, which are compiled into the binary (`fonts/`, exposed by `text::bundled()` in `src/svg/text.rs` and loaded in `src/preview.rs:16-27`). Every generic family, and any font a file names that SVGDog does not have, falls back to one of the two. A font installed on the machine cannot change a measurement or a PNG. The two fonts are under the SIL Open Font License; the licence texts are in `fonts/`.
- **Ordered data.** Layout and routing walk lists in spec order, and the port bookkeeping uses ordered maps (`BTreeMap`), so iteration order never depends on hashing.
- **Rounded numbers.** Every coordinate is written through `util::fmt`, which rounds to two decimals and drops a zero fraction (`src/util.rs:19`). Integer-grid layout means most numbers are whole.
- **Grid snapping.** Boxes and ports sit on the 8 px grid, so floating-point noise cannot move them by a visible amount.

You can check it yourself:

```bash
svgdog render tests/fixtures/orders.yaml -o a.svg
svgdog render tests/fixtures/orders.yaml -o b.svg
cmp a.svg b.svg && echo identical
cmp a.svg tests/golden/orders.svg && echo "matches golden"
```

Both `cmp` commands print their message when run on SVGDog 1.0.0.

## How it is verified

The test `r8_output_matches_the_golden_files` (`tests/cli.rs:1109`) renders the five fixture specs, a dark-theme variant with text as paths, a findings listing, an inspect listing and a PNG, and compares each with the file in `tests/golden/` byte for byte. CI runs it on Linux, macOS and Windows (`.github/workflows/ci.yml`). The three passing together is the cross-platform proof. After an intended change to output, regenerate with `SVGDOG_UPDATE_GOLDEN=1 cargo test r8_output_matches`; see the [tester guide](../../tester/) for the full procedure.

> **Note.** The guarantee covers the same SVGDog version. A new release may change layout or routing and with it the bytes. Pin the version in CI if you commit rendered SVGs and want diffs only when the spec changes.

## Output ids

A rendered SVG carries `data-svgdog="1"` on its root, which marks it as SVGDog's own output. Elements get stable ids built from the spec ids (`src/render/mod.rs:603-648`):

| Element | Id | Example |
| --- | --- | --- |
| Group frame | `group-<id>` | `group-backend` |
| Node | `node-<id>` | `node-api` |
| Edge | `edge-<id>` | `edge-api--db` (default `<from>--<to>`) |
| Note | `note-<n>`, counting from 1 in spec order | `note-1` |

These are what `svgdog inspect --select '#node-api'` and `svgdog edit --set '#node-api.fill=#ffffff'` target. They are also why an id in a spec should be a short lowercase slug: it becomes part of an SVG id.

The `data-svgdog` mark matters for checking: geometry rules are exact on SVGDog output and heuristic on files SVGDog did not make. See [The rule engine](rules-engine.md).

## Text as paths

`render --text-as-paths` writes each label as outlines instead of `<text>`, for pipelines that do not carry the font. The root then has no `font-family`. The output is just as deterministic; it is also larger, and the text is no longer selectable or searchable.
