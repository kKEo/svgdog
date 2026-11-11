# Trust and privacy

This page lists what SVGDog does and does not do with your files, so you can decide whether it is safe to use on your documents. It takes about five minutes.

## It works on your computer only

- **No network.** SVGDog does not need an internet connection to run, and nothing in the commands described in this guide sends your specs, pictures or settings anywhere. The program needs no browser and no other language runtime.
- **No account, no tracking.** There is nothing to sign in to.
- **Fonts are inside the program.** The two fonts SVGDog uses (Inter and JetBrains Mono, both under the SIL Open Font License) are built in. Measuring text and making preview pictures never depends on the fonts installed on your computer, so a preview looks the same everywhere. A saved SVG names Inter and falls back to the viewer's sans-serif font if Inter is missing; use `--text-as-paths` (see [Themes and settings](themes-and-settings.md)) when you need exact letter shapes everywhere.

## It does not overwrite your work by surprise

- **`render` and `preview` refuse to replace an existing file** unless you add `--force`. They exit with code 4 instead.
- **`edit` and `fix` need you to choose where the result goes**: `-o FILE`, `--in-place`, or `--dry-run` to write nothing. They never decide for you.
- **Writes are all-or-nothing.** SVGDog writes the result to a temporary file and then moves it into place, so a crash cannot leave you with half a file.
- **`--dry-run` shows the exact change first.**

## Pictures it draws are safe to publish

SVGDog output is plain, static SVG: no scripts, no links to other files and no embedded web content. The checks go further and flag anything that could make a picture unsafe or unportable in files from other tools: script attributes such as `onclick`, `<foreignObject>` content, and references to resources outside the file. Running `svgdog validate` before you publish pictures from other sources is a cheap safety net.

## It stays within one folder when an assistant drives it

When you connect an AI assistant with `svgdog mcp --root DIR`, every file path the assistant uses must be inside `DIR`. Paths that climb out of it with `..`, or that leave it through a symbolic link, are refused. See [Using SVGDog with an AI agent](using-with-an-ai-agent.md).

## What SVGDog does not promise

- **Findings are advice.** Checks on pictures that SVGDog did not draw rest on estimates and are marked `approximate`.
- **`fix --unsafe` can change how a picture looks.** Use `--dry-run` first.
- **Your assistant is a separate program.** SVGDog protects the folder you give it, but what your assistant does with the files it reads is up to that assistant.

For the technical side of the same guarantees, see the [Operator guide](../operator/).
