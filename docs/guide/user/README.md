# Welcome to SVGDog

SVGDog turns a short text description of a diagram into a clean, correct SVG picture, and checks any SVG picture for mistakes. It is one small program that runs on your own computer: no account, no network and no browser needed.

This guide is for people who want diagrams in their documentation or pages and do not want to draw them by hand: writers, developers, product people, and anyone who asks an AI assistant to make a diagram. It assumes you can open a terminal and type a command. You do not need to know SVG, Rust or anything about how SVGDog works inside.

## Where to go

| If you want to… | Read |
|---|---|
| Make your first diagram in ten minutes | [Quick start](quick-start.md) |
| Know which command does which job | [Everyday use: which command for which job](everyday-use.md) |
| Describe a diagram: boxes, arrows, groups, notes | [Writing a spec](writing-a-spec.md) |
| Pick the right kind of diagram | [Diagram types](diagram-types.md) |
| Find and fix problems in an SVG you already have | [Checking and fixing SVGs](checking-and-fixing.md) |
| See what a diagram looks like as a picture | [Looking at your diagram](looking-at-your-diagram.md) |
| Choose a look, set house rules for a project | [Themes and settings](themes-and-settings.md) |
| See every built-in theme on one diagram | [Theme gallery](theme-gallery.md) |
| See finished diagrams of every type to copy | [Galleries](galleries.md) |
| Use your own colours, font and line style | [Making your own theme](making-a-theme.md) |
| Let an AI assistant drive SVGDog | [Using SVGDog with an AI agent](using-with-an-ai-agent.md) |
| See what people use it for | [Use cases](use-cases.md) |
| Know what it will and will not do to your files | [Trust and privacy](trust-and-privacy.md) |
| Fix an error message | [FAQ and troubleshooting](faq.md) |
| Look up a word | [Glossary](glossary.md) |

## How SVGDog works in one paragraph

You write a small text file called a **spec**. It lists the boxes (nodes) and arrows (edges) of your diagram. You run `svgdog render`, which draws the diagram, places everything on a tidy grid, routes the arrows around the boxes, and then checks its own work. You get an `.svg` file you can put in a web page, a README or a document. Run the same spec twice and you get exactly the same picture.

> **Looking for something else?** If you install, configure or run SVGDog for a team, see the [Operator guide](../operator/). If you change SVGDog itself or want to know how it is tested, see the [Tester guide](../tester/).
