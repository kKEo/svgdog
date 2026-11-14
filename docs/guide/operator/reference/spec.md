# Spec reference

A spec is a YAML or JSON file that describes a diagram; `svgdog render` compiles it to SVG. This page lists every key, its type and limit, the defaults, and the checks that reject a spec. The machine-readable form is `svgdog schema spec` (see [JSON Schemas](schemas.md)).

Parsing is strict: an unknown key is an error with a did-you-mean hint, not silently ignored. Text that starts with `{` is read as JSON, anything else as YAML. YAML scalars such as `42` or `true` are accepted where text is expected and read as `"42"` and `"true"`.

## Root keys

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `version` | integer | yes | | must be `1` |
| `diagram` | string | yes | | `flow`, `layers`, `sequence`, `free`, `state`, `timeline`, `compare` |
| `title` | string | yes | | 1 to 120 characters |
| `description` | string | no | generated | up to 500 characters; becomes the SVG `<desc>`. Without it the `<desc>` is the sentence `describe --short` prints |
| `direction` | string | no | `right` | `right`, `down` |
| `theme` | string | no | `[style] theme` in `svgdog.toml`, else `docs-light` | A built-in theme (`docs`, `blueprint`, `paper`, `bold`, with `-light`, `-dark` or `-auto`) or a theme folder relative to the spec file, at most 200 characters; `render --theme` overrides it. See [Themes](themes.md) |
| `max_width` | integer | no | 960 | whole number from 200 to 2000 |
| `nodes` | list | no | empty | may be omitted when edges or groups name the nodes; a spec needs at least one node either way |
| `edges` | list | no | empty | |
| `groups` | list | no | empty | |
| `notes` | list | no | empty | |
| `steps` | list | no | empty | at most 20; see [Steps, focus, legend and caption](#steps-focus-legend-and-caption) |
| `focus` | list of ids | no | empty | nodes, edges or groups |
| `legend` | boolean or list | no | `false` | |
| `caption` | string | no | | up to 300 characters |

`max_width` is the widest the drawing may be. The renderer tries three spacing levels; if the result is still wider, `render` fails with `too-wide` and a hint, rather than shrinking text below readable size.

## Diagram types

| `diagram` | Layout |
|---|---|
| `flow` | Layered placement along `direction`; cycles are broken, edges are routed orthogonally around boxes |
| `layers` | Top-level groups become full-length bands stacked across the flow; ungrouped nodes share an unframed first band |
| `sequence` | Nodes are lifelines in spec order; edges are messages, listed top to bottom in spec order |
| `free` | Every node is pinned with `at: [x, y]`, snapped to the 8 px grid |
| `state` | Placed and routed as a `flow`. The default node kind is `state`; `start` and `end` markers are allowed, and only here |
| `timeline` | Nodes in spec order along an axis, each with its `when`. Groups are phases and may not nest. `direction: right` wraps to rows at `max_width`, whole phases at a time; `direction: down` is one column. No edges and no notes (`invalid-spec`) |
| `compare` | Exactly two groups, drawn side by side, holding every node between them and no further groups (`invalid-spec` otherwise). Nodes with the same key share a row; a node without a counterpart is drawn emphasised. No notes |

## Nodes

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `id` | string | yes | | lowercase letters, digits and hyphens, starting with a letter; up to 63 characters |
| `label` | string | yes | | 1 to 80 characters; not needed, and not drawn, for `kind: start` and `kind: end` |
| `kind` | string | no | `box` (`state` in a state diagram) | `box`, `service`, `client`, `datastore`, `queue`, `actor`, `external`, `decision`, `document`, `state`; `start` and `end` in a state diagram only |
| `emphasis` | boolean | no | `false` | highlights the node |
| `width` | number | no | automatic | at least 40 |
| `at` | `[x, y]` | `free` only | | two numbers; allowed only when `diagram: free`, and required there |
| `when` | string | no | | `timeline` only; up to 30 characters, printed at the node's place on the axis |
| `match` | string | no | the label | `compare` only; 1 to 80 characters. Nodes on the two sides with the same key share a row. Rows follow the order of both sides, so a pair that would have to cross another is not aligned |

`nodes` may be left out. An id that an edge or a group names but no node declares becomes a `box` node whose label is the id with hyphens turned into spaces. Two guards keep this from hiding mistakes: an undeclared id of five or more characters that is one letter away from a declared node or group is refused as `unknown-reference` with a hint, and a note can only attach to something that exists. Declare a node as soon as it needs a label, a kind or emphasis.

## Edges

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `from` | string | yes | | a node or group id (nodes only in a `sequence`) |
| `to` | string | yes | | same |
| `id` | string | no | `<from>--<to>` | slug as for nodes; give parallel edges their own id |
| `label` | string | no | | up to 40 characters; blank labels are dropped |
| `style` | string | no | `sync` | `sync`, `async`, `dependency` |
| `arrow` | string | no | `end` | `end`, `both`, `none` |

An edge may also be written as one string, `from ARROW to` with an optional `: label`. There must be spaces around the arrow, and the label is everything after the first colon.

| Arrow | Means |
|---|---|
| `->` | `style: sync` |
| `-->` | `style: async` |
| `..>` | `style: dependency` |
| `<->`, `<-->` | sync or async with `arrow: both` |
| `--` | sync with `arrow: none` |

```yaml
edges:
  - "web -> api: POST /orders"
  - api --> queue
```

Quote a short edge that has a label, because YAML reads an unquoted `a -> b: text` as a mapping. A short edge cannot carry its own `id`; use the long form for parallel edges. Both forms give the same drawing, byte for byte. A string that does not parse is `invalid-value` at `edges[N]`.

In a `sequence`, an edge may join a node to itself (a self-message). In other diagrams a self-loop is also accepted.

Nodes, edges and groups also accept two optional keys:

| Key | Type | Limit or values | Effect |
|---|---|---|---|
| `detail` | string | up to 300 characters | written as a `<title>` child of the element's group, which browsers show as a tooltip |
| `link` | string | up to 500 characters; `http:`, `https:`, `mailto:` or a relative path; no spaces | the element's group is wrapped in `<a href>` |

Any other scheme (`javascript:`, `data:` and so on) and a bare `#fragment` are refused as `invalid-value`.

## Groups

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `id` | string | yes | | slug |
| `label` | string | no | | up to 60 characters |
| `members` | list of ids | yes | | non-empty; nodes or other groups; no repeats |

Groups nest: a group may list another group as a member. Membership must not form a cycle (`group-cycle`). A node or group listed in two groups is allowed only when one group is nested inside the other; otherwise the spec is rejected with `group-overlap`.

## Notes

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `attach` | string | yes | | id of a node, group or edge |
| `text` | string | yes | | 1 to 120 characters |
| `side` | string | no | `right` | `top`, `right`, `bottom`, `left` |

## Steps, focus, legend and caption

These keys add an explanation to the drawing. None of them moves a node: the layout of a spec is the same with and without them, except that an edge named by a step has room for its number beside the label.

**`steps`** is an ordered walk-through. A step is a sentence, or a mapping:

| Key | Type | Required | Limit or values |
|---|---|---|---|
| `text` | string | yes | 1 to 200 characters |
| `show` | list of ids | no | nodes, edges (by `id` or `from--to`) or groups |

Steps are numbered from 1 in spec order. Each thing a step shows carries a numbered badge (on a node's top-left corner, a group's top-right corner, or beside an edge's label) and the attribute `data-step` with the numbers. The sentences are listed under the drawing in `<g id="steps">`, one `<g id="step-N">` each.

**`focus`** lists ids to keep at full strength. Everything else keeps its shape and position but is drawn in the theme's muted colour without fill or emphasis. A group in focus brings its members with it, and an edge is in focus when it is listed or when both of its ends are.

**`legend`** explains the drawing's vocabulary in a row under it (`<g id="legend">`). `true` lists what the drawing uses: the edge styles, if it has more than one or any that is not `sync`, and emphasis, if any node has it. A list gives your own entries; each has `text` (1 to 60 characters) and exactly one of `style` (`sync`, `async`, `dependency`), `kind` (a node kind) or `emphasis: true`.

**`caption`** is text drawn under everything else (`<g id="caption">`).

With `render --frames`, each step becomes its own picture: that step's badges only, what it shows in focus, and the full step list with the current entry at full strength. All frames share one layout and one canvas size.

## Cross-checks and errors

Ids are unique across nodes, groups and edges together. After the shape is read, the spec is checked as a whole. Every failure is exit 3 and carries a `path` (such as `edges[0].to`) and, where a near miss exists, a `suggestion`.

| Code | Raised when |
|---|---|
| `malformed-spec` | The file is not valid YAML or JSON |
| `invalid-spec` | A value has the wrong type, is empty, or exceeds a length limit |
| `unknown-field` | A key is not allowed there; suggests the closest allowed key |
| `missing-field` | A required key is absent (`version`, `diagram`, `title`, `nodes`, `at` in free, `members`) |
| `invalid-value` | A value is outside its allowed set, a bad id, `max_width` out of range, `width` below 40 |
| `duplicate-id` | An id is used twice across nodes, groups and edges |
| `unknown-reference` | An edge endpoint, note target or group member names an id that is not defined (edge endpoints and members must be nodes or groups, not edges) |
| `group-cycle` | Group membership loops |
| `group-overlap` | A node or group belongs to two groups that are not nested |
| `at-outside-free` | `at` is set on a node in a diagram that is not `free` |
| `too-wide` | The drawing exceeds `max_width` even at the tightest spacing (raised by `render`) |

Examples:

```bash
$ svgdog render bad.yaml -o out.svg --format json
{"tool":"svgdog","version":"1.0.0","command":"render","ok":false,"error":{"code":"unknown-field","message":"Unknown field 'kynd' in nodes[0].","path":"nodes[0].kynd","suggestion":"Did you mean 'kind'?"}}

$ svgdog render bad2.yaml -o out.svg --format json
{"tool":"svgdog","version":"1.0.0","command":"render","ok":false,"error":{"code":"unknown-reference","message":"edges[0].to names 'b', which is not defined.","path":"edges[0].to","suggestion":"Did you mean 'a'?"}}
```

## Examples

A flow diagram with a group (excerpt of `tests/fixtures/orders.yaml`):

```yaml
version: 1
diagram: flow
title: Order placement
direction: right
theme: docs-light
nodes:
  - id: api
    label: Order API
    kind: service
    emphasis: true
groups:
  - id: backend
    label: Backend
    members: [api, db, queue]
edges:
  - from: api
    to: queue
    label: publish
    style: async
```

A sequence diagram with a self-message and a note (excerpt of `tests/fixtures/login.yaml`):

```yaml
diagram: sequence
edges:
  - {from: auth, to: auth, label: create code, id: make-code}
  - {from: auth, to: app, label: 202 Accepted, style: async, id: accepted}
notes:
  - {attach: make-code, text: "Six digits, valid for ten minutes"}
```

A free diagram with pinned nodes (excerpt of `tests/fixtures/rack.yaml`):

```yaml
version: 1
diagram: free
title: Rack layout
nodes:
  - {id: sw, label: Switch, at: [100, 20]}
  - {id: a, label: Node A, at: [20, 150], kind: service}
groups:
  - {id: rack, label: Rack 12, members: [a, b]}
edges:
  - {from: a, to: nas, arrow: both, label: NFS}
  - {from: b, to: nas, arrow: none, style: dependency}
```

Rendered output uses the ids `group-<id>`, `node-<id>`, `edge-<id>` and `note-<n>`, which is what `inspect --select` and `edit` selectors target. `tests/fixtures/checkout.yaml` (flow, `direction: down`, all nine node kinds, nested groups) and `tests/fixtures/tiers.yaml` (layers) show the remaining combinations.
