# Troubleshooting

This page lists the failures an operator meets most often, with what each means and what to do. Messages below are copied from real runs of SVGDog 1.0.0. Find yours by the `error.code` in the JSON result (or the `error[code]` prefix in text output), or by the symptom.

Start with the exit code: `1` means "findings" and is dealt with in the second half; `2` to `5` mean the run itself failed and are in the first.

## The run failed (exit code 2 to 5)

### Spec errors (exit 3)

| Code | Example message | Cause and fix |
| --- | --- | --- |
| `malformed-spec` | `bad.yaml is not valid YAML: did not find expected ',' or ']' at line 4 column 6...` | The file does not parse. Fix the syntax at the line given. A file whose first character is `{` is read as JSON |
| `unknown-field` | `Unknown field 'nodez' in the spec root.` Did you mean 'nodes'? | A key is not allowed there. Use the suggestion; see the [spec reference](../reference/spec.md) |
| `missing-field` | `Missing required field 'version'.` | Add the field. `version`, `diagram`, `title` and `nodes` are required |
| `invalid-value` | `Spec version 2 is not supported; this build reads version 1.` | A value is out of range or not allowed. The message names the field and the limit |
| `invalid-spec` | `... is 130 characters long; the limit is 120.` | A value has the wrong type, is empty or is too long |
| `duplicate-id` | `Id 'a' is used twice; ids are unique across nodes, groups and edges.` | Give each node, group and edge a different id |
| `unknown-reference` | `edges[0].to names 'bb', which is not defined.` Did you mean 'a'? | An edge, group or note points at an id that does not exist |
| `at-outside-free` | `Node 'a' sets 'at', which is allowed only in a free diagram.` | Remove `at`, or set `diagram: free` |
| `group-cycle`, `group-overlap` | | Groups may nest but not loop, and may not share members unless one contains the other |
| `unknown-theme` | `Unknown theme 'docs-lite'.` Did you mean 'docs-light'? | Use `docs-light` or `docs-dark` |
| `too-wide` | `The drawing is 576 px wide even at the tightest spacing; max_width allows 300.` | See below |
| `file-not-found` | `File not found: nothere.yaml.` | Check the path and working directory |

The `path` field in the error points at the offending part of the input, for example `edges[0].to`.

### `too-wide`

Render tried all three layout tightness levels and the drawing is still wider than `max_width`. Nothing was written. In order of preference:

1. Add `direction: down` to a flow or layers diagram, so the drawing grows downwards instead of sideways.
2. Shorten labels, or put fewer boxes side by side.
3. Raise `max_width` (up to 2000; default 960) if the diagram will be shown wide.

Details are in [Layout and auto-fit](../how-it-works/layout.md#auto-fit).

### Config errors (exit 3)

A config file with an unknown key, unknown rule, unknown option or bad value is rejected, so a typo cannot silently disable a check:

```text
{"error":{"code":"unknown-field","message":"proj/bad.toml: unknown key 'bogus'.","path":"bogus"}}
```

Other codes: `unknown-rule` (with a did-you-mean), `unknown-profile`, `invalid-config`, `config-not-readable`. The message names the file. See [Configuration](../reference/configuration.md).

### Usage errors (exit 2)

| Code | Cause |
| --- | --- |
| `unknown-command` | A subcommand that does not exist. The JSON `command` field reads `svgdog` because no command was identified |
| `unknown-flag` | A flag that does not exist, usually with a suggestion |
| `missing-argument` | A required argument is missing. `edit` and `fix` need one of `-o`, `--in-place` or `--dry-run` |
| `argument-conflict` | Flags that cannot go together, such as `-o` with `--in-place` |
| `unknown-rule` | A rule id in `--rules` or `--skip` does not exist. Run `svgdog rules` |

### Output errors (exit 4)

`output-exists`: `a.svg already exists; pass --force to replace it.` The output path is taken. Add `--force`, choose a new path, or for `edit` and `fix` use `--in-place`. `write-failed` means the directory is missing or not writable.

### MCP errors

| Code | Cause |
| --- | --- |
| `outside-root` | A path climbs out of `--root` with `..`, is elsewhere, or goes through a symlink out. Use a path inside the root |
| `root-not-found` | `--root` does not exist or is not a directory. Exit 3 at start-up |
| `unknown-field` | A tool argument name is wrong; see [MCP tools](../reference/mcp-tools.md) |

### Internal error (exit 5)

`svgdog hit a bug (...)`. A panic was caught. Keep the input file and the exact command, and report it with the SVGDog version. Re-running will give the same result because output is deterministic.

## The run completed but reported findings (exit code 1)

| Symptom | Explanation and fix |
| --- | --- |
| `render` exits 1 for a spec you just wrote | The drawing has an error-level finding. Read the findings; many carry a `fix`. Run `svgdog fix out.svg --in-place`, or change the spec and render again |
| A file shows `desc-missing` | The SVG was not rendered by this version of SVGDog, or its `<desc>` was removed. A rendered file always carries one: the spec's `description`, or a generated sentence. Run `svgdog describe SPEC --short` to see that sentence |
| `fix` finishes but findings remain | `fix` applies only safe fixes without `--unsafe`, and many findings have no fix. `fix` exits 1 if findings remain. Read `fixable` in the summary |
| A finding has a fix but `fix` does not apply it | The fix is marked `unsafe` (it moves or recolours content). Pass `--unsafe` after reading what it does with `--dry-run` |
| A geometry finding on a hand-drawn SVG is only a warning | Geometry findings on files SVGDog did not render are heuristic: they carry `"approximate": true` and are capped at warning. See [The rule engine](../how-it-works/rules-engine.md#findings-that-cannot-be-exact) |
| Two machines report different findings | Different config. Compare the `svgdog: config ...` stderr line, or run both with `--no-config`. Then compare `svgdog --version` |
| A rule you switched off still fires | The config was not found (it is searched from the input file's folder upward, and under MCP only up to `--root`), or a later `[[overrides]]` block turned it back on, or `--rules` re-enabled it |
| `invalid-suppression` | A `svgdog-disable` comment has no reason, names an unknown rule, or is not directly before an element. The format is `<!-- svgdog-disable rule-id: reason -->` |
| Only 50 findings listed | The default cap. `summary.truncated` is `true`. Raise it with `--max-findings` |
| `preview` exits 0 although findings exist | By design: for `preview` findings are information. Use `validate` to gate |

## Still stuck

Reduce the problem: render the smallest spec that fails, run with `--no-config`, and read `svgdog rules --explain <rule>` for any rule involved. Each explanation has a failing and a passing example you can compare with your file.
