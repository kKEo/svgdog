# Part IV: Operations

This part is about the day-two work of running svgdog: wiring it into pipelines, reading what it reports, cutting releases and fixing failures. It follows the shape of the other operator guides in this documentation family, with one deliberate deviation that you should know about before you look for a missing chapter.

> **Deviation from the usual template.** Operator guides normally have a chapter on monitoring a running service: metrics, logs and alerts. SVGDog has no running service. It is a command-line tool that starts, does one job and exits, and the MCP server is a child process of the client that launched it. There is nothing to scrape and nothing to page on. The monitoring slot is therefore filled by [Monitoring and diagnostics](monitoring.md), which covers what a run can tell you: findings, exit codes and JSON output. If you operate SVGDog inside a larger system, the signals to watch are the exit code of each invocation and the `summary` block of its JSON result.

| Chapter | Read it when you need to… |
| --- | --- |
| [CI recipes](ci-recipes.md) | Fail a pull request on diagram problems, keep committed SVGs fresh, or lint a docs folder. |
| [Monitoring and diagnostics](monitoring.md) | Read an exit code, parse the JSON result, or see which config file was used. |
| [Release process](release-process.md) | Publish a version with binaries. |
| [Troubleshooting](troubleshooting.md) | Look up an error code or a puzzling result. |
| [Known limits](known-limits.md) | Check whether something is unsupported before you build on it. |

Two facts shape all four chapters: every command prints exactly one JSON object when it is not writing to a terminal, and the exit code is a stable contract (`0` success, `1` findings at or above the fail level, `2` usage error, `3` input error, `4` output error, `5` internal error). The details are in [Envelope and exit codes](../reference/envelope-exit-codes.md).
