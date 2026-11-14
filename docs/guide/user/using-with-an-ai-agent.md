# Using SVGDog with an AI agent

SVGDog was designed so that an AI assistant can make and check diagrams on its own: the format is small, every message says how to fix the problem, and every result has an exit code. This page explains how to connect an assistant and what to ask. It takes about ten minutes.

## Two ways to connect

**1. Let the assistant run commands.** If your assistant can run terminal commands (as coding assistants can), tell it that `svgdog` is available. It reads the format with `svgdog schema spec`, writes a spec, and repeats `svgdog render` until the exit code is 0. This needs no setup beyond having SVGDog installed.

**2. Connect it as a tool.** SVGDog can talk the Model Context Protocol (MCP), the standard way for assistants to call tools. You start it with:

```bash
svgdog mcp --root-path ./docs
```

`--root-path` is the one folder the assistant may read and write. Every path it uses is relative to that folder, and anything outside is refused. Most assistants that support MCP ask for a command and arguments to start the server; give them `svgdog` and `mcp --root-path ./docs`. Check your assistant's own documentation for where to enter this.

The server offers fourteen tools, named after the commands you already know:

| Tool | Same as |
|---|---|
| `svgdog_render` | `svgdog render` |
| `svgdog_validate` | `svgdog validate` |
| `svgdog_inspect` | `svgdog inspect` |
| `svgdog_preview` | `svgdog preview` |
| `svgdog_edit` | `svgdog edit` |
| `svgdog_fix` | `svgdog fix` |
| `svgdog_schema` | `svgdog schema` |
| `svgdog_rules` | `svgdog rules` |
| `svgdog_examples` | `svgdog examples` |
| `svgdog_docs` | `svgdog docs` |
| `svgdog_describe` | `svgdog describe` |
| `svgdog_theme_list`, `svgdog_theme_init`, `svgdog_theme_check` | `svgdog theme list`, `init` and `check` |

The assistant does not need files to draw. `svgdog_render` accepts the spec as text and, with `preview: true`, answers with the checked drawing and a picture of it in one step. Ask for files only when you want to keep the result.

`svgdog_preview` can return the picture straight to the assistant, so that it can look at its own work, when you do not give an output file. The details of each tool are in the [Operator guide](../operator/).

## What to ask for

Good requests name the diagram, the kind and where to save it:

> Draw the checkout flow as a top-to-bottom flow diagram with SVGDog. Save the spec as `docs/checkout.yaml` and the SVG as `docs/checkout.svg`. Keep going until `svgdog render` exits with 0, then look at a preview and tell me if anything is hard to read.

Ask the assistant to **look at the picture**, not only to check the exit code. A diagram can pass every rule and still be confusing.

## Why this works well

- **Short feedback loop.** Errors in the spec say what is wrong and suggest a fix (`Did you mean 'label'?`).
- **Safe repairs.** `fix` applies only repairs that do not change the look, unless you allow `--unsafe`.
- **Same result every time.** A spec that works today will draw the same picture tomorrow.
- **Contained.** The assistant can only touch files under the root you gave it, and SVGDog never uses the network.

> **Tip.** Keep the spec files in your repository next to the SVGs. The assistant (or you) can then change a spec and redraw the picture, which is much easier than editing SVG by hand.
