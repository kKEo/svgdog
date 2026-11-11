# Monitoring and diagnostics

SVGDog has no running service to monitor, so this chapter covers what each run tells you instead: the exit code, the JSON result, the findings in it and the one line it writes to stderr. If you collect signals from SVGDog in a pipeline or behind an agent, these are all of them. There are no metrics endpoints, log files or health checks.

## The exit code

Every command ends with one of six exit codes (`src/envelope.rs:9-14`):

| Code | Name | Meaning | Typical cause |
| --- | --- | --- | --- |
| 0 | OK | Success | No finding reached the fail level |
| 1 | Findings | Findings at or above the fail level | A rule failed; see the findings |
| 2 | Usage | The command line was wrong | Unknown flag, missing argument, unknown rule id in `--rules` |
| 3 | Input | A file was missing, unreadable or invalid | Bad spec, bad config, `too-wide`, missing input |
| 4 | Output | The result could not be written | Output exists without `--force`, or a write failed |
| 5 | Internal | SVGDog hit a bug | A panic, caught and reported in the normal shape |

Exit `1` is not an error in the program; it means "the check found something". Alert on it differently from `2` to `5`, which mean the run itself did not work. `preview` is the exception: it exits `0` even when findings exist, because findings are information there, not failure.

A pipeline needs only the exit code to gate. The rest of this chapter is for explaining a failure.

## The JSON result

When stdout is not a terminal (or with `--format json`) each command prints exactly one JSON object followed by a newline. A command that finds rule violations looks like this:

```json
{
  "tool": "svgdog",
  "version": "1.0.0",
  "command": "validate",
  "ok": false,
  "summary": {"files": 1, "errors": 7, "warnings": 11, "info": 2, "fixable": 15, "truncated": false},
  "findings": [ ... ]
}
```

- `ok` is `false` when the run failed its gate. It always agrees with the exit code: `true` for `0`, `false` for anything else.
- `summary` counts what was found. `fixable` counts findings that carry a fix. `truncated` is `true` when the list was cut at `--max-findings` (default 50); the counts in `summary` still cover everything.
- `findings` is the list. Commands that write files add `outputs` (path and byte count); `edit` and `fix` add `changed`, `dry_run`, `diff` and `diff_truncated`.

A failure that stops the command before it can check anything has a different shape, with an `error` object:

```json
{"tool":"svgdog","version":"1.0.0","command":"render","ok":false,
 "error":{"code":"too-wide","message":"The drawing is 576 px wide even at the tightest spacing; max_width allows 300.",
          "path":"max_width","suggestion":"Try 'direction: down', shorter labels, or a larger max_width."}}
```

`code` is a stable machine-readable name, `path` points into the input where one applies, and `suggestion` is a hint when there is one. Match on `code`, not on `message`. The full lists are in [Envelope and exit codes](../reference/envelope-exit-codes.md); the JSON Schemas (`svgdog schema findings`) describe the exact shapes. Under MCP, the same envelope arrives as the tool result; a failing command is still a successful protocol exchange, so read `ok`.

## Reading a finding

Each entry in `findings` has these fields (`src/envelope.rs:44-58`):

| Field | Meaning |
| --- | --- |
| `rule` | The rule id, such as `low-contrast`. `svgdog rules --explain <id>` explains it |
| `severity` | `error`, `warning` or `info` |
| `file`, `line` | Where. `line` is a line in the SVG text |
| `target` | A selector for the element, such as `#a > text`, usable with `inspect` and `edit` |
| `bbox` | `[x, y, width, height]` in viewBox units, when the rule measured one |
| `message` | A sentence a person can act on |
| `measured` | The numbers behind the finding. Includes `"approximate": true` for geometry findings on files SVGDog did not render |
| `fix` | If present, `{ "safe": bool, "ops": [...] }`, edit operations that resolve it |

Findings are sorted by line, then rule id, so two runs on the same file produce the same list.

Useful one-liners, all run against SVGDog 1.0.0:

```bash
# counts by rule, most frequent first
svgdog --format json validate docs/img/broken.svg | jq -r '.findings[].rule' | sort | uniq -c | sort -rn

# is anything auto-fixable left?
svgdog --format json validate docs/img/*.svg | jq '.summary.fixable'

# show only findings that are exact (not heuristic)
svgdog --format json validate docs/img/*.svg | jq '.findings[] | select(.measured.approximate != true)'
```

## What goes to stderr

SVGDog writes to stderr only one line, and only when it read a config file:

```text
svgdog: config /path/to/svgdog.toml
```

It appears once per config file loaded. Use it to confirm which `svgdog.toml` governed a run, which is the first question when two machines disagree. `--quiet` removes it. Text-format errors (`error[code]: message`) also go to stderr; JSON errors go to stdout.

In MCP mode each tool call runs as if `--quiet` were given, so the config line is not printed, and stdout carries only protocol messages.

## Diagnosing a run

| Question | How to answer it |
| --- | --- |
| Why did the job fail? | Exit code, then `error.code` or the `findings` list |
| Which config was used? | The `svgdog: config ...` line, or re-run with `--no-config` and compare |
| Was a finding exact? | `measured.approximate` is absent or false |
| Is output stable between runs? | `cmp` two renders; see [Determinism](../how-it-works/determinism.md) |
| Which version ran? | `"version"` in any result, or `svgdog --version` |
| What would a fix change? | `svgdog fix <file> --dry-run` shows the diff and the findings that would remain, without writing |

For causes and remedies of specific codes, see [Troubleshooting](troubleshooting.md).
