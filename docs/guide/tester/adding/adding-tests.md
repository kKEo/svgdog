# Adding tests and the pull-request checklist

This page tells you which kind of test to write for which kind of change, shows the helpers you can reuse, and ends with the checklist for a pull request.

## Choose the test

| You changed… | Write | Where |
|---|---|---|
| a pure helper (an edit operation, a selector, YAML patching, routing, diff text) | a unit test next to the code | the `#[cfg(test)] mod tests` at the bottom of the file in `src/` |
| what a command accepts, prints or returns, or its exit code | a command-line test | `tests/cli.rs`, in the section for that area ([What is tested where](../running/what-is-tested-where.md)) |
| the MCP server or a tool's arguments | an MCP test | `tests/mcp.rs` |
| layout, routing, themes or any byte of rendered output | no new test code; update the golden files | [Golden files](golden-files.md) |
| a lint rule | a catalogue entry with failing and passing examples | `src/catalogue.rs`, then see below |
| the spec format | a fixture plus a schema update | `tests/fixtures/`, `schemas/spec.json` |

## Reusable helpers in `tests/cli.rs`

- `run(&["validate", CLEAN])` runs the binary from the repository root and returns exit code, stdout and stderr; `.json()` parses stdout as one JSON object.
- `run_in(dir, args, stdin)` runs it in another directory, optionally with piped input.
- `CLEAN` is `tests/fixtures/orders.svg` and `BROKEN` is `tests/fixtures/broken.svg`.
- `copy_fixture(name)` copies a fixture into a temporary directory so the test can change it freely.
- `schema(name)` and `assert_valid(&validator, &doc)` check an output against a published schema (R2).
- `rules_of(&json)` lists the rule ids in a findings array.

A new command-line test usually looks like this:

```rust
#[test]
fn my_new_behaviour() {
    let r = run(&["validate", BROKEN, "--no-config", "--rules", "title-missing"]);
    assert_eq!(r.code, 1);
    assert_eq!(rules_of(&r.json()), vec!["title-missing"]);
}
```

Run it alone while you work:

```bash
cargo test --test cli my_new_behaviour -- --nocapture
```

## Rules of thumb

- Name the test after the behaviour, and start the name with `rN_` if it checks a numbered requirement ([glossary](../appendix/glossary.md)).
- Assert on the JSON envelope and the exit code, not on stderr text.
- Never write into the repository from a test. Use `tempfile::tempdir()`.
- Pass `--no-config` when the test must not pick up a `svgdog.toml` from a parent directory.
- Keep timing assertions out of ordinary tests; only the `r15_` tests measure time.

## Changes that force other changes

Several tests are written to fail when two things drift apart. When you change one, expect to change the other:

| If you… | These tests fail until you update… |
|---|---|
| add or remove a rule | `catalogue_has_38_rules_in_four_groups` (the total of 38 and the per-group counts 10 structure, 12 geometry, 10 portability, 6 accessibility) |
| add a rule | `every_rule_example_fails_and_passes_as_documented` needs the rule's `fail` and `pass` examples to behave as documented, and the passing example must not trip other default rules |
| change the shape of a JSON envelope | `r2_output_validates_against_the_published_schemas` needs the matching file in `schemas/` |
| add a CLI command or flag | `every_command_has_a_tool_and_every_tool_argument_is_a_real_flag` in `src/mcp.rs` needs a matching MCP tool entry |
| change rendered output | `r8_output_matches_the_golden_files` needs regenerated golden files |
| add a fix to a rule | the `FIXABLE` table in `tests/cli.rs`, used by `r5_applying_a_findings_ops_clears_it` |

## Pull-request checklist

Copy this into the pull request description and tick it.

- [ ] `cargo clippy --locked --all-targets -- -D warnings` is clean.
- [ ] `cargo test --locked` passes (83 tests on version 1.0.0, plus the ones you added).
- [ ] `cargo test --locked --release r15` passes.
- [ ] New behaviour has a test that fails without the change.
- [ ] Any golden-file change is intended, reviewed ([Golden files](golden-files.md)), and `./docs/sync-assets.sh` was run.
- [ ] If an output shape changed, the schema in `schemas/` and the tests agree.
- [ ] If a CLI flag or command changed, the MCP tool table in `src/mcp.rs` and the docs agree.
- [ ] `Cargo.lock` is committed if dependencies changed.
- [ ] Both CI jobs (`test (ubuntu-latest)`, `test (macos-latest)`) are green.
- [ ] You tried the change by hand ([Exploratory testing by hand](exploratory.md)).
