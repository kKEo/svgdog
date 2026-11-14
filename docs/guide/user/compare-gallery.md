# Compare gallery

Two comparisons with the specs that draw them. Make two groups, one per side. Nodes with the same label share a row; a node that is on one side only gets its own row and is highlighted. For the rules, see [Diagram types](diagram-types.md#compare).

## Before and after

The `Redis cache` is new, so it stands out. The legend explains the arrows.

<!-- spec: compare-cache -->
```yaml
version: 1
diagram: compare
title: Adding a cache
description: Before, every read went to the database; after, a cache answers most reads.
legend: true
nodes:
  - { id: b-app, label: App, kind: client }
  - { id: b-api, label: API, kind: service }
  - { id: b-db, label: Database, kind: datastore }
  - { id: a-app, label: App, kind: client }
  - { id: a-api, label: API, kind: service }
  - { id: a-cache, label: Redis cache, kind: datastore }
  - { id: a-db, label: Database, kind: datastore }
groups:
  - { id: before, label: Before, members: [b-app, b-api, b-db] }
  - { id: after, label: After, members: [a-app, a-api, a-cache, a-db] }
edges:
  - "b-app -> b-api"
  - "b-api -> b-db: every read"
  - "a-app -> a-api"
  - "a-api -> a-cache: most reads"
  - "a-api --> a-db: on a miss"
```

![A comparison titled Adding a cache. On the left, in a frame labelled Before, an App calls an API, which sends every read to a Database. On the right, in a frame labelled After, the App calls the API, which sends most reads to a highlighted Redis cache and, with a dashed arrow, goes to the Database on a miss. A legend explains the solid and dashed lines.](images/compare-cache.svg)

## Two options

The same layout works for a choice that has not been made yet. The caption says what the reader should take away.

<!-- spec: compare-options -->
```yaml
version: 1
diagram: compare
title: Two ways to send notifications
description: Option A calls the mail service directly; option B puts a queue and a worker in between.
caption: Option B survives a mail outage at the cost of one more moving part.
nodes:
  - { id: a-app, label: Orders service, kind: service }
  - { id: a-mail, label: Mail service, kind: external }
  - { id: b-app, label: Orders service, kind: service }
  - { id: b-queue, label: Notification queue, kind: queue }
  - { id: b-worker, label: Notifier, kind: service }
  - { id: b-mail, label: Mail service, kind: external }
groups:
  - { id: option-a, label: "Option A: direct", members: [a-app, a-mail] }
  - { id: option-b, label: "Option B: queued", members: [b-app, b-queue, b-worker, b-mail] }
edges:
  - "a-app -> a-mail: send"
  - "b-app --> b-queue: publish"
  - "b-queue --> b-worker"
  - "b-worker -> b-mail: send"
```

![A comparison titled Two ways to send notifications. In Option A: direct, the Orders service sends straight to the Mail service. In Option B: queued, the Orders service publishes to a highlighted Notification queue, which feeds a highlighted Notifier that sends to the Mail service. A caption says option B survives a mail outage at the cost of one more moving part.](images/compare-options.svg)

## Tips

- Ids must be unique across both sides; a prefix such as `b-` and `a-` keeps them apart.
- When something was renamed, give the new node `match:` with the old label so they share a row.
