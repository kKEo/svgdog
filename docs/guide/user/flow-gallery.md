# Flow gallery

Two flow diagrams with the specs that draw them. For the rules, see [Diagram types](diagram-types.md#flow).

## A webhook, top to bottom

A frame around the parts you own, the three arrow styles, a highlighted box, a note and a legend. The short form `"a -> b: label"` is a call, `-->` is an asynchronous message and `..>` is a dependency.

<!-- spec: flow-webhook -->
```yaml
version: 1
diagram: flow
direction: down
title: Handling a webhook
description: A payment provider calls a webhook, which is queued and processed by a worker that updates orders and emails the customer.
legend: true
nodes:
  - { id: provider, label: Payment provider, kind: external }
  - { id: hook, label: Webhook endpoint, kind: service }
  - { id: queue, label: Events queue, kind: queue }
  - { id: worker, label: Worker, kind: service, emphasis: true }
  - { id: orders, label: Orders, kind: datastore }
  - { id: mail, label: Mail service, kind: external }
groups:
  - { id: ours, label: Our system, members: [hook, queue, worker, orders] }
edges:
  - "provider -> hook: payment.succeeded"
  - "hook --> queue: enqueue"
  - "queue --> worker: deliver"
  - "worker -> orders: mark paid"
  - "worker ..> mail: send receipt"
notes:
  - { attach: hook, text: Answers 200 at once }
```

![A top-to-bottom flow titled Handling a webhook. A Payment provider sends payment.succeeded to a Webhook endpoint inside a frame labelled Our system; a note says it answers 200 at once. The endpoint enqueues to an Events queue, which delivers to a highlighted Worker. The worker marks the order paid in Orders and, with a dotted dependency line, sends a receipt through a Mail service outside the frame. A legend explains the line styles and the highlight.](images/flow-webhook.svg)

## A review loop with decisions

Decision diamonds and arrows that lead back up. SVGDog keeps the main direction readable and routes the loops around the side.

<!-- spec: flow-review -->
```yaml
version: 1
diagram: flow
direction: down
title: Reviewing a pull request
description: A pull request is checked by CI and a reviewer, and goes back to the author until both pass.
caption: Nothing is merged until CI and the reviewer agree.
nodes:
  - { id: author, label: Author, kind: actor }
  - { id: pr, label: Pull request, kind: document }
  - { id: ci, label: CI checks, kind: service }
  - { id: green, label: Checks pass?, kind: decision }
  - { id: review, label: Reviewer, kind: actor }
  - { id: ok, label: Approved?, kind: decision }
  - { id: main, label: Main branch, kind: datastore, emphasis: true }
edges:
  - "author -> pr: opens"
  - "pr -> ci: runs"
  - "ci -> green"
  - "green -> author: no, fix"
  - "green -> review: yes"
  - "review -> ok"
  - "ok -> author: changes requested"
  - "ok -> main: merge"
```

![A top-to-bottom flow titled Reviewing a pull request. The Author opens a Pull request, which runs CI checks, leading to a Checks pass? decision. No leads back to the author to fix; yes leads to the Reviewer and an Approved? decision. Changes requested leads back to the author; merge leads to the highlighted Main branch. A caption says nothing is merged until CI and the reviewer agree.](images/flow-review.svg)

## Tips

- Start with `direction: right`. Switch to `direction: down` when the drawing is too wide or the story reads from top to bottom.
- Use `kind` to say what a box is: `service`, `datastore`, `queue`, `actor`, `external`, `decision`, `document`.
- Highlight one or two boxes at most with `emphasis: true`; more and nothing stands out.
