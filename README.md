# svgdoc

A single-binary command line tool that compiles a declarative diagram description
to SVG and checks any SVG for correctness, legibility and portability. It is built
for diagrams in technical documentation, and for two kinds of caller: AI agents
that use it as a tool, and people or CI pipelines that lint diagrams in a
documentation repository.

## Build

```sh
cargo build --release      # the binary is target/release/svgdoc
cargo test                 # unit, command line and MCP tests
```

No network, browser or language runtime is needed at run time. Fonts (Inter and
JetBrains Mono, both under the SIL Open Font License, see `fonts/`) are compiled in.

## The loop

```sh
svgdoc schema spec                       # learn the spec format, once
svgdoc render orders.yaml -o orders.svg  # compile and check in one call
svgdoc fix orders.svg --in-place         # apply the safe fixes findings carry
svgdoc preview orders.svg -o orders.png --annotate   # look at it
```

Repeat render (or fix) until the exit code is 0.

## Commands

| Command | Purpose |
| --- | --- |
| `render <spec> -o <out.svg>` | Compile a YAML or JSON spec to SVG and validate the result |
| `validate <file.svg>...` | Run the 38 rules on SVG files, including ones svgdoc did not make |
| `inspect <file.svg>` | List visible elements with their bounding boxes, as data |
| `preview <file.svg> -o <out.png>` | Rasterize to PNG; `--annotate` outlines findings, `--crop` zooms in |
| `edit <file> --set ... \| --ops ...` | Targeted changes to an SVG or a spec |
| `fix <file.svg>` | Apply autofixes; `--unsafe` adds those that move or recolour |
| `schema spec\|config\|findings\|scene\|ops` | Print a JSON Schema |
| `rules [--explain ID]` | List rules, or explain one with a failing and a passing example |
| `mcp --root DIR` | Serve the commands as Model Context Protocol tools over stdio |

Every command has `--help` with one example. With `--format json` (the default
when stdout is not a terminal) each command prints exactly one JSON object.

Exit codes: `0` success, `1` findings at or above `--fail-on`, `2` usage error,
`3` input error, `4` output error, `5` internal error.

## A spec

```yaml
version: 1
diagram: flow            # flow, layers, sequence or free
title: Order placement
direction: right

nodes:
  - {id: web, label: Storefront, kind: client}
  - {id: api, label: Order API, kind: service, emphasis: true}
  - {id: db, label: Orders DB, kind: datastore}

groups:
  - {id: backend, label: Backend, members: [api, db]}

edges:
  - {from: web, to: api, label: POST /orders}
  - {from: api, to: db, label: insert}

notes:
  - {attach: db, text: Nightly backup}
```

The author states structure; svgdoc chooses every coordinate. More examples are in
`tests/fixtures/`.

## Configuration

One `svgdoc.toml`, found by walking up from the input file:

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

A rule is silenced for one element with a comment directly before it; the reason
is required:

```xml
<!-- svgdoc-disable shape-overlap: badge sits on the corner by design -->
```

## MCP

```sh
svgdoc mcp --root ./docs
```

The server exposes `svgdoc_render`, `svgdoc_validate`, `svgdoc_inspect`,
`svgdoc_preview`, `svgdoc_edit`, `svgdoc_fix`, `svgdoc_schema` and `svgdoc_rules`.
Inputs mirror the flags and results are the same JSON envelope. Every path is
relative to the root and paths outside it are refused. It serves both the
`initialize` handshake and the per-request protocol style of MCP 2026-07-28.

## Development notes

- `tests/golden/` holds expected output, compared byte for byte. After an intended
  change, regenerate with `SVGDOC_UPDATE_GOLDEN=1 cargo test r8_output_matches`.
- `.github/workflows/ci.yml` runs the tests on Linux, macOS and Windows; passing on
  all three is the cross-platform determinism check.
- `.github/workflows/release.yml` publishes a GitHub release when a version tag is
  pushed. Set `version` in `Cargo.toml`, commit, then `git tag v1.2.3 && git push
  origin v1.2.3`. It reruns the checks, refuses a tag that does not match
  `Cargo.toml`, and attaches binaries for Linux (x86_64 and arm64, static), macOS
  (Apple silicon and Intel) and Windows, plus `SHA256SUMS`. Running it by hand from
  the Actions tab builds the binaries without publishing.
- `svgdoc rules --explain ID` examples are executed by the tests: each failing
  example must raise its rule, each passing one must not, and each fix must clear
  its finding.

## Known limits

- Geometry findings on SVG files svgdoc did not render rest on heuristics; they are
  marked `approximate` and capped at warning.
- Editing a YAML spec keeps comments and layout for field changes, renames and
  additions. Removing a node, group or edge rewrites the spec in canonical form;
  the result says `"formatting": "rewritten"`.
- A drawing wider than `max_width` is first set more tightly; if it still does not
  fit, render fails with advice instead of producing an unreadable figure.
