# Commands

SVGDog has nine subcommands. This page lists each one with every flag, its default and an example. Global flags come first because they apply to all of them.

## Global flags

They may be placed before or after the subcommand.

| Flag | Meaning | Default |
|---|---|---|
| `--format json\|text` | Output form | `text` on a terminal, `json` otherwise |
| `--config PATH` | Use this `svgdog.toml`; conflicts with `--no-config` | nearest `svgdog.toml` above the input file |
| `--no-config` | Ignore any config file | off |
| `--profile NAME` | Rule profile: `minimal`, `docs` or `strict` | `docs`, or the config's `profile` |
| `--quiet` | Suppress the stderr log line `svgdog: config <path>` | off |
| `--no-color` | Plain text output; the `NO_COLOR` variable is also honoured | off |
| `--version`, `--help` | Print the version or help and exit 0 | |

Colour is used only for text output on a terminal. See [Configuration](configuration.md) for profiles and [Envelope and exit codes](envelope-exit-codes.md) for the result shape.

Only `validate` has `--fail-on`. Every other command that checks a drawing (`render`, `edit`, `fix`) fails with exit 1 on findings of severity `error`; `preview` never fails because of findings.

## render

Compiles a spec to SVG, then validates the result.

```bash
svgdog render fig/orders.yaml -o fig/orders.svg
```

| Argument or flag | Meaning | Default |
|---|---|---|
| `SPEC` | Diagram spec, YAML or JSON; `-` reads stdin | required |
| `-o OUT` | Output SVG; `-` writes the SVG to stdout in place of the report | required |
| `--theme NAME` | `docs-light` or `docs-dark` | the spec's `theme`, else `docs-light` |
| `--no-validate` | Skip the checks on the result (the envelope carries `"validated": false`) | off |
| `--force` | Replace an existing output file | off (an existing file gives exit 4) |
| `--text-as-paths` | Convert text to outlines: identical everywhere, but not searchable | off |

Findings are listed up to 50. Exit 1 when an error-level finding remains. Writes are atomic (temporary file, then rename). `[style] fonts` in `svgdog.toml` sets the font-family list written into the SVG.

## validate

Runs the rules on one or more SVG files.

```bash
svgdog validate 'docs/**/*.svg' --fail-on warning
```

| Argument or flag | Meaning | Default |
|---|---|---|
| `FILE...` | SVG files or globs (quote globs so the tool expands them); `-` reads stdin, once at most | required |
| `--rules ID,...` | Run only these rules | all enabled rules |
| `--skip ID,...` | Skip these rules | none |
| `--fail-on error\|warning` | Lowest severity that gives exit 1 | `error` |
| `--max-findings N` | Cap on listed findings; summary counts stay complete and `summary.truncated` is set | 50 |

Each file gets the config nearest to it, so one run can mix projects. A glob that matches nothing is exit 3 (`file-not-found`). Unknown rule ids are exit 2 (`unknown-rule`) with a did-you-mean hint. `--rules` turns a listed rule on even if the profile or config turned it off.

## inspect

Describes an SVG's elements as data (the scene), with bounding boxes in viewBox units.

```bash
svgdog inspect fig/orders.svg --select '#node-api' --fields id,tag,bbox,text
```

| Flag | Meaning | Default |
|---|---|---|
| `FILE` | SVG file; `-` reads stdin | required |
| `--select SELECTOR` | Keep matching elements and their descendants: `#id`, `tag` or `tag[attr="value"]` | all elements |
| `--depth N` | Deepest nesting level to return; the root `<svg>` is 0 | unlimited |
| `--fields LIST` | Any of `id`, `tag`, `bbox`, `text`, `fill`, `stroke`, `refs` | `id,tag,bbox,text` |

An unknown field is exit 2. The result is the scene envelope described by `svgdog schema scene`. `inspect` never reads a config and exits 0 when it succeeds.

## preview

Rasterizes an SVG to a PNG file with the bundled fonts. It writes a still image only; there is no server, watch mode or live reload.

```bash
svgdog preview fig/orders.svg -o /tmp/orders.png --annotate
```

