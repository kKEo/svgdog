# Setting up the MCP server

`svgdog mcp` serves SVGDog's commands as tools to an AI agent over the Model Context Protocol. This page takes about ten minutes. It covers starting the server, how it confines file access, how to register it with a client, and how to test it by hand without a client.

## What it is

The server is the same binary in a different mode. It adds no behaviour: each tool call is turned into the command line it mirrors, run in-process, and answered with the same JSON envelope the command would print (`src/mcp.rs`). The eight tools and their parameters are listed in [MCP tools](../reference/mcp-tools.md).

- **Transport:** stdio. The client starts `svgdog mcp` as a child process and exchanges one JSON-RPC message per line on stdin and stdout. There is no port to open.
- **Protocol versions:** the `initialize` handshake for `2025-11-25`, `2025-06-18`, `2025-03-26` and `2024-11-05`, and the per-request style of `2026-07-28` (clients may call `server/discover` first). An unknown version in the modern style gets error `-32022` with the supported list.
- **Methods:** `initialize`, `server/discover`, `ping`, `tools/list`, `tools/call`. Notifications get no reply.

## Start it

```bash
svgdog mcp --root-path ./docs
```

`--root-path` is required. The server changes its working directory to the root, so every path a tool receives is relative to it.

## File confinement

Every path argument is checked before the command runs (`confine`, `src/mcp.rs:205-231`). A path is refused with error code `outside-root` (exit class 3 in the envelope) if it climbs out with `..`, is absolute and elsewhere, or resolves through a symlink to somewhere outside the root. `-` (stdin or stdout) is refused because the protocol uses them. Config discovery also stops at the root, so a `svgdog.toml` above it is never read.

Verified: a render of `../orders.yaml` returns

```json
{"ok":false,"error":{"code":"outside-root","message":"'../orders.yaml' (spec) is outside the root; the server only reads and writes inside its root directory.","path":"../orders.yaml"}}
```

> **Warning.** The root is the only access control. Everything inside it can be read, and `render`, `edit`, `fix` and `preview` can write inside it. Point `--root-path` at the documentation folder, not at a home directory or a repository root that holds secrets. A failed start (root missing or not a directory) exits with code 3 and error `root-not-found`.

## Register it with a client

Clients differ in where they keep the configuration, but all stdio clients need the same three facts: a command, its arguments and a name. For example, with Claude Code:

```bash
claude mcp add svgdog -- svgdog mcp --root-path ./docs
```

Clients that use a JSON configuration usually take an entry of this shape (check your client's documentation for the file name and exact keys):

```json
{
  "mcpServers": {
    "svgdog": {
      "command": "svgdog",
      "args": ["mcp", "--root-path", "/absolute/path/to/docs"]
    }
  }
}
```

Use an absolute path to the binary if the client does not inherit your `PATH`, and an absolute `--root-path`, since the client's working directory may not be your repository.

> **Note.** Neither registration form has been tested against a real client; the server has been tested by the repository's own protocol tests (`tests/mcp.rs`) and by the manual exchange below. The README records that a real MCP client has not been run yet.

## Test it without a client

Feed the server messages on stdin and read replies on stdout. This is a complete session, run against SVGDog 1.0.0 with `orders.yaml` in the root:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"svgdog_render","arguments":{"spec":"orders.yaml","output":"out.svg"}}}' \
  | svgdog mcp --root-path ./docs
```

You get two lines back (none for the notification). The first echoes `"protocolVersion":"2025-06-18"` with the server's `instructions`. The second is the tool result: the envelope appears both as text content and as `structuredContent`, with `"ok":true` and `"outputs":[{"path":"out.svg","bytes":3052}]`, and `out.svg` now exists in the root. For the modern style, `{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{}}` returns `"supportedVersions":["2026-07-28"]`.

## Behaviour to know about

- **A failing command is not a protocol error.** A tool call that hits validation errors or a bad path still returns a normal JSON-RPC result whose envelope has `"ok": false`; read the envelope, not the transport.
- **Tool arguments are strict.** An unknown argument name is rejected with `unknown-field` and a suggestion.
- **`svgdog_preview` without `output`** returns the PNG as base64 image content instead of writing a file.
- **Config is per call.** Each tool that takes one accepts `profile`, `config` and `no_config`, like the global flags.

## Where to go next

- Look up each tool's parameters in [MCP tools](../reference/mcp-tools.md).
- For output you will see from agents, see [Monitoring and diagnostics](../operations/monitoring.md).
