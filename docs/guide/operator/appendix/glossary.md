# Glossary

This glossary defines the words the operator guide uses, in alphabetical order. Terms in bold within a definition are defined elsewhere in the list.

**Approximate (finding).** A geometry **finding** on an SVG that SVGDog did not render. Its measurement rests on heuristics, so it carries `"approximate": true` and is capped at warning.

**Autofix.** A repair attached to a **finding** as a list of edit operations. It is `safe` (applied by `svgdog fix`) or `unsafe` (applied only with `--unsafe`).

**Auto-fit.** The step in rendering that tries three levels of tightness, roomiest first, to keep the drawing within **max_width**. See [Layout and auto-fit](../how-it-works/layout.md#auto-fit).

**Band.** In a `layers` diagram, a full-length frame stacked across the flow, made from a top-level group.

**Canonical form.** The way SVGDog writes a spec when it cannot patch the text in place. Comments and hand formatting are lost; the result reports `"formatting": "rewritten"`.

**Catalogue.** The list of 38 rules with their groups, severities, autofix levels and options (`src/catalogue.rs`). `svgdog rules` prints it.

**Diagram type.** One of `flow`, `layers`, `sequence` or `free`, chosen by the `diagram` key in a **spec**.

**Envelope.** The single JSON object every command prints, with `tool`, `version`, `command`, `ok` and either results or an `error`. See [Envelope and exit codes](../reference/envelope-exit-codes.md).

**Fail level.** The lowest **severity** that makes `validate` exit with code 1: `error` by default, or `warning` with `--fail-on warning`.

**Finding.** One violation of one rule, with a rule id, **severity**, file, line, target selector and message.

**Golden file.** An expected output stored in `tests/golden/` and compared byte for byte by the test suite. See the [tester guide](../../tester/).

**Grid.** The 8 px lattice on which boxes, ports and routes are placed.

**Group.** A frame around member nodes or other groups. In `layers` diagrams top-level groups become **bands**.

**Lifeline.** In a `sequence` diagram, the vertical line under a node's header along which messages are drawn.

**MCP (Model Context Protocol).** A protocol that lets an AI agent call tools. `svgdog mcp` serves SVGDog's commands as eight tools over **stdio**.

**max_width.** The widest the drawing may be, in px, 200 to 2000, default 960. Exceeding it after **auto-fit** is the `too-wide` error.

**Note.** A short text attached to a node, group or edge, drawn beside it with a dotted leader line.

**Port.** The point where an edge meets the side of a box. Several edges on one side are spaced along it.

**Profile.** A named set of starting rule severities: `minimal`, `docs` (the default) or `strict`.

**Root (MCP).** The directory given to `svgdog mcp --root`. Every path a tool receives must resolve inside it.

**Rule.** One named check, such as `low-contrast`. Each belongs to a group (structure, geometry, portability or accessibility).

**Scene.** SVGDog's resolved view of an SVG: parsed tree, styles, text measurements and an element box for each visible element. Rules, `inspect` and `edit` work on it.

**Severity.** `error`, `warning` or `info`.

**Spec.** The YAML or JSON description of a diagram that `render` compiles. The author states structure; SVGDog chooses coordinates.

**Stdio.** Standard input and output. The MCP transport: one JSON message per line.

**Suppression.** A comment `<!-- svgdog-disable rule-id: reason -->` that silences a rule for the next element. The reason is required.

**Target.** A selector naming an element, such as `#node-api` or `#a > text`. Used by `inspect --select`, `edit --set` and in **findings**.

**Theme.** A named set of colours for rendering: `docs-light` or `docs-dark`.

**Tuning level.** One of the three tightness settings **auto-fit** tries; they set where labels wrap and how much room edge labels get.
