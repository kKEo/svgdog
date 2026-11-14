# From spec to validated spec

Before anything is drawn, SVGDog reads the spec file, checks it strictly, and builds an in-memory diagram. This chapter describes that stage and the mistakes it stops, because most failures an operator sees on the create path happen here, with exit code 3.

The stage has three steps, all in `src/spec.rs`.

## 1. Read the document

`read_document` (`src/spec.rs:219-230`) decides the format by the first character: if the text starts with `{` after leading whitespace it is parsed as JSON, otherwise as YAML. A file that does not parse is rejected with the error code `malformed-spec` and the parser's message.

The spec can come from a file or from standard input (`-`). The file extension does not matter.

## 2. Check structure and values

`parse` (`src/spec.rs:232-467`) walks the generic document and rejects what the format does not allow. The checks are strict on purpose, so an agent that mistypes a key learns at once:

- **Unknown fields are errors.** Each object type has a fixed list of allowed keys (`ROOT_FIELDS`, `NODE_FIELDS`, `EDGE_FIELDS`, `GROUP_FIELDS`). An unknown key fails with `unknown-field` and a did-you-mean suggestion when one is close.
- **Required fields.** `version`, `diagram`, `title` and `nodes` must be present (`missing-field`), and `nodes` must not be empty. `version` must be `1` (`invalid-value` otherwise).
- **Shape and length.** A value of the wrong type, an empty string or one over its length limit fails with `invalid-spec`.
- **Value ranges.** For example `max_width` must be a whole number from 200 to 2000 (default 960), titles are 1 to 120 characters, node labels at most 80, and `diagram` must be one of `flow`, `layers`, `sequence`, `free`, `state`, `timeline`, `compare` (`invalid-value`). The full table is in the [spec reference](../reference/spec.md).
- **Ids.** Node and group ids are lowercase slugs of at most 63 characters, and ids must be unique across nodes, groups and edges (`duplicate-id`). An edge without an `id` gets `<from>--<to>`.
- **References.** Every `from`, `to`, group member and note `attach` must name something that exists (`unknown-reference`).
- **Groups.** Groups may nest, but cycles are rejected (`group-cycle`) and two groups may not overlap unless one contains the other (`group-overlap`).
- **Position.** `at` on a node is allowed only in a `free` diagram (`at-outside-free`).
- **Sequence diagrams.** Edges must join two nodes; they cannot point at groups.

Every error carries a `path` into the spec (for example `nodes[2].label`) and, when possible, a suggestion. See [Troubleshooting](../operations/troubleshooting.md) for the common ones.

## 3. Build the diagram model

The checked values become a `Spec` structure: nodes with kind, emphasis, optional fixed width and optional position; edges with style and arrow; groups with members; notes with a target and a side. The author states structure and SVGDog chooses every coordinate. The only place an author gives coordinates is `at` in a `free` diagram, and even those are snapped to the 8 px grid.

## Choosing a theme

The theme is resolved before rendering starts (`choose` in `src/render/theme.rs`): the `--theme` flag wins over the spec's `theme` key, which wins over `[style] theme` in the config, which wins over the default `docs-light`. A theme is a `theme.toml`: one of the five compiled in (`docs`, `blueprint`, `paper`, `bold`, `cards`) or one in a folder on disk, and it carries colours, the font-family list, two text sizes, two line widths and two corner radii (see [Themes](../reference/themes.md)). Layout reads the sizes and the face from it, so two themes can give different box sizes. Each has a light and a dark variant; the adaptive one (`-auto`) is rendered with a placeholder per colour role; a last pass writes the light colours as ordinary attributes, gives each element a class per role (`f-tx`, `s-ed` and so on), and adds one `<style>` rule under `@media (prefers-color-scheme: dark)` with the dark colours. Take away the classes and that rule and the file is the light variant's output byte for byte. An unknown name is a usage error (exit 2) when it came from the flag and an input error (exit 3) when it came from the spec or the config.

## What happens next

A valid `Spec` goes to [layout](layout.md). If layout cannot fit the drawing in `max_width`, rendering fails with `too-wide` instead of producing an unreadable figure. After rendering, the SVG text is validated with the same rules as `svgdog validate`, unless `--no-validate` was given.

## Editing a spec

`svgdog edit` on a spec works on this same data. It changes the document, validates it again, re-renders it and reports findings on the drawing. To keep your comments and layout it patches the YAML text where it can, and keeps the patched text only if it reads back as exactly the intended data; otherwise it rewrites the spec in canonical form and says so with `"formatting": "rewritten"`. See [Known limits](../operations/known-limits.md).
