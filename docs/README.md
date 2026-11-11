# SVGDog documentation

The documentation is one landing page and three books, one per reader. Each book is a
GitBook space (synced from this repository, see `gitbook-docs.yaml`) and an mdBook twin that
the Pages workflow builds.

| Start here | For | What it holds |
|---|---|---|
| [Landing page](guide/index.html) | everyone | One card per book and a link to the deck |
| [TL;DR deck](guide/tldr.html) | everyone | Ten slides: what SVGDog is and the 30-second loop |
| [User guide](guide/user/README.md) | people who write diagrams | Quick start, spec writing, checking, preview, FAQ |
| [Operator guide](guide/operator/README.md) | people who install and run it | How it works, install, full reference, CI recipes |
| [Tester guide](guide/tester/README.md) | people who change it | Running tests, CI gates, golden files, adding tests |

## Working on the docs

```bash
make docs-check          # links and coverage against the code
make docs                # check, then build all books into site/
make docs-serve BOOK=user  # serve one book with live reload
./docs/sync-assets.sh    # refresh diagram images and rebuild the deck (docs/deck/)
./docs/gen-rules.sh      # regenerate the rules catalogue page
```

Pictures come from `tests/golden/`, and the rules catalogue is generated from
`svgdog rules`, so neither can drift from the renderer.
