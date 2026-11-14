# Writing a spec

A spec is a small text file that describes your diagram. This page explains every part of it, with a picture to go with the main example. It takes about fifteen minutes to read.

Specs are written in YAML (a simple list-and-label format) or JSON. SVGDog treats a file that starts with `{` as JSON and anything else as YAML. The examples here use YAML.

## A complete example

This spec describes how an order moves through an online shop:

```yaml
version: 1
diagram: flow
title: Order placement
description: How an order moves from the storefront to fulfilment.
direction: right
theme: docs-light

nodes:
  - id: web
    label: Storefront
    kind: client
  - id: api
    label: Order API
    kind: service
    emphasis: true
  - id: db
    label: Orders DB
    kind: datastore
  - id: queue
    label: Fulfilment queue
    kind: queue

groups:
  - id: backend
    label: Backend
    members: [api, db, queue]

edges:
  - from: web
    to: api
    label: POST /orders
  - from: api
    to: db
    label: insert
  - from: api
    to: queue
    label: publish
    style: async

notes:
  - attach: queue
    text: At-least-once delivery
```

And this is the picture SVGDog draws from it:

![A left-to-right flow diagram titled Order placement. A Storefront box on the left sends POST /orders to a highlighted Order API box. The Order API, the Orders DB and the Fulfilment queue sit together inside a frame labelled Backend. The Order API sends insert to the Orders DB and a dashed publish arrow to the Fulfilment queue, which carries the note At-least-once delivery.](images/orders.svg)

## The top level

| Key | Required | What it does |
|---|---|---|
| `version` | yes | Always `1` for now |
| `diagram` | yes | The kind of diagram: `flow`, `layers`, `sequence`, `free`, `state`, `timeline` or `compare` (see [Diagram types](diagram-types.md)) |
| `title` | yes | The name of the diagram, 1 to 120 characters. It becomes the picture's accessible name |
| `description` | no | One or two sentences saying what the diagram shows, up to 500 characters |
| `direction` | no | `right` (default) or `down`: which way the flow runs |
| `theme` | no | A built-in theme such as `docs-light`, `paper` or `blueprint-dark`, or the folder of a theme of your own (see [Themes and settings](themes-and-settings.md)) |
| `max_width` | no | The widest the picture may be, 200 to 2000 px. The default is 960 |
| `nodes` | no | The boxes. You can leave the list out when the arrows name them (see [The short way](#the-short-way)) |
| `edges` | no | The arrows |
| `groups` | no | Frames drawn around related boxes |
| `notes` | no | Short callouts attached to a box, a group or an arrow |
| `steps`, `focus`, `legend`, `caption` | no | Words and markers that explain the picture (see [Explaining a diagram](explaining-a-diagram.md)) |

> **Tip.** If you leave out `description`, SVGDog writes one for you from the diagram, for example "Flow diagram: browser to api (request)". Screen readers speak it. Your own sentence is usually better, because it can say why the diagram matters; write one in real specs.

## Nodes: the boxes

Each node has an `id` (a short name you use to refer to it) and a `label` (the text shown).

- `id`: lowercase letters, digits and dashes, at most 63 characters. Each id must be unique across nodes, groups and edges.
- `label`: up to 80 characters. Use `\n` inside quotes for a line break.
- `kind`: the shape and colour. One of `box` (default), `service`, `client`, `datastore`, `queue`, `actor`, `external`, `decision` or `document`.
- `emphasis`: `true` draws the node with a stronger outline so it stands out.
- `width`: a minimum width in pixels, 40 or more.

## Edges: the arrows

An edge joins two nodes by their ids.

```yaml
edges:
  - {from: api, to: queue, label: publish, style: async}
```

- `from` and `to`: required.
- `label`: up to 40 characters, shown on the arrow.
- `style`: `sync` (solid line, the default), `async` (dashed line) or `dependency` (dotted line).
- `arrow`: `end` (default), `both` or `none`.
- `id`: optional. If you leave it out, SVGDog uses `from--to`. Give an edge an id when you want to attach a note to it.

A node may point to itself (`from: auth, to: auth`); SVGDog draws a small loop.

### The short way

An edge can also be one line of text: two ids with an arrow between them, and a label after a colon.

```yaml
edges:
  - "web -> api: POST /orders"
  - "api --> queue: publish"
  - api ..> cache
```

`->` is a solid arrow, `-->` a dashed one (`async`), `..>` a dotted one (`dependency`). `<->` points both ways and `--` has no arrowhead. Put spaces around the arrow, and put quotes around an edge that has a label.

With short edges you can skip the `nodes` list altogether. A name that only appears in an edge becomes a plain box labelled with that name, so this is a complete spec:

```yaml
version: 1
diagram: flow
title: Request path
edges:
  - "browser -> api: request"
  - "api -> database: query"
```

Add a node to `nodes` when you want a nicer label, a shape or emphasis. If a name in an edge is one letter away from a node you declared, SVGDog treats it as a typing mistake and tells you, so a slip does not quietly add a box.

## Groups: the frames

A group draws a labelled frame around some nodes.

```yaml
groups:
  - {id: backend, label: Backend, members: [api, db, queue]}
```

Groups may be nested inside each other. Two groups may not overlap without one containing the other, and a group cannot contain itself; SVGDog reports an error if you try.

## Notes: the callouts

A note is attached to a node, group or edge by its id.

```yaml
notes:
  - {attach: queue, text: At-least-once delivery}
  - {attach: core, text: Deployed as one unit, side: bottom}
```

`text` is up to 120 characters. `side` is `right` (default), `top`, `bottom` or `left`.

## Mistakes are explained

If the spec has a typo, SVGDog says where and often suggests the fix. For example a node written as `{id: a, labl: A}` gives:

```text
error[unknown-field]: Unknown field 'labl' in nodes[0].
  Did you mean 'label'?
```

and the exit code is 3. An edge that points at a node that does not exist gives:

```text
error[unknown-reference]: edges[0].to names 'bb', which is not defined.
  Did you mean 'a'?
```

## Start from an example

SVGDog carries a few small specs you can copy:

```bash
svgdog examples
svgdog examples flow --format text > my-diagram.yaml
```

## Read the format yourself

SVGDog can print the exact format it accepts, as a JSON Schema:

```bash
svgdog schema spec
```

Editors that understand JSON Schema can use this for completion and checking. The same text is what an AI assistant reads to learn the format.

## Make small changes without rewriting

To change one value in a spec, use `edit` rather than opening the file:

```bash
svgdog --format text edit hello.yaml --set '#api.label=Orders API' --dry-run
```

SVGDog shows the exact change to the file and, because of `--dry-run`, writes nothing. Remove `--dry-run` and add `--in-place` to apply it. Comments and layout in your YAML are kept, with one exception: removing a node, group or edge rewrites the file in a standard layout.
