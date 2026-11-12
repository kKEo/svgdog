# Welcome to the show room

SVGDog is one small program with a dozen commands. This show room does not list them. It shows **five complete workflows**, ordered from the simplest to the most advanced, so you can see the way SVGDog is meant to be used and stop at the step that fits your work.

The `svgdog` commands in these pages were run against SVGDog 1.0.4 and their output is shown as printed, shortened in places. The pictures were drawn by SVGDog from the specs shown next to them. The setup snippets (installing, the CI job, registering the server with a client) are templates to adapt.

## The five workflows

| # | Workflow | You will | Commands | Time |
|---|---|---|---|---|
| 1 | [Your first diagram](1-first-diagram.md) | Turn a short text spec into an SVG and look at it | `render`, `preview` | 5 min |
| 2 | [Check an SVG you did not make](2-check-any-svg.md) | Find, fix and re-check problems in any SVG | `validate`, `fix`, `preview --annotate` | 10 min |
| 3 | [Diagrams that live in your pages](3-diagrams-in-pages.md) | Keep the spec in the Markdown page and let SVGDog draw it | `docs`, `docs --check` | 10 min |
| 4 | [Explain it, then change it safely](4-explain-and-change.md) | Tell a story with steps, write alt text, edit without moving boxes | `describe`, `render --previous`, `edit` | 15 min |
| 5 | [Let an agent and CI do the work](5-agents-and-ci.md) | Set house rules, gate pull requests, hand the tools to an AI agent | `svgdog.toml`, `validate`, `mcp` | 20 min |

Each workflow builds on the one before it, but none needs the earlier ones to be finished. The rule of thumb:

- **Drawing diagrams by hand in a text file?** Start at 1.
- **Holding SVGs from other tools?** Jump to 2.
- **Writing documentation?** Go to 3.
- **Maintaining diagrams over months?** Read 4.
- **Running a team, a pipeline or an AI assistant?** Read 5.

## What you need

Only the `svgdog` program. Install it with `cargo install svgdog`, `cargo binstall svgdog` or `brew install kKEo/tap/svgdog`, or download a release from the [releases page](https://github.com/kKEo/svgdog/releases). There is no account, network access or browser involved. Check it works:

```bash
svgdog --version
```

> **Tip.** SVGDog prints JSON when its output is not a terminal and readable text when it is. The pages here add `--format text` so you see the same thing in a script as on screen. In a pipeline, leave it off and read the JSON.

## Going deeper

This book is a tour. The [User guide](https://kkeo.github.io/svgdog/user/) explains every feature in plain language, the [Operator guide](https://kkeo.github.io/svgdog/operator/) is the full reference and has the CI recipes, and the [Tester guide](https://kkeo.github.io/svgdog/tester/) is for people who change SVGDog itself.
