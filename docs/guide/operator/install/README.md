# Part II: Install and deploy

SVGDog is one self-contained executable. There is nothing to configure on a server, no service to start and no runtime to install: installing it means putting one file on the machines that need it. This part covers the two ways to get that file and the one piece of wiring that needs care, the MCP server.

| Chapter | Read it when you need to… |
| --- | --- |
| [Build from source](build-from-source.md) | Get a binary today. This is the only install path that works right now. |
| [Release binaries](release-binaries.md) | Use prebuilt archives, once the first `v*` release exists. |
| [Setting up the MCP server](mcp-setup.md) | Let an AI agent client call SVGDog as a set of tools. |

## What you are deploying

- **A binary of about 9 MB** (the 1.0.0 release build on macOS arm64 is 9,357,216 bytes). The two fonts are inside it.
- **No network use.** SVGDog never opens a connection, so it runs in an air-gapped build.
- **No required files.** An optional `svgdog.toml` next to your diagrams changes which rules apply; see [Configuration](../reference/configuration.md).
- **Five supported targets** in the release workflow: Linux x86_64 and arm64 (static), macOS Apple silicon and Intel, and Windows x86_64. CI runs the test suite on Linux, macOS and Windows.

## Choosing a path

| Situation | Use |
| --- | --- |
| Developer machine, or a CI image that already has Rust | [Build from source](build-from-source.md), or `cargo install --path .` |
| CI image without Rust, after a release exists | [Release binaries](release-binaries.md) |
| An agent client such as a coding assistant | Either of the above, then [MCP setup](mcp-setup.md) |

> **Note.** There is no crates.io package, Homebrew formula, npm wrapper or container image. `Cargo.toml` has no `license`, `description` or `repository` field yet, which a crates.io publish would require.
