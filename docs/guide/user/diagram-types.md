# Diagram types

SVGDog draws seven kinds of diagram. You choose one with the `diagram:` line of your spec. This page shows each one with a real example and says when to pick it.

| Type | Best for | You decide the positions? |
|---|---|---|
| `flow` | How things connect and in what order: services, steps, data paths | No |
| `layers` | Stacks and tiers: architecture layers, deployment tiers | No |
| `sequence` | Who sends which message to whom, in time order | No |
| `free` | Drawings where position carries meaning: a rack, a floor plan | Yes |
| `state` | The states something can be in and what moves it between them | No |
| `timeline` | Things in time order: a plan, a history, a release schedule | No |
| `compare` | Two versions side by side: before and after, option A and option B | No |

In every type SVGDog places things on an 8-pixel grid, routes arrows around boxes, and keeps the result the same every time.

## Flow

Use `flow` when you want to show how parts connect. SVGDog arranges the nodes in ranks from left to right (`direction: right`, the default) or from top to bottom (`direction: down`). Arrows that would form a loop are drawn so the main direction stays readable.

The [writing a spec](writing-a-spec.md) page shows a left-to-right flow. Here is a larger top-to-bottom one that uses every node kind, a nested group and a self-loop:

![A top-to-bottom diagram titled Checkout platform. A Shopper actor reaches a Web app, which calls an API gateway. The gateway verifies against an Auth service and places orders with an Order service. The Order service charges a Payment service that captures from an external Card processor, then stores orders in a database and publishes to an event bus, which feeds a Notification worker that renders an Invoice PDF and has a retry loop on itself.](images/checkout.svg)

If the drawing would be wider than `max_width`, SVGDog first tries tighter spacing. If that still does not fit, it stops with advice instead of making an unreadable picture:

```text
error[too-wide]: The drawing is 2240 px wide even at the tightest spacing; max_width allows 960.
  Try 'direction: down', shorter labels, or a larger max_width.
```

The exit code is 3. Follow the advice: switch to `direction: down`, shorten labels, or raise `max_width`.

More examples to copy: [Flow gallery](flow-gallery.md).

## Layers

Use `layers` for tiers. Each top-level group becomes a band that runs across the full width, stacked in order. Nodes that are in no group share an unframed first band.

```yaml
version: 1
diagram: layers
title: Deployment tiers
nodes:
  - {id: cdn, label: CDN, kind: external}
  - {id: spa, label: Single-page app, kind: client}
  - {id: api, label: REST API, kind: service}
  - {id: jobs, label: Job runner, kind: service}
  - {id: pg, label: PostgreSQL, kind: datastore}
  - {id: redis, label: Redis, kind: datastore}
  - {id: s3, label: Object storage, kind: datastore}
groups:
  - {id: presentation, label: Presentation, members: [cdn, spa]}
  - {id: application, label: Application, members: [api, jobs]}
  - {id: storage, label: Storage, members: [pg, redis, s3]}
edges:
  - {from: cdn, to: spa}
  - {from: spa, to: api, label: JSON}
  - {from: api, to: pg}
  - {from: api, to: redis, label: cache}
  - {from: jobs, to: s3, label: upload}
  - {from: api, to: jobs, label: enqueue, style: async}
```

![A layered diagram titled Deployment tiers with three horizontal bands stacked top to bottom: Presentation holding a CDN and a single-page app, Application holding a REST API and a job runner, and Storage holding PostgreSQL, Redis and object storage. Arrows run down between the bands, for example JSON from the app to the API and a dashed enqueue arrow from the API to the job runner.](images/tiers.svg)

More examples to copy: [Layers gallery](layers-gallery.md).

## Sequence

Use `sequence` to show a conversation. Each node becomes a vertical line (a lifeline) in the order you list them, and each edge becomes a message. Messages are drawn from top to bottom in the order of the `edges` list, so write them in the order they happen. In a sequence diagram every edge must join two nodes.

```yaml
version: 1
diagram: sequence
title: Login with a one-time code
nodes:
  - {id: user, label: User, kind: actor}
  - {id: app, label: Web app, kind: client}
  - {id: auth, label: Auth service, kind: service, emphasis: true}
  - {id: mail, label: Mail provider, kind: external}
edges:
  - {from: user, to: app, label: enter email}
  - {from: app, to: auth, label: POST /login}
  - {from: auth, to: auth, label: create code, id: make-code}
  - {from: auth, to: mail, label: send code, style: async}
notes:
  - {attach: make-code, text: "Six digits, valid for ten minutes"}
```

This is a shortened version. The full example in the repository has five participants and eleven messages:

![A sequence diagram titled Login with a one-time code. Five vertical lifelines from left to right are User, Web app, Auth service, Users DB and Mail provider, with the Auth service and Users DB grouped in a frame labelled Backend. Messages run top to bottom: the user enters an email, the app posts to login, the auth service finds the user, creates a code, sends it by mail, answers 202 Accepted, and finally verifies the code and returns a session cookie.](images/login.svg)

Dashed messages (`style: async`) are replies or fire-and-forget calls.

More examples to copy: [Sequence gallery](sequence-gallery.md).

## Free

