# Known limits

This page lists what SVGDog does not do, or does only approximately, so you can decide before you build a pipeline on it. Each item is a property of version 1.0.0 as read from the code and the README, not a bug list.

## Checking

- **Geometry findings on foreign SVGs are heuristic.** For a file SVGDog did not render, measurements such as overlap, clipping and text size rest on estimates. Those findings carry `"approximate": true` and are capped at warning severity. A clean result on a hand-made SVG is weaker evidence than on SVGDog output. See [The rule engine](../how-it-works/rules-engine.md#findings-that-cannot-be-exact).
- **Static SVG only.** There is no script execution, no animation and no external resources. The rules flag `<script>`, `<foreignObject>`, external references and animation rather than evaluate them.
- **Output cap.** At most 50 findings are listed by default (`--max-findings` raises it); `summary.truncated` says when the list was cut.

## Creating

- **Four diagram types:** `flow`, `layers`, `sequence` and `free`. There is no class diagram, entity-relationship diagram, Gantt chart or free-hand drawing.
- **Nine node kinds, three edge styles, two themes.** The kinds are `box`, `service`, `client`, `datastore`, `queue`, `actor`, `external`, `decision` and `document`; the styles are `sync`, `async` and `dependency`; the themes are `docs-light` and `docs-dark`. Colours cannot be set per node, except through `emphasis`.
- **No manual routing.** You cannot place a bend. Routes come from an automatic search; the levers you have are listed in [Edge routing](../how-it-works/routing.md).
- **Width is the constraint.** A drawing wider than `max_width` (at most 2000) is first set more tightly and then rejected with `too-wide`. Height is unconstrained.
- **Size limits on text:** titles up to 120 characters, labels 80, edge labels 40, group labels 60, notes 120.
- **A spec is version 1 only.** `version: 1` is required and anything else is rejected.
- **Sequence diagrams** join nodes only; a message cannot point at a group. They have no activation boxes, fragments or alternatives.

## Editing

- **Editing a YAML spec keeps comments and layout for most changes.** Field changes, renames and additions are patched into the text. Removing a node, group or edge rewrites the spec in canonical form, and JSON specs are always rewritten. The result says `"formatting": "preserved"` or `"rewritten"`, so check it before committing if your comments matter.
- **`fix` runs at most 30 rounds** and stops early if two fixes undo each other. Some findings have no fix at all (`fixable` counts those that do).
- **Unsafe fixes change appearance.** They move or recolour content; read the diff from `--dry-run` first.

## Preview

- **`preview` writes a PNG file, nothing else.** It is not a server: there is no port, no watching and no live reload. Re-run it after a change.
- **Size caps:** the image is at most 2000 px wide and 4000 px high. A `--width` above 2000 is reduced and the text output says so. The minimum is 16.
- **Bundled fonts only.** Text always renders in Inter or JetBrains Mono, so a preview shows what SVGDog measured, not what a viewer with other fonts will see. The 8 % width reserve on labels exists for that gap.

## Running

- **Single binary, no daemon.** There is no HTTP server and no background mode. `svgdog mcp` speaks stdio only, one JSON-RPC message per line.
- **The MCP server has not been run against a real client.** It is tested by the repository's own tests, which cover the handshake, the envelope, image content and path confinement.
- **Standard input once.** `-` can be given only once per command.
- **The `mcp` command cannot be started through an MCP tool call.**

## Project status

- **No licence for SVGDog itself.** `Cargo.toml` has no `license`, `description` or `repository`, and there is no LICENSE file. The bundled fonts have their own licence files (SIL Open Font License) in `fonts/`.
- **No published releases yet.** The release workflow is written but not committed, and no tag exists; see [Release binaries](../install/release-binaries.md).
- **The JSON Schemas use a placeholder domain.** The `$id` values in `schemas/*.json` start with `https://svgdog.example/schema/`, which does not resolve. Use `svgdog schema <name>` to get the schema rather than fetching the id.
- **The requirements document is not in the repository.** Tests and CI comments cite "R1 to R16" from a specification that is not tracked here.
- **Cross-platform determinism rests on CI that has not yet been seen passing.** The golden-file test is designed to prove it on Linux, macOS and Windows; see [Determinism](../how-it-works/determinism.md).
