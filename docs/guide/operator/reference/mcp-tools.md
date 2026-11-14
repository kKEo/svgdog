# MCP tools

`svgdog mcp --root-path DIR` serves the commands as Model Context Protocol tools over stdio, so an AI agent can render, check, fix and look at diagrams without a shell. The server adds no behaviour of its own: a tool call is turned into the command line it mirrors, run in process, and answered with the same JSON envelope the CLI prints. This page lists the transport, the protocol versions, root confinement and every tool with its parameters.

## Starting the server

```bash
svgdog mcp --root-path ./docs
```

Register it in an MCP client as a stdio server, for example:

```json
{ "mcpServers": { "svgdog": { "command": "svgdog", "args": ["mcp", "--root-path", "/path/to/docs"] } } }
```

- **Transport:** stdio, one JSON-RPC message per line, in both directions. The server runs until stdin closes.
- **Root:** required. The server changes its working directory to the root, so relative paths in tool calls are relative to it. A root that does not exist or is not a directory ends the start-up with a JSON error (`root-not-found`, exit 3).
- **Methods:** `initialize`, `server/discover`, `ping`, `tools/list`, `tools/call`. Notifications get no reply. Unknown methods answer `-32601`, bad JSON `-32700`.
- **Server instructions:** sent to the client at start-up. They describe the loop: `svgdog_examples` for a spec to start from (`svgdog_schema`, name `spec`, has every field), `svgdog_render` with the spec as text and `preview: true`, read the findings and look at the image, `svgdog_fix` or change the spec, repeat until no errors, `svgdog_preview` with `annotate` for a look.

## Protocol versions

| Style | Versions | How the client talks |
|---|---|---|
| With the `initialize` handshake | 2025-11-25, 2025-06-18, 2025-03-26, 2024-11-05 | `initialize` first. If the client asks for a version the server does not know, the server answers with 2025-11-25 |
| Per request | 2026-07-28 | Each request carries `_meta` with `io.modelcontextprotocol/protocolVersion` and `io.modelcontextprotocol/clientCapabilities`; the client may call `server/discover` first (it returns `supportedVersions`, `capabilities` and `instructions`) |

A request naming an unsupported per-request version gets error `-32022` with the supported versions. A per-request call without client capabilities gets `-32602`.

## Root confinement

Every path argument is checked before anything runs. A path is refused (`outside-root`, exit class 3) if, after resolving it:

- it lies outside the root, including through symlinks, or
- it climbs out with `..`.

`-` (stdin or stdout) is refused in every path argument, because stdin and stdout carry the protocol (`invalid-argument`). Globs given to `svgdog_validate` are expanded first, and every file they reach is checked. Config discovery also stops at the root. A theme folder is checked the same way, whether the `theme` argument, a spec or `svgdog.toml` names it.

## Tools

All fourteen tools return the CLI envelope as `structuredContent` and as JSON text in `content`. A failure sets `isError: true` and carries the error envelope. Unknown arguments are rejected (`unknown-field`, with a hint).

Tools marked † also accept the three config arguments:

| Argument | Type | Meaning |
|---|---|---|
| `profile` | string | `minimal`, `docs` or `strict` |
| `config` | path | A config file to use instead of the nearest `svgdog.toml` |
| `no_config` | boolean | Ignore any config file |

### svgdog_render †

