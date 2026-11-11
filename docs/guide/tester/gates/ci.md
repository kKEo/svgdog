# Continuous integration gates

This page lists every check in `.github/workflows/ci.yml`, what each one proves, and how to reproduce a failure on your own machine.

## When it runs

The `ci` workflow runs on every push to `master`, on every pull request, on demand (`workflow_dispatch`), and when another workflow calls it (`workflow_call`; the release workflow does this, see [Release gates](release.md)). A newer push to the same branch or pull request cancels the run in progress (`concurrency` with `cancel-in-progress: true`). The workflow has read-only access to the repository (`permissions: contents: read`).

## The gates

There is one job, `test`, run on two systems with `fail-fast: false`, so a failure on one system does not hide the result on the others. Each run has a 30 minute timeout.

| Job name | Runs on |
|---|---|
| `test (ubuntu-latest)` | Linux |
| `test (macos-latest)` | macOS |

Each job runs these steps in order. A failing step fails the job.

| Step | Command | What it proves |
|---|---|---|
| Lint | `cargo clippy --locked --all-targets -- -D warnings` | no clippy warning anywhere, including in tests |
| Test | `cargo test --locked` | all 83 unit, command-line and MCP tests pass |
| Timing budgets | `cargo test --locked --release r15` | speed budgets hold in a release build |

`--locked` makes cargo fail if `Cargo.lock` would change, so a dependency change must be committed together with its lock file.

Every job also checks out the code, installs the stable Rust toolchain with the `clippy` component, and restores a cache (`Swatinem/rust-cache`).

> **Note.** This repository's files do not show whether GitHub branch protection requires these three jobs before a merge. Check **Settings → Branches** on GitHub; the guide cannot verify it.

## Reproduce the gates locally

```bash
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --release r15
```

If all three pass, the Linux job should pass. See below for macOS.

## Why two systems

The golden-file test `r8_output_matches_the_golden_files` in `tests/cli.rs` compares the bytes of rendered SVGs, scene JSON and a PNG with the files in `tests/golden/`. Those files were produced on one machine. A pass on both Linux (x86_64) and macOS (Apple silicon) is therefore the evidence for requirement R8: render, validate, inspect and preview give the same bytes on different systems and processors. Windows is not built or tested, so nothing is claimed for it. A comment above the matrix in `ci.yml` says the same.

`.gitattributes` supports this: it forces LF line endings for `*.svg`, `*.yaml` and `*.json` and marks `*.png` and `*.ttf` as binary, so a checkout that translates line endings does not change the bytes that are compared.

## When one system fails

1. Read the failing test name in the job log. If it is `r8_output_matches_the_golden_files`, the message names the file and both sizes: `{name} differs from tests/golden/{name} ({n} vs {m} bytes)`.
2. If only one system differs, do not regenerate the golden files. A difference between systems is a real portability bug (floating-point formatting, line endings, font measuring, path separators). Fix the cause.
3. If both systems differ, the change is probably intended. Follow [Golden files](../adding/golden-files.md).
4. Check that no file in the diff was committed with CRLF line endings.

## Timing budgets

Two tests in `tests/cli.rs` carry the `r15` prefix and are the only timing checks:

| Test | Budget in release builds | Budget in debug builds |
|---|---|---|
| `r15_cold_start_is_fast` | best of 5 runs of `svgdog --version` under 0.1 s | 1.0 s |
| `r15_a_200_kb_file_validates_quickly` | best of 3 runs of `svgdog validate` on a generated 200 KB SVG under 1 s | 20 s |

The best run counts, because the first start of a fresh binary pays a one-off operating-system cost. That is why the CI step uses `--release`: the debug budgets are deliberately loose, so only the release run is a real gate. On a developer laptop both tests finish in under a second together.

If a timing test fails once on CI and passes on re-run, suspect a loaded runner. If it fails every time, something got slower: profile `svgdog validate` on the generated file.
