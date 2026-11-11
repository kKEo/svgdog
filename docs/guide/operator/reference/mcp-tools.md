# MCP tools

`svgdog mcp --root DIR` serves the commands as Model Context Protocol tools over stdio, so an AI agent can render, check, fix and look at diagrams without a shell. The server adds no behaviour of its own: a tool call is turned into the command line it mirrors, run in process, and answered with the same JSON envelope the CLI prints. This page lists the transport, the protocol versions, root confinement and every tool with its parameters.

## Starting the server

```bash
svgdog mcp --root ./docs
```

Register it in an MCP client as a stdio server, for example:

```json
{ "mcpServers": { "svgdog": { "command": "svgdog", "args": ["mcp", "--root", "/path/to/docs"] } } }
```

- **Transport:** stdio, one JSON-RPC message per line, in both directions. The server runs until stdin closes.
- **Root:** required. The server changes its working directory to the root, so relative paths in tool calls are relative to it. A root that does not exist or is not a directory ends the start-up with a JSON error (`root-not-found`, exit 3).
- **Methods:** `initialize`, `server/discover`, `ping`, `tools/list`, `tools/call`. Notifications get no reply. Unknown methods answer `-32601`, bad JSON `-32700`.
- **Server instructions:** sent to the client at start-up. They describe the loop: `svgdog_schema` (name `spec`) once, write a spec, `svgdog_render`, read the findings, `svgdog_fix` or change the spec, repeat until no errors, `svgdog_preview` with `annotate` for a look.

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

`-` (stdin or stdout) is refused in every path argument, because stdin and stdout carry the protocol (`invalid-argument`). Globs given to `svgdog_validate` are expanded first, and every file they reach is checked. Config discovery also stops at the root.

## Tools

All eight tools return the CLI envelope as `structuredContent` and as JSON text in `content`. A failure sets `isError: true` and carries the error envelope. Unknown arguments are rejected (`unknown-field`, with a hint).

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
| `spec` | path | yes | Diagram spec file |
| `output` | path | yes | Where to write the SVG |
| `theme` | string | no | `docs-light` or `docs-dark` |
| `no_validate` | boolean | no | Skip the checks |
| `force` | boolean | no | Replace an existing output file |
| `text_as_paths` | boolean | no | Convert text to outlines |

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

Each tool's input schema sets `additionalProperties: false`, and its annotations say `readOnlyHint` (true for validate, inspect, schema and rules) and `openWorldHint: false`.

## Example call

Request (one line on stdin):

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"svgdog_render","arguments":{"spec":"orders.yaml","output":"orders.svg"}}}
```

Response (one line on stdout; `structuredContent` repeats the JSON in `content[0].text`):

```json
{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"text","text":"{\"tool\":\"svgdog\",\"version\":\"1.0.0\",\"command\":\"render\",\"ok\":true,\"summary\":{\"files\":1,\"errors\":0,\"warnings\":0,\"info\":0,\"fixable\":0,\"truncated\":false},\"findings\":[],\"outputs\":[{\"path\":\"orders.svg\",\"bytes\":3052}]}"}],"structuredContent":{"tool":"svgdog","version":"1.0.0","command":"render","ok":true,"summary":{"files":1,"errors":0,"warnings":0,"info":0,"fixable":0,"truncated":false},"findings":[],"outputs":[{"path":"orders.svg","bytes":3052}]},"isError":false}}
```

A path outside the root:

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"svgdog_render","arguments":{"spec":"../orders.yaml","output":"x.svg"}}}
```

answers with `isError: true` and this envelope:

```json
{"tool":"svgdog","version":"1.0.0","command":"render","ok":false,"error":{"code":"outside-root","message":"'../orders.yaml' (spec) is outside the root; the server only reads and writes inside its root directory.","path":"../orders.yaml"}}
```

An unknown tool name answers with the JSON-RPC error `-32602` and a did-you-mean hint. A panic inside a tool is caught and returned as an `internal-error` envelope, so the server stays up.
