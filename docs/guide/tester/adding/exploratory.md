# Exploratory testing by hand

This page is a guided half-hour session: it uses the commands and fixtures that ship with SVGDog to look for problems that the automated tests do not cover.

Run the commands from the repository root with a release binary (`cargo build --release`, binary at `target/release/svgdog`). Write outputs to a scratch directory, not into the repository.

```bash
alias svgdog=./target/release/svgdog
mkdir -p /tmp/svgdog-explore
```

## 1. Look at a deliberately broken file

`tests/fixtures/broken.svg` is a hand-made file with many faults. Validate it:

```bash
svgdog validate tests/fixtures/broken.svg --no-config ; echo "exit $?"
```

On version 1.0.0 you should see `"ok":false`, the summary `errors 7, warnings 11, info 2, fixable 15`, and exit code `1`. The findings cover 18 different rules, from `title-missing` to `foreign-object`. Anything that differs from this is worth a note.

Try the controls:

```bash
svgdog validate tests/fixtures/broken.svg --no-config --rules low-contrast,duplicate-id
svgdog validate tests/fixtures/broken.svg --no-config --skip desc-missing --fail-on warning
svgdog validate tests/fixtures/broken.svg --profile minimal
svgdog validate tests/fixtures/broken.svg --max-findings 3     # summary counts stay complete
```

## 2. See what the findings point at

Findings that sit on a drawn element carry a bounding box. Draw them:

```bash
svgdog preview tests/fixtures/broken.svg -o /tmp/svgdog-explore/broken.png --annotate
svgdog preview tests/fixtures/broken.svg -o /tmp/svgdog-explore/crop.png --crop '#a' --background dark
```

Open the PNGs. With `--annotate`, each finding's box is outlined with its rule id. Check that the outline sits on the element the message talks about. Findings marked `"approximate": true` in the JSON come from geometry measured on a file SVGDog did not draw, so small offsets are expected there.

## 3. Ask the scene questions

`inspect` reports what is on the page:

```bash
svgdog inspect tests/fixtures/orders.svg --select '#node-api' --fields id,tag,bbox,text
svgdog inspect tests/fixtures/orders.svg --depth 1
```

On version 1.0.0 the clean `orders.svg` has a `viewBox` of `0 0 720 240` and 17 elements, and `#node-api` returns three: the group, its `rect` and its `text`. Check the boxes against the image from `svgdog preview`.

## 4. Fix it, then check what the fixer did

```bash
svgdog fix tests/fixtures/broken.svg --dry-run          # reports the diff, writes nothing
svgdog fix tests/fixtures/broken.svg -o /tmp/svgdog-explore/fixed.svg
svgdog validate /tmp/svgdog-explore/fixed.svg --no-config
svgdog fix /tmp/svgdog-explore/fixed.svg --dry-run      # must be an empty diff (R9)
```

After the safe fixes, the dry run on version 1.0.0 leaves `errors 6, warnings 5, fixable 6`. The remaining findings need a human or `--unsafe`. Then try `--unsafe` and look at the picture again: unsafe fixes move or recolour content, so check that nothing now overlaps.

## 5. Break the inputs on purpose

| Try | Expect |
|---|---|
| `svgdog validate nope.svg` | exit code 3 and `"code":"file-not-found"` |
| `svgdog validate tests/fixtures/orders.svg --bogus` | exit code 2 and `"code":"unknown-flag"` |
| `svgdog frobnicate` | exit code 2 |
| a spec with a misspelt key, such as `lable:` instead of `label:` | a "did you mean" suggestion with the path |
| a spec with two nodes sharing an id | `duplicate-id` error naming both |
| a 40-node spec in `flow` direction `right` | either a clean render or a `too-wide` error with a hint |
| an SVG with a UTF-8 label, an emoji, or a very long label | no crash; check the picture |
| `svgdog render spec.yaml -o -` and `svgdog validate -` | stdout and stdin forms work |
| rendering the same spec twice and running `diff` | no difference (R8) |

## 6. Edit with comments in the spec

Copy `tests/fixtures/orders.yaml`, add `# my comment` lines, then edit through svgdog:

```bash
cp tests/fixtures/orders.yaml /tmp/svgdog-explore/o.yaml
# add a comment to the file, then:
svgdog edit /tmp/svgdog-explore/o.yaml --set '#api.label=Orders API' --in-place
diff tests/fixtures/orders.yaml /tmp/svgdog-explore/o.yaml
```

The comments should survive, and the JSON envelope should report `"formatting": "preserved"`. Removing a node always rewrites the file in canonical form (`"formatting": "rewritten"`); that is expected. The test `spec_edits_keep_yaml_comments` covers the basic case, so spend your time on odd layouts: flow-style lists, anchors, multi-line strings, Windows line endings.

## 7. Drive the MCP server

```bash
mkdir -p /tmp/svgdog-explore/root && cp tests/fixtures/orders.yaml /tmp/svgdog-explore/root/
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | svgdog mcp --root /tmp/svgdog-explore/root
```

You should get one JSON line listing eight tools. Then try a call whose path escapes the root (`../x.svg`): it must be refused.

## Write down what you find

For each problem, note the command, the input file, the expected and the actual result, and the version (`svgdog --version`). Turn every real bug into a test with [Adding tests and the pull-request checklist](adding-tests.md).
