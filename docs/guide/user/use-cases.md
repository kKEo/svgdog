# Use cases

This page describes five ways people use SVGDog, so you can decide whether it fits your work. Each one names the commands involved and links to the page that explains them.

## 1. Architecture pictures in a README

**Situation.** Your repository's README or wiki needs an up-to-date picture of how the services connect. Hand-drawn pictures go stale because nobody wants to open the drawing tool.

**With SVGDog.** Keep a small `architecture.yaml` next to the README. When the system changes, edit one line and run `svgdog render architecture.yaml -o architecture.svg --force`. Because the output is identical for identical input, the change in version control shows exactly what moved.

Read: [Writing a spec](writing-a-spec.md), [Diagram types](diagram-types.md).

## 2. A diagram check in your documentation build

**Situation.** Many people add SVG pictures to a documentation site, and some of them are unreadable in dark mode, too wide for a phone, or have no text description for screen readers.

**With SVGDog.** Run `svgdog validate "docs/**/*.svg"` in your build. It exits with 1 when any picture has an error, and with `--fail-on warning` it can fail on warnings too. Pictures drawn in other tools are checked as well.

Read: [Checking and fixing SVGs](checking-and-fixing.md), [Themes and settings](themes-and-settings.md).

## 3. Cleaning up a picture exported from a drawing tool

**Situation.** An SVG exported from a drawing tool carries editor clutter, a fixed width and height so it cannot scale, unused definitions, and sometimes script attributes that web platforms reject.

**With SVGDog.** `svgdog fix picture.svg --in-place` removes the clutter and makes it scale. The example file in the repository goes from 7 errors and 11 warnings to a short list of the problems only a person can solve (a missing title, for example).

Read: [Checking and fixing SVGs](checking-and-fixing.md).

## 4. An assistant that draws and checks its own diagrams

**Situation.** You ask an AI assistant for a diagram and get an SVG that looks plausible but has overlapping labels or arrows through boxes.

**With SVGDog.** The assistant writes a spec instead of raw SVG, renders it, reads any findings, repairs and looks at a preview. The result is checked before you see it.

Read: [Using SVGDog with an AI agent](using-with-an-ai-agent.md).

## 5. Sequence diagrams for an API or a login flow

**Situation.** You need to document who talks to whom and in what order, and want it to stay editable as text.

**With SVGDog.** Write a `sequence` spec with the participants as nodes and the messages as edges in order. Add notes for important details such as "valid for ten minutes".

Read: [Diagram types](diagram-types.md).

## What SVGDog is not for

- **Pictures that are not diagrams.** It draws boxes, arrows, frames and notes, not charts, maps or illustrations.
- **Hand-tuned artwork.** SVGDog decides the layout. Only the `free` type lets you place boxes yourself, and arrows are still routed for you.
- **Live editing.** `preview` writes a PNG when you ask; it does not refresh a window as you type.
