# Glossary

These are the words this guide uses, in plain language. The [Operator guide](../operator/) and the [Tester guide](../tester/) each have their own glossary for their more technical words.

| Word | Meaning |
|---|---|
| **annotate** | Draw a labelled outline around each problem on a preview picture (`preview --annotate`) |
| **async** | An arrow style for messages that do not wait for an answer; drawn as a dashed line |
| **band** | In a `layers` diagram, one full-width stripe that holds a group |
| **diagram type** | The kind of picture: `flow`, `layers`, `sequence`, `free`, `state`, `timeline` or `compare` |
| **dry run** | Showing what a command would change without changing anything (`--dry-run`) |
| **edge** | An arrow between two nodes |
| **emphasis** | Drawing a node with a stronger outline so it stands out |
| **exit code** | The number a command ends with: 0 means success, anything else means something needs attention |
| **finding** | One problem reported by a check, with a rule name, a place and a sentence |
| **fix** | Repairing findings automatically. A *safe* fix does not change how the picture looks; an *unsafe* fix can |
| **flow** | A diagram of how parts connect, arranged in ranks |
| **free** | A diagram where you place each node yourself with `at: [x, y]` |
| **grid** | The 8-pixel grid every box and arrow snaps to, so diagrams look tidy |
| **group** | A labelled frame drawn around related nodes |
| **id** | A short name for a node, group or edge, used to refer to it |
| **JSON** | A text format for data that programs read. SVGDog prints it by default when its output is not a terminal |
| **kind** | The shape and colour of a node: `box`, `service`, `client`, `datastore`, `queue`, `actor`, `external`, `decision` or `document` |
| **layers** | A diagram of stacked bands, such as tiers of an architecture |
| **lifeline** | In a `sequence` diagram, the vertical line that belongs to one participant |
| **MCP** | Model Context Protocol, the standard way for an AI assistant to call tools such as SVGDog |
| **node** | A box in the diagram |
| **note** | A short callout attached to a node, group or edge |
| **PNG** | A picture file format. `preview` makes PNG files |
| **preview** | Drawing an SVG into a PNG so you can look at it |
| **profile** | How strict the checks are: `minimal`, `docs` or `strict` |
| **render** | Turning a spec into an SVG, and checking the result |
| **rule** | One check SVGDog can run, such as `low-contrast`. There are 38 |
| **selector** | A short expression that names elements: `#id`, a tag such as `rect`, or `tag[attr="value"]` |
| **sequence** | A diagram of messages between participants in time order |
| **severity** | How serious a finding is: error, warning or info |
| **spec** | The text file that describes your diagram, in YAML or JSON |
| **SVG** | A picture format made of text instructions. It scales to any size without going blurry |
| **theme** | The look of a diagram: its colours, font, text sizes and line style. Built in are `docs`, `blueprint`, `paper` and `bold`; a folder with a `theme.toml` is one of your own |
| **validate** | Checking a file against the rules |
| **viewBox** | The part of the picture's own coordinate system that is shown. It lets an SVG scale to its page |
| **YAML** | A simple text format for lists and labelled values, used for specs |
