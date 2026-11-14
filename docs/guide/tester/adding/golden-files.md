# Golden files

This page explains what the golden files in `tests/golden/` are, how to regenerate them when a change is intended, and how to review the result.

## What they are

A golden file is a saved expected output. The test `r8_output_matches_the_golden_files` in `tests/cli.rs` runs SVGDog, compares the bytes it produces with the saved files, and fails on any difference. Because SVGDog promises identical output for identical input (requirement R8), a one-byte change is a finding.

The test produces and compares these files, among others:

| Golden file | Produced by |
|---|---|
| `orders.svg`, `checkout.svg`, `tiers.svg`, `rack.svg`, `login.svg` | `svgdog render tests/fixtures/<name>.yaml -o <name>.svg --no-config` |
| `orders-dark-paths.svg` | `render` of `orders.yaml` with `--theme docs-dark --text-as-paths` |
| `orders-blueprint-dark.svg`, `walkthrough-paper.svg`, `login-bold-auto.svg` | `render` of the fixture in the name with `--theme blueprint-dark`, `paper` and `bold-auto`: one drawing per built-in theme beyond the default |
| `broken.findings.json` | `svgdog validate tests/fixtures/broken.svg --no-config --max-findings 500` (the file path is replaced by `broken.svg`) |
| `orders.scene.json` | `svgdog inspect orders.svg --fields id,tag,bbox,text,fill,stroke,refs` |
| `orders.png` | `svgdog preview orders.svg -o orders.png --width 800 --annotate --no-config` |

Together they exercise the layout and routing code, every built-in theme, the text-to-path emitter, the finding format, the scene format, and the PNG rasterizer with the bundled fonts.

![Preview image of the orders diagram, 800 pixels wide: a Storefront client box on the left sends POST /orders to an Order API service inside a Backend group, which inserts into an Orders DB cylinder and publishes through a dashed arrow to a Fulfilment queue with an At-least-once delivery note](../images/orders.png)

The picture above is `tests/golden/orders.png`, copied into this book by `docs/sync-assets.sh`.

## When a golden test fails

Read the failure message first:

```text
orders.svg differs from tests/golden/orders.svg (3120 vs 3052 bytes). If the change is intended, regenerate with SVGDOG_UPDATE_GOLDEN=1.
```

Then decide:

| Situation | Action |
|---|---|
| You changed layout, routing, colours, text measuring, finding wording or the scene format on purpose | regenerate, review the diff, commit |
| You changed something unrelated and a golden file moved | do not regenerate; find the unintended side effect |
| CI fails on one system only | do not regenerate; this is a portability bug, see [Continuous integration gates](../gates/ci.md#when-one-system-fails) |
| `tests/golden/<name>` is missing | the test panics and tells you to regenerate; this happens when a new output is added to the test |

## Regenerate

```bash
SVGDOG_UPDATE_GOLDEN=1 cargo test r8_output_matches
```

With that variable set, the test writes every produced file into `tests/golden/` instead of comparing, and passes. Run it again without the variable to confirm:

```bash
cargo test r8_output_matches
```

Then review exactly what moved:

```bash
git status tests/golden
git diff --stat tests/golden
git diff tests/golden/orders.svg          # SVG and JSON diff as text
```

Look at the PNG with your image viewer, or render a fresh one: `svgdog preview tests/golden/orders.svg -o /tmp/orders.png`.

> **Warning.** Regenerating on one machine proves nothing about the others. Push the regenerated files and wait for both CI jobs to pass before you merge.

## Keep the documentation pictures in step

The books under `docs/guide/` show golden diagrams. After regenerating, copy the new files into the books:

```bash
./docs/sync-assets.sh
```

## Review checklist for a golden diff

- [ ] Every changed file is explained by the change in the pull request.
- [ ] SVG diffs show coordinates moving by the amounts you expect, not wholesale reordering.
- [ ] No absolute path (such as `/Users/…` or `C:\…`) appears in `broken.findings.json` or `orders.scene.json`.
- [ ] Line endings are LF (`.gitattributes` enforces this for `*.svg`, `*.yaml`, `*.json`).
- [ ] `orders.png` still looks right to a human.
