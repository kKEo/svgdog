# JSON Schemas

SVGDog embeds six JSON Schemas (draft 2020-12) in the binary. They describe the formats it reads and writes, so an editor, a CI job or an agent can check a file or a result without running SVGDog. The binary and the schemas always match: `svgdog schema NAME` prints the copy compiled into the build you are running.

## Getting a schema

```bash
svgdog schema spec > spec.schema.json
svgdog schema findings --format text     # pretty-printed, as stored in the repository
```

With `--format json` (the default when piped) the schema is printed as compact JSON; with `--format text` it is printed as stored. Either way the output is the schema document itself, not wrapped in an envelope. The source files are `schemas/*.json` in the repository.

The `$id` values (`https://svgdog.example/schema/<name>-1.json`) are identifiers on a placeholder domain, not URLs that resolve. Point your validator at the local file rather than the `$id`.

## The six schemas

| Name | Title | Describes | Used by |
|---|---|---|---|
| `spec` | SVGDog diagram spec | The diagram spec: root keys `version`, `diagram`, `title`, `description`, `direction`, `theme`, `max_width`, `nodes`, `edges`, `groups`, `notes`; required `version`, `diagram`, `title`, `nodes`; no extra keys. Definitions for `id`, `node`, `edge`, `group`, `note` | Authoring specs; see [Spec reference](spec.md) |
| `config` | svgdog.toml | The config file as data (convert the TOML to JSON to validate it): `version`, `profile`, `target`, `style`, `rules`, `overrides`. A rule entry is a severity (`off`, `info`, `warning`, `error`) or an object with an optional `severity` and non-negative numeric thresholds | Editors and CI; see [Configuration](configuration.md) |
| `findings` | SVGDog result envelope | The envelope of `render`, `validate`, `edit`, `fix` and `preview`, and the error shape. Required `tool`, `version`, `command`, `ok`; optional `summary`, `findings`, `outputs`, `error`, `changed`, `dry_run`, `diff`, `diff_truncated`, `operations`, `fixed`, `validated`, `image`, `formatting`. Definitions for `bbox`, `finding`, `error` | Parsing results; see [Envelope and exit codes](envelope-exit-codes.md) |
| `scene` | SVGDog scene | The result of `inspect`: `file`, `viewBox`, `summary`, `elements` (each with `target`, `line`, `depth` and the requested fields), or an `error` | Reading drawings as data |
| `theme` | theme.toml | A theme file as data (convert the TOML to JSON to validate it): `name` and `light` required; `version`, `description`, `font` (`family`, `size`, `small`), `line` (`width`, `thin`, `corner`, `box_corner`), `dark`. The nine colours of `light` and `dark` are `#rrggbb` | Editors and CI; see [Themes](themes.md) |
| `ops` | SVGDog edit operations | The input of `edit --ops`: an array of operations, or an object with an `ops` array such as a finding's `fix`. Operations: `set`, `remove`, `move`, `rename-id`, `add-node`, `add-edge` (spec only) | Scripting edits |

## Validating against a schema

Any draft 2020-12 validator works. Examples (the validators are not part of SVGDog):

```bash
# Python, with the jsonschema package installed
python3 -c "
import json, subprocess, jsonschema, yaml
schema = json.loads(subprocess.check_output(['svgdog', 'schema', 'spec']))
jsonschema.validate(yaml.safe_load(open('orders.yaml')), schema)
print('valid')"

# Node, with ajv-cli installed (spec given as JSON)
svgdog schema findings > findings.schema.json
svgdog validate fig/orders.svg > result.json
ajv validate --spec=draft2020 -s findings.schema.json -d result.json
```

A schema check is not a substitute for SVGDog's own validation. Rules such as unique ids across nodes, groups and edges, unknown references, group cycles and `at` only in a `free` diagram live in the parser (see [Spec reference](spec.md#cross-checks-and-errors)), not in the schema.

## Using the schemas in an editor

Most YAML-aware editors accept a schema per file. Save `svgdog schema spec` next to your specs and reference it, for example with the YAML language server comment:

```yaml
# yaml-language-server: $schema=./spec.schema.json
version: 1
diagram: flow
```

## Stability

Every schema carries its major version in its `$id` (`-1`). The spec and config files also carry `version: 1`; a different value is rejected with a message saying this build reads version 1.
