# Spec reference

A spec is a YAML or JSON file that describes a diagram; `svgdog render` compiles it to SVG. This page lists every key, its type and limit, the defaults, and the checks that reject a spec. The machine-readable form is `svgdog schema spec` (see [JSON Schemas](schemas.md)).

Parsing is strict: an unknown key is an error with a did-you-mean hint, not silently ignored. Text that starts with `{` is read as JSON, anything else as YAML. YAML scalars such as `42` or `true` are accepted where text is expected and read as `"42"` and `"true"`.

## Root keys

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `version` | integer | yes | | must be `1` |
| `diagram` | string | yes | | `flow`, `layers`, `sequence`, `free` |
| `title` | string | yes | | 1 to 120 characters |
| `description` | string | no | | up to 500 characters |
| `direction` | string | no | `right` | `right`, `down` |
| `theme` | string | no | `docs-light` | `docs-light`, `docs-dark`; `render --theme` overrides it |
| `max_width` | integer | no | 960 | whole number from 200 to 2000 |
| `nodes` | list | yes | | at least one node |
| `edges` | list | no | empty | |
| `groups` | list | no | empty | |
| `notes` | list | no | empty | |

`max_width` is the widest the drawing may be. The renderer tries three spacing levels; if the result is still wider, `render` fails with `too-wide` and a hint, rather than shrinking text below readable size.

## Diagram types

| `diagram` | Layout |
|---|---|
| `flow` | Layered placement along `direction`; cycles are broken, edges are routed orthogonally around boxes |
| `layers` | Top-level groups become full-length bands stacked across the flow; ungrouped nodes share an unframed first band |
| `sequence` | Nodes are lifelines in spec order; edges are messages, listed top to bottom in spec order |
| `free` | Every node is pinned with `at: [x, y]`, snapped to the 8 px grid |

## Nodes

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `id` | string | yes | | lowercase letters, digits and hyphens, starting with a letter; up to 63 characters |
| `label` | string | yes | | 1 to 80 characters |
| `kind` | string | no | `box` | `box`, `service`, `client`, `datastore`, `queue`, `actor`, `external`, `decision`, `document` |
| `emphasis` | boolean | no | `false` | highlights the node |
| `width` | number | no | automatic | at least 40 |
| `at` | `[x, y]` | `free` only | | two numbers; allowed only when `diagram: free`, and required there |

## Edges

| Key | Type | Required | Default | Limit or values |
|---|---|---|---|---|
| `from` | string | yes | | a node or group id (nodes only in a `sequence`) |
| `to` | string | yes | | same |
| `id` | string | no | `<from>--<to>` | slug as for nodes; give parallel edges their own id |
| `label` | string | no | | up to 40 characters; blank labels are dropped |
| `style` | string | no | `sync` | `sync`, `async`, `dependency` |
| `arrow` | string | no | `end` | `end`, `both`, `none` |

In a `sequence`, an edge may join a node to itself (a self-message). In other diagrams a self-loop is also accepted.

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
