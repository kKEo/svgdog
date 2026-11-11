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
| `--theme NAME` | `docs-light`, `docs-dark` or `docs-auto` (follows the reader's light or dark mode) | the spec's `theme`, else `docs-light` |
| `--interactive none\|css\|html` | `css`: the same SVG with classes and one `<style>` block, so related things light up on hover and keyboard focus; no script. `html`: a self-contained page holding that SVG and a small script with step controls and a detail line. See [Interactive versions](#interactive-versions) | `none` |
| `--frames` | Write one picture per entry of the spec's `steps`, named `OUT` with the step number before the extension (`orders.1.svg`, `orders.2.svg`, ...); `OUT` itself is not written | off |
| `--embed-spec` | Store the spec in the SVG as `<metadata id="svgdog-spec">` (compact JSON), so `describe` can read the drawing back. The `editor-metadata` rule and its fix leave this element alone | off |
| `--check` | Write nothing. Render in memory and compare with `OUT` byte for byte; exit 1 if it is missing or differs. Cannot be combined with `--force` or `-o -` | off |
| `--no-validate` | Skip the checks on the result (the envelope carries `"validated": false`) | off |
| `--force` | Replace an existing output file | off (an existing file gives exit 4) |
| `--text-as-paths` | Convert text to outlines: identical everywhere, but not searchable | off |

With `--frames`, every frame is checked and listed in `outputs`, and `summary.files` counts them. A spec without `steps` is exit 3 (`no-steps`), and `-o -` is exit 2. If any frame file exists and `--force` is not given, nothing is written (exit 4).

With `--check` the envelope has `checked` (the number of files compared) and `stale` (a list of `path` and `reason`, `missing` or `differs`) in place of findings; `ok` is false when anything is stale. Give `--check` the same flags the file was rendered with (`--theme`, `--embed-spec`, `--frames`), or it will differ.

Findings are listed up to 50. Exit 1 when an error-level finding remains. Writes are atomic (temporary file, then rename). `[style] fonts` in `svgdog.toml` sets the font-family list written into the SVG.

### Interactive versions

Both versions are the static drawing with something added, never a different drawing.

**`css`** adds, and only adds:

- on the root: `data-interactive="css"` and a scope class `svgdog-<8 hex digits>`, a hash of the drawing, so two diagrams inlined in one page keep their rules apart;
- on every node, edge, group and note: the class `it`, one `r-<id>` class per node or group it stays lit with (itself, what it is joined to, the frames around it), and `st-<N>` per step that shows it;
- on every node: `tabindex="0"`, so the keyboard reaches it;
- one `<style>` block: per node, a rule that fades everything unrelated while the node is hovered or focused (it uses `:has()`); per step, the same while that step's line in the list is hovered, or while the root carries `data-current="N"`.

Take those away and the file is the static output byte for byte, and `preview` draws the same picture from both. Hover and focus work when the SVG is inlined in a page or loaded with `<object>`. As an `<img>`, which is what Markdown image syntax gives, it is simply the static drawing. The file contains no script, so it passes every rule, `script-content` included.

**`html`** writes one `.html` file: the `css` SVG inline, and a script of about fifty lines compiled into the binary. The script shows Previous, Next and Show all buttons when the spec has steps (arrow keys and Escape do the same, and a click on a step's line chooses it), and prints an item's `detail` under the drawing when the item is clicked. It works by setting `data-current` on the SVG; the style block does the rest. The page loads nothing from anywhere. Without the script the buttons stay hidden and the page shows the complete drawing with its numbered step list. Findings are those of the SVG inside. `--frames` cannot be combined with it.

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
| `--step N` | Show the drawing with step `N` of its walk-through chosen: what the step does not show is faded, as the interactive versions fade it. Exit 3 (`no-match`) if the SVG has no such step | off |
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

## docs

Renders the diagrams written inside Markdown pages.

```bash
svgdog docs docs/
svgdog docs docs/ README.md --check
```

| Argument or flag | Meaning | Default |
|---|---|---|
| `PATH...` | Markdown files, or folders searched for `.md` files (hidden folders, `target` and `node_modules` are skipped) | required |
| `--check` | Write nothing; exit 1 if any picture or image line is out of date | off |
| `--assets DIR` | Folder for the pictures, relative to each page; no `..` | `diagrams` |
| `--theme NAME` | Theme for every diagram | each spec's `theme`, else `docs-light` |
| `--interactive none\|css\|html` | `css`: the pictures are the `css` version. `html`: the pictures stay static, a page `DIR/<page>-<name>.html` is written beside each, and the image line ends with a link to it, labelled "Interactive version" | `none` |

A diagram is a fenced code block whose info word is `svgdog`, or, when the source should not be shown to readers, an HTML comment that opens with `<!-- svgdog` on its own line and closes with `-->` on its own line. An optional second word names the diagram.

````markdown
```svgdog request-path
version: 1
diagram: flow
title: Request path
edges:
  - "browser -> api: request"
```

![Request path. Flow diagram: browser to api (request).](diagrams/page-request-path.svg)
````

For each block the command:

1. renders the spec to `DIR/<page>-<name>.svg` next to the page, where `<page>` is the page's file name in lower case without `.md` and `<name>` is the block's name or its number on the page;
2. writes the file only if its bytes would change;
3. puts an image line after the block, or replaces the image line that is already there if it points into `DIR/`. The alt text is the spec's title followed by its description, or by the generated sentence.

Nothing else in the page is touched, and a second run writes nothing. Pictures of blocks that were removed or renamed are not deleted.

The envelope lists what was written in `outputs`, the pages that hold diagrams in `documents`, and the findings of every rendered picture; `summary.files` counts the diagrams. With `--check` it also has `stale` (`path` and `reason`: `missing`, `differs` or `image links differ`). A spec error ends the run with exit 3; its message names the page and the line the diagram starts on. Two diagrams on one page that would get the same file name are refused (`duplicate-id`).

## describe

Says in words what a diagram shows.

```bash
svgdog describe fig/orders.yaml --format text
svgdog describe fig/orders.svg --short --format text
```

| Argument or flag | Meaning | Default |
|---|---|---|
| `FILE` | A spec, or an SVG rendered with `--embed-spec`; `-` reads stdin | required |
| `--short` | Text output is one or two sentences, as for alt text | off |

The envelope carries `title`, `short` (the sentence also used for a generated `<desc>`), `text` (title, description, nodes, connections, notes, steps and caption, in that order) and `spec` (the spec as data, which is how an agent recovers the source from a drawing). An SVG without an embedded spec is exit 3 (`no-spec`); for such a file use `inspect`.

## examples

Lists the example specs bundled in the binary, or prints one.

```bash
svgdog examples
svgdog examples minimal --format text > first.yaml
```

Without a name, each example is listed with its diagram type and one line about what it shows. With a name, the JSON envelope carries the spec text in `examples[0].spec`, and `--format text` prints the spec alone. An unknown name is exit 2 (`unknown-example`) with a hint. The tests render every example in both themes under the `strict` profile and require zero findings, so each one is a safe starting point.

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
