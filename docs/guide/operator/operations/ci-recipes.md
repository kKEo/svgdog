# CI recipes

This page gives tested recipes for running SVGDog in a pipeline. Each one uses only the exit code and the JSON result, so it works in any CI system. The examples use GitHub Actions syntax where a workflow is shown; the shell lines run anywhere.

All commands below were run against SVGDog 1.0.0. Output is JSON automatically when stdout is not a terminal, which is the case in CI; add `--format json` to be explicit.

## Recipe 1: fail a pull request on broken diagrams

Check every SVG in a docs folder and fail on any error:

```bash
svgdog validate 'docs/**/*.svg'
```

Quote the glob so SVGDog expands it rather than the shell, which keeps `**` working and the file list stable. The exit code is `0` when no finding reaches the fail level and `1` when one does. By default only errors fail the run; add `--fail-on warning` to make warnings fail too:

```bash
svgdog validate 'docs/**/*.svg' --fail-on warning
```

`--fail-on` exists only on `validate`. `render`, `edit` and `fix` always fail on errors.

Verified on a folder of three files (`orders.svg` and `tiers.svg` rendered by SVGDog, and the deliberately bad `broken.svg`): `"summary":{"files":3,"errors":7,"warnings":12,"info":2,"fixable":15,"truncated":false}` and exit code 1.

## Recipe 2: show failures as a short list

```bash
svgdog --format json validate 'docs/**/*.svg' \
  | jq -r '.findings[] | select(.severity=="error") | "\(.file):\(.line): \(.rule): \(.message)"'
```

Each line has the form `file:line: rule: message`, which most CI log viewers turn into a clickable annotation. The finding fields are listed in [Monitoring and diagnostics](monitoring.md).

## Recipe 3: keep committed SVGs in step with their specs

If you commit both `diagram.yaml` and the `diagram.svg` rendered from it, make CI prove they match. Rendering to standard output writes the raw SVG and nothing else, so it can be compared directly:

```bash
svgdog render docs/orders.yaml -o - | cmp - docs/img/orders.svg \
  && echo "up to date"
```

`cmp` exits non-zero when the files differ, and because SVGDog output is [deterministic](../how-it-works/determinism.md) a difference means the spec or the SVGDog version changed. Pin the SVGDog version in the pipeline, or this check will fail on a tool upgrade; see [Release binaries](../install/release-binaries.md#pinning-in-ci). The same command fails (exit 1) if the rendered diagram has errors, so it covers recipe 1 for rendered files.

## Recipe 4: render everything in one step

```bash
for spec in docs/specs/*.yaml; do
  svgdog render "$spec" -o "docs/img/$(basename "${spec%.yaml}").svg" --force || exit 1
done
```

`--force` replaces an existing output; without it a render onto an existing file stops with exit code 4 and the error `output-exists`. Writes are atomic, so a failed render never leaves a half-written SVG.

## Recipe 5: apply safe fixes and fail if anything is left

```bash
svgdog fix docs/img/hand-drawn.svg --in-place
svgdog validate docs/img/hand-drawn.svg
```

`fix` applies only fixes marked safe unless you pass `--unsafe`, which also applies ones that move or recolour content. It exits `1` if findings remain after the fixes, so it can be the last step. Commit the changed file or fail the job if `git diff --exit-code` shows a change, depending on whether you want CI to auto-fix or to demand the fix be made in the pull request.

## Recipe 6: per-folder rules

Put an `svgdog.toml` where it applies. SVGDog looks in the input file's directory and then each parent, so a file at the repository root covers everything and a nested one overrides it for its own folder. Use `[[overrides]]` to loosen rules for a legacy folder without touching the rest:

```toml
version = 1
profile = "docs"

[[overrides]]
files = ["legacy/**"]
rules = { shape-overlap = "off" }
```

See [Configuration](../reference/configuration.md) for every key. Run once with `--no-config` to see what the defaults would report.

## A complete GitHub Actions job

```yaml
jobs:
  diagrams:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install svgdog
        run: |
          # Replace with your pinned install; see Release binaries.
          cargo install --git https://github.com/kKEo/svg-diagram-generator --locked --tag v1.0.0 svgdog
      - name: Check diagrams
        run: svgdog validate 'docs/**/*.svg' --fail-on warning
```

The `--git ... --tag` form needs the tag to exist and has not been exercised; until a release is published, vendor the binary or build from a checkout (see [Build from source](../install/build-from-source.md)).

## Choosing the fail level

| If you want… | Use |
| --- | --- |
| Only real breakage to block merges | The default (`--fail-on error`) |
| A clean report | `--fail-on warning`, plus `profile = "strict"` in `svgdog.toml` |
| Only portability and structure checked | `--profile minimal` |

Run `svgdog rules` to see the 38 rules and their default severities.