Use `free` when position carries meaning, such as which slot of a rack a machine sits in. You give each node an `at: [x, y]` position in pixels; SVGDog snaps it to the grid and still routes the arrows. `at` is allowed only in a `free` diagram, and SVGDog rejects it elsewhere.

```yaml
version: 1
diagram: free
title: Rack layout
nodes:
  - {id: sw, label: Switch, at: [100, 20]}
  - {id: a, label: Node A, at: [20, 150], kind: service}
  - {id: b, label: Node B, at: [203, 150], kind: service}
  - {id: nas, label: NAS, at: [110, 290], kind: datastore}
groups:
  - {id: rack, label: Rack 12, members: [a, b]}
edges:
  - {from: sw, to: a}
  - {from: sw, to: b}
  - {from: a, to: nas, arrow: both, label: NFS}
  - {from: b, to: nas, arrow: none, style: dependency}
notes:
  - {attach: sw, text: 10 GbE uplink, side: right}
```

![A free-form diagram titled Rack layout. A Switch box sits at the top with the note 10 GbE uplink. Below it, Node A and Node B sit side by side inside a frame labelled Rack 12. A NAS box is lower down. The switch connects to both nodes, Node A connects to the NAS with a double-headed arrow labelled NFS, and Node B connects to the NAS with a dotted line without arrowheads.](images/rack.svg)

More examples to copy: [Free gallery](free-gallery.md).

## State

A state diagram shows the states of one thing, such as an order or a connection, and the events that move it from one state to the next. Every node is a state, drawn with round corners. Two special kinds mark where it begins and ends: `kind: start` is a solid dot and `kind: end` is a ringed dot. They need no label.

```yaml
version: 1
diagram: state
title: Order states
nodes:
  - { id: begin, kind: start }
  - { id: draft, label: Draft }
  - { id: placed, label: Placed }
  - { id: done, kind: end }
edges:
  - "begin -> draft"
  - "draft -> placed: submit"
  - "placed -> draft: edit"
  - "placed -> done: delivered"
```

An arrow may lead back to an earlier state, or from a state to itself. Use `direction: down` for a tall diagram.

![A state diagram titled Order states. From a start dot the order becomes Draft, then Placed on submit, with an edit arrow back to Draft. From Placed it becomes Paid on payment ok, or Cancelled on payment failed. Paid has a retry loop on itself and leads to Shipped on dispatch. Shipped and Cancelled both lead to the end marker.](images/states.svg)

More examples to copy: [State gallery](state-gallery.md).

## Timeline

A timeline puts its nodes along a line in the order you list them. Give each node a `when`, the date or moment printed on the line. Groups become phases, framed around their members. A timeline has no edges: the order is the connection.

```yaml
version: 1
diagram: timeline
title: Migration plan
nodes:
  - { id: audit, label: Schema audit, when: Jan 2026 }
  - { id: dual, label: Dual writes on, when: Mar 2026 }
  - { id: cutover, label: Read cutover, when: Jun 2026 }
groups:
  - { id: prep, label: Preparation, members: [audit, dual] }
```

A long timeline continues on a new row when it reaches `max_width`; a phase is never split across rows. With `direction: down` the line runs from top to bottom, with the dates on its left, which suits many entries or long labels.

![A timeline titled Migration plan. A Preparation phase holds Schema audit in January 2026 and Dual writes on in March 2026. A Migration phase holds Backfill complete in April, which is highlighted, and Read cutover in June, marked as step 1. On a second row, Old cluster retired follows in August 2026.](images/migration.svg)

More examples to copy: [Timeline gallery](timeline-gallery.md).

## Compare

A comparison shows two versions next to each other. Make two groups, one per side, and put every node in one of them. Nodes with the same label land on the same row, so the reader sees at once what stayed. A node that exists on one side only gets a row of its own and is highlighted.

```yaml
version: 1
diagram: compare
title: Checkout before and after
nodes:
  - { id: b-web, label: Web app }
  - { id: b-mono, label: Monolith }
  - { id: a-web, label: Web app }
  - { id: a-orders, label: Orders service, match: Monolith }
  - { id: a-pay, label: Payments service }
groups:
  - { id: before, label: Before, members: [b-web, b-mono] }
  - { id: after, label: After, members: [a-web, a-orders, a-pay] }
```

When a node was renamed, tell SVGDog what it corresponds to with `match`: above, `Orders service` takes the row of `Monolith`. Ids must still be unique across both sides, so a prefix such as `b-` and `a-` helps. Arrows work as in a flow.

![A comparison titled Checkout before and after. On the left, in a frame labelled Before, a Web app calls a Monolith, which uses a Database. On the right, in a frame labelled After, the Web app calls an Orders service on the same row as the Monolith; a highlighted Payments service is new, and the Database is on the same row as before.](images/split.svg)

More examples to copy: [Compare gallery](compare-gallery.md).

## Which one should I pick?

- Start with `flow`. It is the most forgiving and covers most diagrams.
- Choose `layers` the moment you think "this is a stack".
- Choose `sequence` when the order of messages matters more than the structure.
- Choose `free` only when you must control where things sit.
- Choose `state` when the boxes are conditions of one thing, not separate things.
- Choose `timeline` when the only relationship is "then".
- Choose `compare` when the point is what changed.
