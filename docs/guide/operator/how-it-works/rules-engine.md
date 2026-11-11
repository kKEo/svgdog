# The rule engine

`validate`, `render`, `edit`, `fix` and `preview --annotate` all use one engine to check an SVG. This chapter explains how a rule run works, how profiles and `svgdog.toml` change what runs, and how a single finding is silenced. For the list of rules, see the [rules catalogue](../reference/rules.md); for the config keys, see [Configuration](../reference/configuration.md).

## One run, step by step

`rules::analyze` (`src/rules/mod.rs:142-202`) takes a file name, the SVG text and the resolved settings for that file:

1. **Parse.** The text is parsed as XML. If it is not well-formed, the only finding is `xml-well-formed` (when that rule is on) and the run ends.
2. **Build the scene.** `Scene::build` resolves styles, measures text and computes a box for every visible element.
3. **Read suppressions.** `svgdog-disable` comments are collected.
4. **Run four rule groups.** Structure, geometry, portability and accessibility each add raw hits (`src/rules/structure.rs`, `geometry.rs`, `portability.rs`, `accessibility.rs`).
5. **Turn hits into findings.** A hit is dropped if its rule is off or if a suppression covers its element or an ancestor. Otherwise it gets its severity from the settings, a target selector, a line number and a box.
6. **Sort.** Findings are ordered by line, then rule id.

The catalogue (`src/catalogue.rs`) holds each rule's group, default severity, autofix level (`none`, `safe` or `unsafe`) and numeric options. There are 38 rules: 10 structure, 12 geometry, 10 portability, 6 accessibility. Run `svgdog rules` to list them and `svgdog rules --explain <id>` to see a failing and a passing example; those examples are executed by the test suite.

## Findings that cannot be exact

Geometry rules measure positions. On a file SVGDog rendered, the measurements are exact. On any other SVG they rest on heuristics, so every geometry finding on such a file is marked `"approximate": true` in its `measured` data and its severity is capped at `warning` (`src/rules/mod.rs:179-184`). The difference is visible in practice: `content-clipped` is an error by default, but on the hand-made `tests/fixtures/broken.svg` it is reported as a warning.

## Profiles

A profile sets the starting severities (`Profile::default_level`, `src/config.rs:30-47`). Pick one with `--profile` or `profile =` in `svgdog.toml`; the default is `docs`.

| Profile | Rules on | Severities |
| --- | --- | --- |
| `minimal` | The structure group, plus `script-content` and `external-resource` | As catalogued |
| `docs` | All 38 | As catalogued |
| `strict` | All 38 | `info` becomes `warning`; `warning` and `error` both become `error` |

On `tests/fixtures/broken.svg` the three profiles give `3 errors, 0 warnings, 1 info`, `7 errors, 11 warnings, 2 info` and `10 errors, 10 warnings, 0 info`.

## Precedence

For one input file, settings are resolved in this order, later winning (`Config::settings`, `src/config.rs:324`):

1. The profile's defaults.
2. The `[rules]` table in `svgdog.toml`.
3. Each `[[overrides]]` block whose `files` globs match the file, in file order. Globs are matched against the path relative to the config file's directory.
4. Command-line selection: `--rules` keeps only the listed rules (a rule that a config turned off is switched back on at its catalogue severity), and `--skip` turns rules off.

A rule can be set to `"off"`, `"info"`, `"warning"` or `"error"`, or to a table that sets `severity` and numeric options, such as `font-too-small = { severity = "error", min_px = 10 }`.

## Finding the config

Unless `--config` or `--no-config` is given, SVGDog looks for `svgdog.toml` in the input file's directory and then in each parent (`Loader::for_input`, `src/config.rs:411-440`). The first one found is used and nothing is merged from further up. Under `svgdog mcp`, the search stops at the server root. When a config file is read, SVGDog prints `svgdog: config <path>` on stderr, unless `--quiet` is set. A config with an unknown key, rule or option is an error (exit 3), so a typo cannot silently disable a check.

## Suppression

One element can be exempted from one or more rules with a comment directly before it:

```xml
<!-- svgdog-disable shape-overlap: badge sits on the corner by design -->
<circle .../>
```

The reason after the colon is required. A comment with no reason, an empty reason, an unknown rule id, no rule at all, or no element after it raises `invalid-suppression` (an error), so suppressions cannot go stale unnoticed. A suppression covers the element that follows it and everything inside it. Several rules can be listed, separated by commas.

> **Warning.** Prefer suppressing one element over switching a rule off for the whole project. A project-wide `"off"` also hides real problems in diagrams nobody has looked at yet.

## Autofix

A finding may carry a `fix`: a list of edit operations in the same format that `svgdog edit --ops` takes, marked `safe` or not. `svgdog fix` applies the safe ones; `--unsafe` adds the ones that move or recolour content. It repeats up to 30 rounds, because a fix can reveal another (adding a viewBox makes margins measurable), and it stops early when two fixes would undo each other (`src/commands/fix.rs`). Fixes whose result depends on the drawing's scale (`stroke-too-thin`, `font-too-small`) wait until the scale-changing fixes (`viewbox-missing`, `content-clipped`, `excess-margin`, `text-overflow`) have settled.
