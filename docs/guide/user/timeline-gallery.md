# Timeline gallery

Two timelines with the specs that draw them. Nodes go along the line in the order you list them, each with its `when`. For the rules, see [Diagram types](diagram-types.md#timeline).

## A release plan in phases

Groups become phases. When the line reaches `max_width` it continues on a new row, and a phase is never split.

<!-- spec: timeline-release -->
```yaml
version: 1
diagram: timeline
title: Version 2.0 release plan
description: The release goes through design, build and launch phases over six months.
nodes:
  - { id: kickoff, label: Kick-off, when: 6 Jan }
  - { id: rfc, label: Design approved, when: 3 Feb }
  - { id: alpha, label: Alpha, when: 10 Mar }
  - { id: beta, label: Public beta, when: 21 Apr, emphasis: true }
  - { id: rc, label: Release candidate, when: 26 May }
  - { id: ga, label: General availability, when: 16 Jun }
groups:
  - { id: design, label: Design, members: [kickoff, rfc] }
  - { id: build, label: Build, members: [alpha, beta] }
  - { id: launch, label: Launch, members: [rc, ga] }
```

![A timeline titled Version 2.0 release plan. A Design phase holds Kick-off on 6 January and Design approved on 3 February. A Build phase holds Alpha on 10 March and the highlighted Public beta on 21 April. On a second row a Launch phase holds Release candidate on 26 May and General availability on 16 June.](images/timeline-release.svg)

## A history, top to bottom

`direction: down` puts the dates on the left of a vertical line, which suits long labels.

<!-- spec: timeline-history -->
```yaml
version: 1
diagram: timeline
direction: down
title: How our architecture grew
description: From a single server to regions in three continents, one decision at a time.
caption: Each move was made when the previous setup hit a limit.
nodes:
  - { id: one, label: One server running everything, when: "2019" }
  - { id: split, label: Database moved to its own machine, when: "2020" }
  - { id: cloud, label: Moved to the cloud, when: "2021" }
  - { id: k8s, label: Services on Kubernetes, when: "2023", emphasis: true }
  - { id: regions, label: Second and third region, when: "2025" }
```

![A vertical timeline titled How our architecture grew. 2019: One server running everything. 2020: Database moved to its own machine. 2021: Moved to the cloud. 2023: Services on Kubernetes, highlighted. 2025: Second and third region. A caption says each move was made when the previous setup hit a limit.](images/timeline-history.svg)

## Tips

- A timeline has no arrows and no notes: the order is the connection. Put extra words in the label, a caption or `steps`.
- Quote a `when` that is only a number, such as `"2023"`, so it stays text.
