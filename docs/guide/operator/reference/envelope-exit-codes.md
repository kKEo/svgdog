# Envelope and exit codes

Every command answers with one JSON object, the envelope, and an exit code. With `--format json` (the default when stdout is not a terminal) the envelope is the only thing on stdout, so a script or an agent can parse it without scraping text. This page defines the shape, the exit codes and the error codes.

## Envelope

Every envelope starts with the same four fields.

| Field | Type | Meaning |
|---|---|---|
| `tool` | string | Always `svgdog` |
| `version` | string | The SVGDog version, for example `1.0.0` |
| `command` | string | The command that ran (`svgdog` if the command could not be parsed) |
| `ok` | boolean | `true` when the command succeeded and no finding reached the fail level |

A successful `render`, `validate`, `edit`, `fix` or `preview` adds:

| Field | Meaning |
|---|---|
| `summary` | `files`, `errors`, `warnings`, `info`, `fixable` (counts over all findings) and `truncated` (more findings than `--max-findings`; default cap 50) |
| `findings` | Findings, sorted by file, line and rule, capped at the limit |
| `outputs` | Files written: `[{"path": "...", "bytes": N}]`, empty if none |

Some commands add fields:

| Field | Commands | Meaning |
|---|---|---|
| `validated` | `render`, `preview` | `false` when checks were skipped (`--no-validate`, or `preview` without `--annotate`) |
| `changed`, `dry_run`, `diff`, `diff_truncated` | `edit`, `fix` | Whether text changed, whether nothing was written, the unified diff |
| `operations` | `edit` | `{"given": N, "applied": N}` |
| `formatting` | `edit` on a spec | `preserved` (comments and layout kept) or `rewritten` |
| `fixed` | `fix` | `[{"rule": "...", "count": N}]` |
| `image` | `preview` | `width`, `height`, `scale`, `region`, `background`, `annotated` |
| `file`, `viewBox`, `summary`, `elements` | `inspect` | The scene: see `svgdog schema scene` |
| `rules` or `rule` | `rules` | The list, or one rule explained |

Example of a successful render:

```json
{"tool":"svgdog","version":"1.0.0","command":"render","ok":true,"summary":{"files":1,"errors":0,"warnings":0,"info":0,"fixable":0,"truncated":false},"findings":[],"outputs":[{"path":"o.svg","bytes":3052}]}
```

### Findings

| Field | Meaning |
|---|---|
| `rule` | Rule id (see [Rules catalogue](rules.md)), or `invalid-suppression` |
| `severity` | `info`, `warning` or `error`, after profile, config and approximation capping |
| `file` | The file checked; `-` for stdin |
| `target` | A selector that finds the element again, such as `#node-api > rect` or `svg > rect:nth-of-type(2)`; `:root` for the whole file |
| `line` | Line number in the source |
| `bbox` | Optional `[x, y, width, height]` in viewBox units |
| `message` | Human-readable explanation |
| `measured` | Optional measurements; includes `"approximate": true` for geometry findings on SVGs SVGDog did not draw |
| `fix` | Optional autofix: `{"safe": true, "ops": [...]}`, with `safe` false for unsafe fixes. The ops are `edit --ops` operations; `safe` fixes are applied by `fix`, the others only with `--unsafe` |

### Errors

A command that cannot finish answers with `"ok": false` and an `error` object instead of findings.

```json
{"tool":"svgdog","version":"1.0.0","command":"render","ok":false,"error":{"code":"unknown-reference","message":"edges[0].to names 'b', which is not defined.","path":"edges[0].to","suggestion":"Did you mean 'a'?"}}
```

| Field | Meaning |
|---|---|
| `code` | Stable, kebab-case identifier |
| `message` | What went wrong |
| `path` | Optional: where, such as `nodes[0].kynd`, a file name or a flag |
| `suggestion` | Optional: a did-you-mean or the next step |

In text mode the same error is printed on stderr as `error[code]: message` followed by the suggestion. `ok` is `false` both for an error and for findings at or above the fail level; tell them apart by the presence of `error`.

## Exit codes

| Code | Name | When |
|---|---|---|
| 0 | OK | Success, with no finding at or above the fail level. Help and version also exit 0 |
| 1 | Findings | At least one finding at or above the fail level: `error` for `render`, `edit` and `fix`; `--fail-on` (default `error`) for `validate`. `preview` and `inspect` never give 1 |
| 2 | Usage | Unknown flag or command, missing or conflicting arguments, unknown rule or profile on the command line |
| 3 | Input | A file is missing or unreadable or not UTF-8, a spec or config is invalid, an edit target matches nothing, a drawing is too wide |
| 4 | Output | The output file exists (and `--force` was not given), or writing failed |
| 5 | Internal | A bug. A panic is caught and reported as an `internal-error` envelope instead of a backtrace |

## Error codes

| Code | Exit | Raised when |
|---|---|---|
| `unknown-flag`, `unknown-command` | 2 | A flag or subcommand is not recognised |
| `missing-argument` | 2 | A required argument, or a destination for `edit` and `fix`, is absent |
| `argument-conflict` | 2 | Flags that exclude each other were given (`-o` with `--in-place`, `--set` with `--ops`) |
| `invalid-argument` | 2 | A value is unusable (a width under 16, a bad selector, stdin used twice) |
| `unknown-rule`, `unknown-profile` | 2 (3 in a config file) | An id is not in the catalogue or the profile list |
| `unknown-theme` | 2 (`--theme`) or 3 (spec) | Theme is not `docs-light`, `docs-dark` or `docs-auto` |
| `file-not-found`, `file-not-readable`, `not-utf8`, `stdin-not-readable` | 3 | An input could not be read |
| `malformed-spec`, `malformed-xml` | 3 | Input is not valid YAML/JSON or XML |
| `invalid-spec`, `unknown-field`, `missing-field`, `invalid-value`, `duplicate-id`, `unknown-reference`, `group-cycle`, `group-overlap`, `at-outside-free`, `too-wide` | 3 | A spec failed validation; see [Spec reference](spec.md) |
| `invalid-config`, `config-not-readable` | 3 | `svgdog.toml` is invalid or unreadable; see [Configuration](configuration.md) |
| `invalid-op`, `unknown-op`, `conflicting-ops`, `edit-breaks-xml`, `no-match` | 3 | An edit operation is malformed, clashes with another, would break the XML, or its target matches nothing |
| `not-renderable` | 3 | `preview` cannot rasterize the file |
| `outside-root`, `root-not-found` | 3 | MCP only: a path escapes the root, or the root is unusable |
| `output-exists` | 4 | The output exists; add `--force` (render, preview) or choose a new path |
| `write-failed` | 4 | The file could not be written |
| `internal-error` | 5 | A bug; report it with the command and the input |

Writes are atomic: SVGDog writes a temporary file beside the target and renames it, so a failure never leaves half a file.
