# Part I: How it works

This part explains what SVGDog does between reading a file and writing a result. Read it once from top to bottom if you are going to operate SVGDog seriously; later parts quote its terms without redefining them.

| Chapter | What it tells you |
| --- | --- |
| [System overview](overview.md) | The three paths (create, check, change) and the modules behind each. |
| [From spec to validated spec](spec-pipeline.md) | How a YAML or JSON file becomes a checked, in-memory diagram, and which mistakes stop it. |
| [Layout and auto-fit](layout.md) | How boxes are placed on the grid, and what happens when a drawing is too wide. |
| [Edge routing](routing.md) | How connectors find their way between boxes. |
| [Determinism and output ids](determinism.md) | Why the output is byte-identical everywhere, and the ids you can rely on. |
| [The rule engine](rules-engine.md) | How the 38 rules run, how profiles and config change them, and how suppression works. |

Read the [Reference](../reference/README.md) when you need exact keys, limits and flags; this part is about reasons, not lists.
