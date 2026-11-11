# Everyday use: which command for which job

SVGDog has nine commands. Most days you will use three or four of them. This page tells you which one to pick, and what its exit code means.

## The loop

Most of the time you repeat these steps until the exit code is 0:

```bash
svgdog schema spec                        # learn the spec format (once)
svgdog render orders.yaml -o orders.svg   # draw it and check it
svgdog fix orders.svg --in-place          # apply the safe repairs
svgdog preview orders.svg -o orders.png   # look at it
```

## Which command for which job

| I want to… | Command | Read more |
|---|---|---|
| Make an SVG from my spec | `render` | [Writing a spec](writing-a-spec.md) |
| Check any SVG for mistakes | `validate` | [Checking and fixing SVGs](checking-and-fixing.md) |
| Repair the mistakes SVGDog can fix by itself | `fix` | [Checking and fixing SVGs](checking-and-fixing.md) |
| Change one thing (a colour, a label) without redoing everything | `edit` | [Checking and fixing SVGs](checking-and-fixing.md) |
| Find out what is inside an SVG and where it sits | `inspect` | [Checking and fixing SVGs](checking-and-fixing.md) |
| See the diagram as a picture | `preview` | [Looking at your diagram](looking-at-your-diagram.md) |
| See the exact format of a spec | `schema spec` | [Writing a spec](writing-a-spec.md) |
| List the checks, or read why one exists | `rules` | [Checking and fixing SVGs](checking-and-fixing.md) |
| Let an AI assistant use SVGDog | `mcp` | [Using SVGDog with an AI agent](using-with-an-ai-agent.md) |

## Exit codes

Every command ends with a number. Scripts and AI assistants read it; you can too (`echo $?` in most shells).

| Code | Meaning | What to do |
|---|---|---|
| 0 | All good | Nothing |
| 1 | The check found problems at or above the level that fails | Read the findings; try `fix` |
| 2 | The command was typed wrongly | Check the flags with `svgdog <command> --help` |
| 3 | Something is wrong with your input (a typo in the spec, a missing file) | Read the message; it usually suggests the fix |
| 4 | SVGDog could not write the output, for example the file already exists | Use `--force`, or choose another name |
| 5 | An internal error | Report it; see [FAQ and troubleshooting](faq.md) |

## Two habits that save time

- **Add `--format text`** when you read the output yourself. The default for non-terminal output is JSON, which is meant for programs.
- **Use `--dry-run`** with `edit` and `fix` the first time. It shows what would change and writes nothing.

> **Tip.** Every command accepts `--help`. For example `svgdog render --help` lists every option with a one-line explanation.
