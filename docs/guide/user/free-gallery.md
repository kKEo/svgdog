# Free gallery

Two free-form diagrams with the specs that draw them. You place every node with `at: [x, y]` in pixels; SVGDog snaps it to the grid and still routes the arrows. For the rules, see [Diagram types](diagram-types.md#free).

## An office network

Positions follow the floor: the line comes in at one corner, the access points sit at the far wall. `arrow: both` draws a cable that works both ways.

<!-- spec: free-office -->
```yaml
version: 1
diagram: free
title: Office network
description: Where the network equipment sits on the office floor and how it is cabled.
nodes:
  - { id: isp, label: Internet line, kind: external, at: [24, 24] }
  - { id: router, label: Router, at: [24, 160] }
  - { id: switch, label: Core switch, kind: service, emphasis: true, at: [264, 160] }
  - { id: ap1, label: Wi-Fi east, at: [504, 64] }
  - { id: ap2, label: Wi-Fi west, at: [504, 256] }
  - { id: nas, label: File server, kind: datastore, at: [264, 320] }
groups:
  - { id: closet, label: Server closet, members: [switch, nas] }
edges:
  - "isp -> router: fibre"
  - { from: router, to: switch, arrow: both }
  - { from: switch, to: ap1, arrow: both, label: PoE }
  - { from: switch, to: ap2, arrow: both, label: PoE }
  - { from: switch, to: nas, arrow: both }
notes:
  - { attach: isp, text: 1 Gbit/s, side: right }
```

![A free-form diagram titled Office network. An Internet line at the top left, with a note saying 1 Gbit/s, runs over fibre to a Router. The router is cabled to a highlighted Core switch inside a frame labelled Server closet, which also holds a File server. The switch powers two access points, Wi-Fi east and Wi-Fi west, over PoE.](images/free-office.svg)

## Two regions side by side

The layout mirrors the two regions, with the shared parts in the middle.

<!-- spec: free-regions -->
```yaml
version: 1
diagram: free
title: Two regions
description: The primary region replicates to a standby region, and both send backups to cold storage.
caption: The standby can take over within fifteen minutes.
nodes:
  - { id: lb, label: Global DNS, kind: external, at: [232, 152] }
  - { id: eu-app, label: App EU, kind: service, emphasis: true, at: [40, 152] }
  - { id: eu-db, label: Primary DB, kind: datastore, at: [40, 288] }
  - { id: us-app, label: App US, kind: service, at: [424, 152] }
  - { id: us-db, label: Standby DB, kind: datastore, at: [424, 288] }
  - { id: cold, label: Cold storage, kind: document, at: [232, 440] }
groups:
  - { id: eu, label: Europe (active), members: [eu-app, eu-db] }
  - { id: us, label: US (standby), members: [us-app, us-db] }
edges:
  - "lb -> eu-app"
  - { from: lb, to: us-app, style: dependency, label: failover }
  - "eu-app -> eu-db"
  - "us-app -> us-db"
  - "eu-db --> us-db: replicate"
  - "eu-db ..> cold: nightly"
  - "us-db ..> cold: nightly"
```

![A free-form diagram titled Two regions. A Global DNS box sits between a frame labelled Europe (active) on the left and US (standby) on the right. DNS sends traffic to the highlighted App EU, and to App US on failover with a dotted line. Each app uses its database; the Primary DB replicates to the Standby DB with a dashed arrow, and both send nightly backups to Cold storage below. A caption says the standby can take over within fifteen minutes.](images/free-regions.svg)

## Tips

- Use `free` only when position carries meaning. For anything else, `flow` places the boxes for you.
- Leave room above a frame's top-left corner: that is where its label goes, and an arrow passing there is reported as a clash.
