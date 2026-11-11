# Running the tests

This page takes about five minutes: it builds SVGDog, runs every test once, and shows how to run a single test.

## Before you start

You need a Rust toolchain with `cargo` and `clippy` (the CI uses the stable channel). No network access, browser or other runtime is needed to run the tests; the fonts the tests draw with are compiled into the binary.

> **Tip.** On the maintainer's macOS setup, run `export PATH=/opt/homebrew/opt/rustup/bin:$PATH` first so that `cargo` is found.

## Run everything once

```bash
cargo test                                # unit, command-line and MCP tests (debug build)
cargo test --release r15                  # the timing budgets, which are strict only in release builds
cargo clippy --locked --all-targets -- -D warnings   # the lint gate CI also runs
```

On version 1.0.0, `cargo test` prints three result blocks, all passing:

| Block printed by cargo | Source | Tests |
|---|---|---|
| `Running unittests src/main.rs` | `#[cfg(test)]` modules inside `src/` | 21 |
| `Running tests/cli.rs` | `tests/cli.rs`, drives the real binary | 55 |
| `Running tests/mcp.rs` | `tests/mcp.rs`, drives `svgdog mcp` over stdio | 7 |

That is 83 tests. The `tests/cli.rs` block takes about nine seconds on a laptop in a debug build; the other two take well under a second. The release-mode command runs two tests (`r15_cold_start_is_fast` and `r15_a_200_kb_file_validates_quickly`) and finishes in under a second.

> **Note.** `cargo test --release r15` builds the project a second time in release mode, so the first run is slow even though the tests are fast.

## Run one test, or one group

`cargo test` accepts a substring of the test name. Add `-- --nocapture` to see what a test prints.

```bash
cargo test --test cli r4_exit_codes            # one test in tests/cli.rs
cargo test --test cli preview                  # every test with "preview" in its name
cargo test --test mcp                          # the whole MCP file
cargo test --bin svgdog ops::                  # unit tests in src/ops.rs
cargo test --test cli r8 -- --nocapture        # show output while it runs
```

Test names start with the requirement they check (`r1_`, `r4_`, `r8_`…) or describe the behaviour in words. To list names without running anything:

```bash
cargo test -- --list                           # every test, grouped by binary
```

## How the tests run the program

The tests in `tests/cli.rs` and `tests/mcp.rs` do not call Rust functions. They start the compiled `svgdog` binary (the `CARGO_BIN_EXE_svgdog` path that cargo provides), pass arguments, and parse the single JSON object printed on stdout. This is deliberate: the tests see what a user or an AI agent sees, including exit codes (`r4_exit_codes`) and the fact that no command waits for input (`r1_every_command_completes_without_stdin_or_terminal`).

Many tests copy a fixture from `tests/fixtures/` into a temporary directory (`tempfile::tempdir`) and run the binary there, so a test never changes a file in the repository. The only exception is the golden-file test, which writes into `tests/golden/` only when you ask it to ([Golden files](../adding/golden-files.md)).

## If a test fails

1. Re-run just that test with `--nocapture` and read the assertion message. Most messages print the whole JSON envelope.
2. If `r8_output_matches_the_golden_files` fails, read [Golden files](../adding/golden-files.md) before regenerating anything.
3. If only `r15_*` fails, you are probably on a loaded machine or in a debug build; see [Continuous integration gates](../gates/ci.md#timing-budgets).
4. If a test fails on one operating system in CI but not on yours, see [Continuous integration gates](../gates/ci.md#when-one-system-fails).
