# Edge routing

After boxes are placed, SVGDog draws each connector as a polyline of horizontal and vertical segments. This chapter explains how it chooses which side of a box an edge uses, where on that side it attaches, and how the path between two attachments is found. Knowing this helps you read why an edge bends where it does and what to change in the spec when a route looks wrong. The code is in `src/render/mod.rs` (`arrange`, `sides`) and `src/render/route.rs`.

## Step 1: choose the sides

For each edge (other than a self-loop) `sides` (`src/render/mod.rs:132-150`) picks a side of the source box and a side of the target box:

- If the two boxes are separated along only one axis, that axis is used.
- If they are separated along both, the diagram type decides: a **flow** keeps to its direction, **layers** connect across the flow (between bands), and **free** and **sequence** take the wider gap.
- If they overlap on both axes, the flow direction is used.

## Step 2: share each side among its edges

Several edges may leave or enter the same side of a box. Each side's length is shared among them (ports are spaced 16 px apart when they fit, 8 px if they must, and all at the middle if not). A port aims straight at the other end, so two boxes that line up get a straight edge; ports that want the same spot are spread along the side and ordered to avoid crossings. This is done in two passes: the second aims at the ports the first one chose. Ports are snapped to the grid.

A connector that starts and ends on the same box is a self-loop. It is drawn round the top right corner, and the box keeps that corner free for it.

## Step 3: find the path

Between the two attachment points (each pushed out 16 px as a stub), `Grid::route` runs an A* search over a uniform 8 px grid (`src/render/route.rs`). The search moves only straight ahead or turns 90 degrees, and it minimises a cost made of:

| Cost | Value | Effect |
| --- | --- | --- |
| Step | 10 | Shorter paths win |
| Bend | 45 | Fewer turns |
| Cross another route | 30 | Crossings are allowed but discouraged |
| Share a track with another route | 400 | Edges avoid running on top of each other |
| Run along a group frame | 60 | Edges cross a frame at a right angle instead of hugging it |

Boxes (with an 8 px margin) and notes are obstacles the path cannot enter. Frames that hold neither end of the edge are treated as walls for that edge; if that leaves no path, the search is repeated without the walls. Each finished route is marked as taken so later routes keep clear. Routes are found in spec order, so the order of `edges:` in the spec can change which of two competing edges gets the straighter path.

Two clean-ups follow: collinear points are removed, and a route with a single jog (four points) has the jog moved to the half-way point when that is free.

## Step 4: land on the outline

For non-rectangular shapes (a decision diamond, a datastore cylinder, a queue) the endpoint is slid inward until it meets the shape's outline, so arrows touch the drawn edge rather than the bounding box (`land`, `src/render/mod.rs:98`).

## Edge labels

A label is placed near the middle of its route, in the gap it sits in, with room reserved by the tuning level (see [auto-fit](layout.md#auto-fit)). Labels that would sit on a connector are the subject of the `label-on-line` rule.

## What you can change from the spec

You cannot draw a route by hand; you can only change the inputs the router sees. The levers, from the description above, are:

- **Edge order.** Routes are found in the order of `edges:`, and an earlier route is avoided by later ones, so reordering changes which edge gets the straighter path.
- **Direction.** `direction: down` changes which sides are used for a flow and which gap is the long one.
- **Box width.** A wider `width` on a box gives its sides more room for ports (see step 2).
- **Group membership.** Frames that hold neither end of an edge are walls for it, so moving a box into or out of a group changes what the edge must go round.

Try one change at a time and re-render; the result is deterministic, so a diff shows exactly what moved.
