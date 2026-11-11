# Checking and fixing SVGs

SVGDog can check any SVG file, including ones drawn in other tools, against 38 rules for correctness, readability, portability and accessibility. It can then repair most problems itself. This page takes about ten minutes and uses the broken example file from the repository.

## Check a file: validate

```bash
svgdog --format text validate tests/fixtures/broken.svg
```

Each line is one finding: the line number in the file, how serious it is, the rule name, where it is, and a plain sentence. A `[fix]` tag means SVGDog can repair it safely; `[fix --unsafe]` means it can repair it but the repair changes how the picture looks. The first lines look like this:

```text
      3  warning  desc-missing        svg  The root <svg> has no <desc> saying what the diagram shows.
      3  info     editor-metadata     svg  The file holds editor namespaces (inkscape), 1 generator comment. [fix]
      3  warning  fixed-size          svg  The root sets width="400" and height="200", so the image cannot scale to the page. [fix]
      3  error    title-missing       svg  The root <svg> has no <title>; it is the image's accessible name.
```

and the report ends with a summary and exit code 1:

```text
1 file checked: 7 errors, 11 warnings, 2 info, 15 fixable
```

### What the severities mean

| Severity | Meaning | Fails the check? |
|---|---|---|
| error | The picture is wrong, unreadable or unsafe | Yes (exit code 1) |
| warning | Probably a problem; worth fixing | Only with `--fail-on warning` |
| info | A tidiness note | No |

Check several files at once with several names or a pattern such as `"docs/**/*.svg"`. To be strict in a build, add `--fail-on warning`.

## Repair a file: fix

```bash
svgdog --format text fix tests/fixtures/broken.svg --dry-run
```

`--dry-run` shows the result and writes nothing. When you are happy, choose where the repaired file goes:

```bash
svgdog --format text fix broken.svg --in-place     # replace the file
svgdog --format text fix broken.svg -o fixed.svg   # write a new file
```

On the example file, the safe repairs apply 9 fixes (for example `content-clipped`, `fixed-size`, `script-content` and `unused-def`) and write the file. Findings tagged `[fix --unsafe]` stay until you opt in:

```bash
svgdog --format text fix broken.svg --in-place --unsafe
```

That applies 6 more fixes: it renames a duplicate id, recolours text with too little contrast, moves a label off a line, and similar. The file you started with is left alone when you use `-o`, so you can compare.

Some problems SVGDog cannot repair because only you know the answer: a missing `<title>`, a reference to something that does not exist (`dangling-reference`), or `<foreignObject>` content. These stay in the list. Note that `fix` ends with exit code 0 even when findings remain; run `validate` afterwards to get a pass or fail answer.

> **Warning.** `fix --unsafe` changes how the picture looks. Always run it with `--dry-run` first, or write to a new file with `-o`.

## Change one thing: edit

`edit` changes a single value without touching the rest of the file:

```bash
svgdog --format text edit hello.svg --set '#node-api.fill=#ffeeaa' --dry-run
```

The output is a small before-and-after, like this:

```text
-  <g id="node-api">
+  <g id="node-api" fill="#ffeeaa">
```

`--set` takes `SELECTOR.attribute=value`. A selector is `#id` for one element, a tag name such as `rect`, or `tag[attr="value"]`. You can repeat `--set`. You must choose `-o FILE`, `--in-place` or `--dry-run`, so an edit never overwrites a file by accident.

For a spec file, `edit` changes the spec and redraws it; see [Writing a spec](writing-a-spec.md).

## Look inside: inspect

`inspect` lists the elements of an SVG with their position and size as `[x y width height]`:

```bash
svgdog --format text inspect hello.svg --select '#node-api'
```

```text
#node-api <g> [200 16 96 48]
  #node-api > rect <rect> [200 16 96 48]
  #node-api > text <text> [236.82 31.53 22.36 16.94] "API"

3 of 19 visible elements
```

Use it to find an element's id before an `edit`, or to understand a finding that points at `svg > rect:nth-of-type(2)`.

## Learn about a rule: rules

```bash
svgdog --format text rules                      # all 38 rules, grouped
svgdog --format text rules --explain low-contrast   # one rule, with a failing and a passing example
```

The rules are in four groups: structure (is the file valid), geometry (does everything fit and not collide), portability (will it look the same in other programs) and accessibility (can everyone read it). The full list with every setting is in the [Operator guide](../operator/).

## Silence one finding on purpose

If a finding is wrong for one element, put a comment directly before it, with a reason:

```xml
<!-- svgdog-disable shape-overlap: badge sits on the corner by design -->
```

The reason is required; a comment without one is itself reported as `invalid-suppression`.

## Files from other tools

For an SVG that SVGDog did not draw, findings about positions and overlaps rest on estimates. SVGDog marks them `approximate` and never rates them above warning.
