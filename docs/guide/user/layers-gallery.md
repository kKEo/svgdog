# Layers gallery

Two layered diagrams with the specs that draw them. Each top-level group becomes a band across the full width, stacked in the order you list the groups. Nodes in no group share an unframed band at the top. For the rules, see [Diagram types](diagram-types.md#layers).

## A product in tiers

The clients are in no group, so they sit above the first band.

<!-- spec: layers-saas -->
```yaml
version: 1
diagram: layers
title: A SaaS product in four tiers
description: Browsers and the mobile app go through the edge to the services, which keep their data in three stores.
nodes:
  - { id: browser, label: Browser, kind: client }
  - { id: mobile, label: Mobile app, kind: client }
  - { id: cdn, label: CDN, kind: external }
  - { id: lb, label: Load balancer, kind: service }
  - { id: api, label: API, kind: service, emphasis: true }
  - { id: jobs, label: Background jobs, kind: service }
  - { id: pg, label: PostgreSQL, kind: datastore }
  - { id: redis, label: Redis, kind: datastore }
  - { id: files, label: Object storage, kind: datastore }
groups:
  - { id: edge, label: Edge, members: [cdn, lb] }
  - { id: app, label: Application, members: [api, jobs] }
  - { id: data, label: Data, members: [pg, redis, files] }
edges:
  - "browser -> cdn"
  - "browser -> lb"
  - "mobile -> lb"
  - "lb -> api"
  - "api -> pg"
  - "api -> redis"
  - "jobs -> pg"
  - "jobs -> files"
  - "api --> jobs: schedule"
```

![A layered diagram titled A SaaS product in four tiers. Browser and Mobile app sit at the top. An Edge band holds a CDN and a Load balancer; an Application band holds a highlighted API and Background jobs, with a dashed schedule arrow between them; a Data band holds PostgreSQL, Redis and Object storage. Arrows run down from each tier to the next.](images/layers-saas.svg)

## Network zones

Every node is in a band, and the caption states the rule the picture shows.

<!-- spec: layers-network -->
```yaml
version: 1
diagram: layers
title: Network zones
description: Traffic passes from the internet through the DMZ to the private and restricted zones.
caption: Each zone only talks to the one directly below it.
nodes:
  - { id: users, label: Users, kind: actor }
  - { id: waf, label: Firewall, kind: service }
  - { id: proxy, label: Reverse proxy, kind: service }
  - { id: app, label: App servers, kind: service }
  - { id: vault, label: Secrets vault, kind: datastore, emphasis: true }
  - { id: db, label: Customer data, kind: datastore }
groups:
  - { id: internet, label: Internet, members: [users] }
  - { id: dmz, label: DMZ, members: [waf, proxy] }
  - { id: private, label: Private, members: [app] }
  - { id: restricted, label: Restricted, members: [db, vault] }
edges:
  - "users -> waf: HTTPS"
  - "waf -> proxy"
  - "proxy -> app"
  - "app -> vault"
  - "app -> db"
```

![A layered diagram titled Network zones with four bands. Users in the Internet band reach a Firewall over HTTPS in the DMZ band, which passes to a Reverse proxy. The proxy calls App servers in the Private band, which use Customer data and a highlighted Secrets vault in the Restricted band. A caption says each zone only talks to the one directly below it.](images/layers-network.svg)

## Tips

- List the groups from top to bottom in the order you want the bands.
- Keep band labels short; a long label can collide with an arrow coming in from above, and SVGDog will tell you so.
- Arrows may go between nodes of the same band, as from the firewall to the proxy.
