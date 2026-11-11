# Part III: Reference

This part is the look-up half of the operator book: every command and flag, every key a spec accepts, every setting in `svgdog.toml`, the rule catalogue, the output contract, the JSON Schemas and the MCP tools. Read it with the command open next to you; each page states defaults and limits as the binary enforces them.

| Chapter | What it holds |
|---|---|
| [Commands](commands.md) | The nine subcommands, their flags, defaults and examples |
| [Spec reference](spec.md) | Every key of the YAML/JSON diagram spec, its type and limit, and the checks that reject a spec |
| [Configuration](configuration.md) | `svgdog.toml`: discovery, profiles, rules, overrides, and the `svgdog-disable` comment |
| [Rules catalogue](rules.md) | All 38 rules with severity, autofix, thresholds and examples (generated) |
| [Envelope and exit codes](envelope-exit-codes.md) | The JSON result shape, exit codes 0 to 5 and the error codes |
| [JSON Schemas](schemas.md) | The five embedded schemas and how to validate against them |
| [MCP tools](mcp-tools.md) | The eight tools, their parameters, the protocol versions and root confinement |

Conventions: `svgdog` is the binary on your `PATH` (or `target/release/svgdog`); `-` as a file name means stdin or stdout; a flag shown as `--x N` takes a value.