| Flag | Meaning | Default |
|---|---|---|
| `FILE` | SVG file; `-` reads stdin | required |
| `-o OUT` | PNG to write; `-` writes the PNG bytes to stdout | required |
| `--width PX` | Image width; at least 16, values above 2000 are cut to 2000 | 1200 |
| `--background light\|dark` | Page colour behind a transparent drawing | `light` |
| `--annotate` | Outline each finding's box and print its rule id beside it | off |
| `--crop SELECTOR` | Show only the matching elements plus surroundings; no match is exit 3 (`no-match`) | whole drawing |
| `--force` | Replace an existing output file | off |

Height is capped at 4000 px. The envelope reports `image` (width, height, scale, region, background, annotated). With `--annotate` the findings are included, but they do not change the exit code.

## edit

Applies targeted changes to an SVG, or to a diagram spec.

```bash
svgdog edit fig/orders.svg --set '#node-api > rect.fill=#2563eb' --dry-run
svgdog edit fig/orders.yaml --set '#api.label=Orders API' --in-place
```

| Flag | Meaning | Default |
|---|---|---|
| `FILE` | SVG or spec; any input not starting with `<` is a spec; `-` reads stdin | required |
| `--set SELECTOR.attr=value` | Set one attribute on every matching element; repeatable | |
| `--ops FILE\|-` | JSON operations: `set`, `remove`, `move`, `rename-id`, `add-node`, `add-edge` (see `svgdog schema ops`) | |
| `--dry-run` | Print the diff and resulting findings; write nothing | off |
| `-o OUT` | Write the result to a new file; `-` writes the text to stdout | |
| `--in-place` | Change the input file | off |

Give `--set` or `--ops`, not both, and exactly one destination (`--dry-run`, `-o` or `--in-place`); otherwise exit 2 (`missing-argument` or `argument-conflict`). An existing `-o` target is not overwritten (exit 4). On an SVG, text outside the targeted elements is left untouched. On a spec, targets are `#id` or `:root`, the spec is re-rendered to check it, and the envelope's `formatting` says `preserved` (comments and layout kept) or `rewritten` (canonical form). Removing a node, group or edge, and any edit of a JSON spec, always rewrites. A change that would leave the file malformed is refused (`edit-breaks-xml`); two operations changing the same bytes are refused (`conflicting-ops`).

## fix

Applies the autofixes that findings carry.

```bash
svgdog fix fig/orders.svg --unsafe --dry-run
```

| Flag | Meaning | Default |
|---|---|---|
| `FILE` | SVG file; `-` reads stdin | required |
| `--rules ID,...` | Fix only these rules | all |
| `--unsafe` | Also apply fixes that move or recolour content | off (safe fixes only) |
| `--dry-run`, `-o OUT`, `--in-place` | Destination, as for `edit` | one is required |

It repeats up to 30 rounds until a round changes nothing, because a fix can expose another (a new viewBox makes margins measurable). Fixes that depend on scale (`stroke-too-thin`, `font-too-small`) wait until the viewBox-changing fixes (`viewbox-missing`, `content-clipped`, `excess-margin`, `text-overflow`) have settled. If two fixes undo each other the loop stops. The envelope lists `fixed` as rule and count pairs, and its findings are those left after fixing.

## mcp

Serves the commands as Model Context Protocol tools over stdio.

```bash
svgdog mcp --root ./docs
```

`--root DIR` is required: the directory the server may read and write. See [MCP tools](mcp-tools.md).

## schema

Prints an embedded JSON Schema.

```bash
svgdog schema findings
```

The argument is one of `spec`, `config`, `findings`, `scene`, `ops`. See [JSON Schemas](schemas.md).

## rules

Lists the rules, or explains one.

```bash
svgdog rules
svgdog rules --explain text-overflow
```

`--explain ID` prints the rationale, thresholds and a failing and passing example; an unknown id is exit 2 with a hint. The full catalogue is in [Rules catalogue](rules.md).

## Exit codes

| Code | Meaning |
|---|---|
| 0 | OK |
| 1 | Findings at or above the fail level |
| 2 | Usage error: bad flag, missing argument, unknown rule or profile |
| 3 | Input error: file missing, spec invalid, config invalid, no match |
| 4 | Output error: file exists, write failed |
| 5 | Internal error: a bug, reported in the usual shape |

Details and the error codes are in [Envelope and exit codes](envelope-exit-codes.md).
