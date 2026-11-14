# State gallery

Two state diagrams with the specs that draw them. Every node is a state; `kind: start` and `kind: end` mark where it begins and ends. For the rules, see [Diagram types](diagram-types.md#state).

## A connection, left to right

A state that loops on itself (`open -> open`), and arrows back to earlier states.

<!-- spec: state-connection -->
```yaml
version: 1
diagram: state
title: A network connection
description: A connection is opened, may drop and reconnect, and is finally closed.
nodes:
  - { id: begin, kind: start }
  - { id: idle, label: Idle }
  - { id: connecting, label: Connecting }
  - { id: open, label: Open, emphasis: true }
  - { id: retry, label: Retrying }
  - { id: closed, label: Closed }
  - { id: finish, kind: end }
edges:
  - "begin -> idle"
  - "idle -> connecting: connect"
  - "connecting -> open: ok"
  - "connecting -> idle: refused"
  - "open -> retry: dropped"
  - "retry -> connecting: backoff"
  - "open -> open: heartbeat"
  - "open -> closed: close"
  - "closed -> finish"
```

![A left-to-right state diagram titled A network connection. From the start dot the connection is Idle, goes to Connecting on connect and back to Idle when refused. On ok it becomes Open, which is highlighted and has a heartbeat loop. When dropped it goes to Retrying, which returns to Connecting after the backoff. On close it becomes Closed and then reaches the end marker.](images/state-connection.svg)

## A support ticket, top to bottom

`direction: down` suits a tall life cycle. A note explains a rule that is not an arrow.

<!-- spec: state-ticket -->
```yaml
version: 1
diagram: state
direction: down
title: A support ticket
description: A ticket is triaged, worked on, may wait for the customer, and is closed or reopened.
caption: A closed ticket can be reopened for seven days.
nodes:
  - { id: begin, kind: start }
  - { id: new, label: New }
  - { id: triaged, label: Triaged }
  - { id: progress, label: In progress, emphasis: true }
  - { id: waiting, label: Waiting for customer }
  - { id: solved, label: Solved }
  - { id: finish, kind: end }
edges:
  - "begin -> new"
  - "new -> triaged: assign"
  - "triaged -> progress: start"
  - "progress -> waiting: ask"
  - "waiting -> progress: reply"
  - "progress -> solved: fix"
  - "solved -> progress: reopen"
  - "solved -> finish: after 7 days"
notes:
  - { attach: waiting, text: Closes itself after 14 days }
```

![A top-to-bottom state diagram titled A support ticket. From the start dot the ticket is New, then Triaged on assign, then In progress on start, which is highlighted. It moves to Waiting for customer on ask and back on reply; a note says it closes itself after 14 days. On fix it becomes Solved, which can go back to In progress on reopen, or reach the end marker after 7 days. A caption says a closed ticket can be reopened for seven days.](images/state-ticket.svg)

## Tips

- Name states as conditions (`Open`, `Waiting for customer`) and arrows as events (`close`, `reply`).
- Keep arrow labels short. When two arrows run between the same pair of states, long labels may not fit.