Compile a spec (YAML or JSON) to SVG and validate it. Mirrors [`render`](commands.md#render). Not read-only.

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `spec` | string | one of the two | The spec itself, as YAML or JSON text |
| `file` | path | one of the two | Diagram spec file |
| `output` | path | no | Where to write the SVG; without it nothing is written and the SVG comes back in the envelope's `svg` field |
| `preview` | boolean | no | Also return a PNG of the result as image content, 1200 px wide on a white page; the envelope gains `image` |
| `theme` | string | no | A built-in theme (`docs`, `blueprint`, `paper`, `bold`, `cards`; add `-dark` or `-auto`) or a theme folder inside the root (add `#dark` or `#auto`) |
| `frames` | boolean | no | One picture per step of the spec's `steps`; needs `output` |
| `interactive` | string | no | `css` or `html`; see [Interactive versions](commands.md#interactive-versions) |
| `embed_spec` | boolean | no | Store the spec inside the SVG |
| `check` | boolean | no | Write nothing; report whether `output` is what the spec renders now |
| `previous` | path | no | The SVG rendered from the earlier spec: boxes in both stay where they were. May be the same file as `output`. See [Keeping the layout](commands.md#keeping-the-layout) |
| `no_validate` | boolean | no | Skip the checks |
| `force` | boolean | no | Replace an existing output file; with `previous`, lay out afresh when the layout cannot be kept |
| `text_as_paths` | boolean | no | Convert text to outlines |

With `spec` and no `output`, one call takes a diagram from text to a checked drawing and a picture, with no files involved. Giving both `spec` and `file` is `argument-conflict`; giving neither is `missing-argument`. A `spec` value that is a single line ending in `.yaml`, `.yml` or `.json` is refused with a hint to use `file`. A spec given as text has no location, so config discovery starts at the root.

### svgdog_validate †

Run the rules on SVG files. Read-only. Mirrors [`validate`](commands.md#validate).

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `files` | list of paths | yes | SVG files or globs |
| `rules` | list of strings | no | Run only these rule ids |
| `skip` | list of strings | no | Skip these rule ids |
| `fail_on` | string | no | `error` or `warning`: lowest severity that makes `ok` false |
| `max_findings` | integer | no | Cap on listed findings; default 50 |

### svgdog_inspect

Describe an SVG's elements as data with bounding boxes. Read-only; takes no config arguments. Mirrors [`inspect`](commands.md#inspect).

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `file` | path | yes | SVG file |
| `select` | string | no | `#id`, `tag` or `tag[attr="value"]` |
| `depth` | integer | no | Deepest nesting level to return |
| `fields` | list of strings | no | Any of `id`, `tag`, `bbox`, `text`, `fill`, `stroke`, `refs` |

### svgdog_preview †

Rasterize an SVG to PNG. Mirrors [`preview`](commands.md#preview). Not read-only.

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `file` | path | yes | SVG file |
| `output` | path | no | Where to write the PNG. Omit it and the PNG comes back as image content (`image/png`, base64) before the JSON text |
| `width` | integer | no | Pixels; default 1200, at most 2000 |
| `background` | string | no | `light` or `dark` |
| `annotate` | boolean | no | Outline each finding's box with its rule id |
| `crop` | string | no | Selector; show only matching elements and surroundings |
| `step` | integer | no | Show the drawing with this step chosen, the rest faded |
| `force` | boolean | no | Replace an existing output file |

### svgdog_edit †

Apply targeted changes to an SVG or a spec. Mirrors [`edit`](commands.md#edit). Not read-only. Give `set` or `ops` (not both), and one of `dry_run`, `output` or `in_place`.

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `file` | path | yes | SVG or spec file |
| `set` | list of strings | no | Changes of the form `SELECTOR.attr=value` |
| `ops` | JSON | no | Operations inline: an array, or a finding's `fix` object (see `svgdog_schema` with `ops`) |
| `dry_run` | boolean | no | Report the diff and findings; write nothing |
| `output` | path | no | Write the result to a new file |
| `in_place` | boolean | no | Change the input file |

Unlike the CLI's `--ops FILE`, `ops` here is the JSON itself, so a client can pass a finding's `fix` straight back.

### svgdog_fix †

Apply the autofixes that findings carry. Mirrors [`fix`](commands.md#fix). Not read-only. Give one of `dry_run`, `output` or `in_place`.

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `file` | path | yes | SVG file |
| `rules` | list of strings | no | Fix only these rule ids |
| `unsafe` | boolean | no | Also apply fixes that move or recolour content |
| `dry_run` | boolean | no | Report the diff and findings; write nothing |
| `output` | path | no | Write the result to a new file |
| `in_place` | boolean | no | Change the input file |

### svgdog_schema

Print the JSON Schema of a format. Read-only. Mirrors [`schema`](commands.md#schema).

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | yes | `spec`, `config`, `findings`, `scene` or `ops` |

### svgdog_rules

List every rule, or explain one. Read-only. Mirrors [`rules`](commands.md#rules).

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `explain` | string | no | Rule id to explain |

### svgdog_docs †

Render the diagrams inside Markdown pages. Mirrors [`docs`](commands.md#docs). Not read-only.

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `paths` | list of paths | yes | Markdown files, or folders to search |
| `check` | boolean | no | Write nothing; report what is out of date |
| `assets` | string | no | Folder for the pictures, next to each page; default `diagrams`. A value with `..` is refused, so pictures stay below their page |
| `theme` | string | no | Theme for every diagram: a built-in name or a theme folder inside the root |
| `force` | boolean | no | Lay a diagram out afresh when its boxes cannot stay where the picture on disk has them |
| `interactive` | string | no | `css` or `html` |

### svgdog_describe

Say in words what a diagram shows, and return its spec. Read-only. Mirrors [`describe`](commands.md#describe).

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `file` | path | yes | Spec, or an SVG rendered with `embed_spec` |
| `short` | boolean | no | One or two sentences |

### svgdog_theme_list

List the built-in themes with their colours, fonts, sizes and line styles. Read-only. Takes no arguments. Mirrors [`theme list`](commands.md#theme).

### svgdog_theme_init

Write a `theme.toml`, copied from a built-in theme, into a folder. Mirrors [`theme init`](commands.md#theme).

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `dir` | path | yes | Folder for the theme; created when missing |
| `from` | string | no | Built-in theme to copy; default `docs` |
| `force` | boolean | no | Replace a `theme.toml` that is already there |

### svgdog_theme_check

Check a theme folder: the file, the contrast of its colours, and the bundled examples drawn with it. Read-only. Mirrors [`theme check`](commands.md#theme). Also takes the config arguments.

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `dir` | path | yes | Folder that holds the `theme.toml` |

### svgdog_examples

List the bundled example specs, or return one in full. Read-only. Mirrors [`examples`](commands.md#examples).

| Argument | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | no | Example to return; omit to list them |

Each tool's input schema sets `additionalProperties: false`, and its annotations say `readOnlyHint` (true for validate, inspect, schema, rules, describe, examples, theme_list and theme_check) and `openWorldHint: false`.

## Example call

Request (one line on stdin):

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"svgdog_render","arguments":{"file":"orders.yaml","output":"orders.svg"}}}
```

Response (one line on stdout; `structuredContent` repeats the JSON in `content[0].text`):

```json
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"tool\":\"svgdog\",\"version\":\"1.0.0\",\"command\":\"render\",\"ok\":true,\"summary\":{\"files\":1,\"errors\":0,\"warnings\":0,\"info\":0,\"fixable\":0,\"truncated\":false},\"findings\":[],\"outputs\":[{\"path\":\"orders.svg\",\"bytes\":3052}]}"}],"structuredContent":{"tool":"svgdog","version":"1.0.0","command":"render","ok":true,"summary":{"files":1,"errors":0,"warnings":0,"info":0,"fixable":0,"truncated":false},"findings":[],"outputs":[{"path":"orders.svg","bytes":3052}]},"isError":false}}
```

A path outside the root:

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"svgdog_render","arguments":{"file":"../orders.yaml","output":"x.svg"}}}
```

answers with `isError: true` and this envelope:

```json
{"tool":"svgdog","version":"1.0.0","command":"render","ok":false,"error":{"code":"outside-root","message":"'../orders.yaml' (file) is outside the root; the server only reads and writes inside its root directory.","path":"../orders.yaml"}}
```

An unknown tool name answers with the JSON-RPC error `-32602` and a did-you-mean hint. A panic inside a tool is caught and returned as an `internal-error` envelope, so the server stays up.
