# SVGDog tester guide

This guide shows how every change to SVGDog is checked: how to run the tests, what blocks a merge or a release, how to add a test, and how to look for problems by hand.

This guide is for anyone who reviews or changes SVGDog code: testers, contributors and release managers. It assumes you can use a terminal, run `git`, and read a short Rust test. It does not assume you know the code base.

## Reading map

| Part | Read it when you need to… |
|---|---|
| [Part I: Running the tests](running/README.md) | run everything once, or find the tests that cover a feature |
| [Part II: Gates](gates/README.md) | know what blocks a merge or a release, and why a CI job failed |
| [Part III: Adding tests](adding/README.md) | add a test, update golden files, or explore a change by hand |
| [Appendix](appendix/README.md) | look up a word such as *golden*, *envelope* or *R8* |

## Conventions

- Commands are shown in `bash` blocks and are run from the repository root unless a page says otherwise.
- File paths are relative to the repository root, for example `tests/cli.rs`.
- Test counts and timings in this guide were measured on version 1.0.0 (83 tests). Recount with `cargo test` before you quote them.
- "R1" to "R16" are the requirement numbers used in test names and CI comments. See [the glossary](appendix/glossary.md) for what they mean and for one important caveat about them.
- **Tip.** On the maintainer's macOS setup, `cargo` comes from Homebrew's keg-only rustup and is not on the default PATH. If `cargo: command not found`, run `export PATH=/opt/homebrew/opt/rustup/bin:$PATH` first. Other machines do not need this.

> **Looking for something else?** If you only want to draw and check diagrams, read the [user guide](../user/). If you install, configure or run SVGDog in your own pipeline, read the [operator guide](../operator/).
