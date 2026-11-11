# SVGDog operator guide

This book is for the people who install, configure, run and look after svgdog: platform and documentation engineers who put it in a build pipeline, and developers who need to know what it does inside. It assumes you are comfortable on a command line, can read a YAML file and a TOML file, and know what an exit code is. It does not assume you know Rust.

SVGDog is one binary. It has no daemon, no database and no network access at run time, so "operating" it means three things: getting the right build onto the right machines, setting the rules it applies, and reading what it reports when it runs in CI or behind an AI agent. The book is organised around those jobs.

## Reading map

| Part | Read it when you need to… |
| --- | --- |
| [Part I: How it works](how-it-works/README.md) | Understand what happens between a spec file and an SVG, and why the same input always gives the same bytes. |
| [Part II: Install and deploy](install/README.md) | Build SVGDog, fetch release binaries, or hook the MCP server into an agent client. |
| [Part III: Reference](reference/README.md) | Look up a flag, a spec key, a rule, a config option, an exit code or an MCP tool. |
| [Part IV: Operations](operations/README.md) | Run SVGDog in CI, read its output, cut a release or work out why a run failed. |
| [Appendix](appendix/README.md) | Find a word you do not know. |

Quick routes for common needs:

| If you want to… | Read |
| --- | --- |
| Add a diagram check to a pull-request pipeline | [CI recipes](operations/ci-recipes.md) |
| Know what the exit code means | [Envelope and exit codes](reference/envelope-exit-codes.md) |
| Switch a rule off for one project | [Configuration](reference/configuration.md) |
| Fix "the drawing is N px wide" | [Layout and auto-fit](how-it-works/layout.md) |
| Let an agent call SVGDog | [Setting up the MCP server](install/mcp-setup.md) |
| See what is not supported | [Known limits](operations/known-limits.md) |

## Conventions

- Commands are shown in `bash` fences. A line starting with `#` inside a fence is a comment.
- `svgdog` stands for the binary on your `PATH`. If you built from source and did not install it, read it as `target/release/svgdog`.
- Source paths such as `src/render/layout.rs` are relative to the repository root. Line numbers are given only where the code is unlikely to move; check the file if they look off.
- Pixel values are SVG user units, which equal pixels at the diagram's own scale.
- Callouts: **Note** is useful context, **Warning** is something that costs you time or data if ignored.
- Every example in this book was run against SVGDog 1.0.0.

> **Looking for something else?** If you only want to draw diagrams, start with the [user guide](../user/). If you want to know how changes to SVGDog are tested before they merge, read the [tester guide](../tester/).
