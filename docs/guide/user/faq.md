# FAQ and troubleshooting

This page answers the questions people ask most and explains the messages SVGDog prints when something goes wrong. Find your message in the list; each entry says what it means and what to do.

## Messages and exit codes

### `error[output-exists]: hello.svg already exists; pass --force to replace it.` (exit code 4)

SVGDog will not overwrite a file by accident. Add `--force` to replace it, or choose a different name with `-o`. To update a diagram and keep its boxes where they are, add `--previous hello.svg` instead.

### `error[layout-changed]: The layout of orders.svg cannot be kept: 'api' changed size and no longer fits beside 'web'.` (exit code 4)

You asked SVGDog to keep the boxes where an earlier picture has them (`--previous`, or `svgdog docs` on a page whose picture exists), and after your change they no longer fit. Nothing was written. Make the change smaller (a shorter label, for example), or add `--force` to let SVGDog arrange the diagram again. See [Changing a diagram without moving it](changing-a-diagram.md).

### `error[unknown-field]: Unknown field 'labl' in nodes[0].` (exit code 3)

A key in your spec is misspelled or not allowed there. The line below the message suggests the fix (`Did you mean 'label'?`). Run `svgdog schema spec` to see every key that is allowed.

### `error[unknown-reference]: edges[0].to names 'bb', which is not defined.` (exit code 3)

An edge, group or note points at an id that no node, group or edge has. Check the spelling; the message suggests the closest id.

### `error[too-wide]: The drawing is 2240 px wide even at the tightest spacing; max_width allows 960.` (exit code 3)

The diagram does not fit in `max_width`. Switch to `direction: down`, shorten the labels, or raise `max_width` (up to 2000).

### `error[at-outside-free]: Node 'a' sets 'at', which is allowed only in a free diagram.` (exit code 3)

Positions (`at`) are for `diagram: free`. Either remove `at` or change the diagram type. See [Diagram types](diagram-types.md).

### `error[file-not-found]: File not found: nope.yaml.` (exit code 3)

The path is wrong. Check the spelling and the folder you are in.

### `error[unknown-flag]` or `error[missing-argument]` (exit code 2)

The command was typed wrongly: a flag that does not exist, or something required is missing (for `render`, both the spec and `-o`). Run `svgdog <command> --help`.

### `error[no-match]: --crop '#nope' matches no visible element with a box.` (exit code 3)

The selector in `--crop` found nothing. Use `svgdog inspect FILE` to list the ids that exist.

### Exit code 1

The check found problems at or above the failing level: errors by default, or warnings too with `--fail-on warning` on `validate`. Read the findings, then try `svgdog fix`.

### Exit code 5

An internal error. Please report it, with the command you ran and the input file, in the [repository](https://github.com/kKEo/svgdog).

## Questions

### `fix` says "No change" but findings remain. Why?

`fix` only applies repairs that findings carry. Findings without a `[fix]` tag need a person: a missing title, a reference to an element that does not exist. Findings tagged `[fix --unsafe]` need `--unsafe`, which can change the picture's look; try it with `--dry-run` first.

### `fix` finished with exit code 0 but the file still has errors. Is that a bug?

No. `fix` succeeds when it could do its job. Run `svgdog validate` afterwards to get a pass or fail answer.

### I get JSON instead of readable text.

When the output is not a terminal (for example it is piped or captured by a script), SVGDog prints JSON by default. Add `--format text`.

### Can I watch the preview update as I edit?

No. `preview` writes a PNG file when you run it; it has no window or watch mode. Run it again after each change.

### Why is there a warning `desc-missing`?

The SVG has no `<desc>` saying what it shows; screen readers speak that text. You see this on SVGs made by other tools. A diagram rendered by SVGDog always has one: your spec's `description`, or a sentence SVGDog writes from the diagram when you leave it out.

### Why does SVGDog change my SVG's width and height?

The `fixed-size` rule reports a root with fixed `width` and `height`, because such a picture cannot scale to its page. `fix` removes them and relies on the `viewBox`. If you need fixed sizes, switch the rule off in [`svgdog.toml`](themes-and-settings.md).

### Does editing a spec destroy my comments?

Not for ordinary changes: changing a value, renaming or adding keeps your comments and layout. Removing a node, group or edge rewrites the spec in a standard layout; the result says `"formatting": "rewritten"`.

### Where is the licence?

The repository does not state a licence for SVGDog yet. Ask the maintainers before you redistribute it. The two built-in fonts are under the SIL Open Font License, and their licence texts are in the `fonts/` folder of the repository.

### Where do I report a problem?

Open an issue in the [repository](https://github.com/kKEo/svgdog).

For deeper troubleshooting, such as CI failures and configuration questions, see the [Operator guide](../operator/).
