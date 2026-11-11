# Glossary

This glossary defines the testing words and the requirement numbers used in this guide.

## Words

**Approximate.** A flag (`"approximate": true`) on a finding whose geometry SVGDog measured on an SVG it did not draw. Such findings are capped at warning severity.

**Baseline.** See *golden file*.

**Envelope.** The single JSON object every command prints on stdout. It has `tool`, `version`, `command`, `ok`, and (for most commands) `summary`, `findings` and `outputs`. Errors use the same envelope with an `error` object holding a `code`. The envelope is validated against `schemas/*.json` by the test for R2.

**Exit code.** The process result: 0 success, 1 findings at or above the failing level, 2 usage error, 3 input error, 4 output error, 5 internal error. Tested by `r4_exit_codes`.

**Fixture.** An input file for tests, kept in `tests/fixtures/`.

**Finding.** One reported problem: a rule id, a severity (`error`, `warning`, `info`), a file, a line, a target selector, a message, and sometimes a box and a fix.

**Gate.** A check that must pass before a change is merged or a release is published. See [Continuous integration gates](../gates/ci.md) and [Release gates](../gates/release.md).

**Golden file.** A saved expected output in `tests/golden/`, compared byte for byte. See [Golden files](../adding/golden-files.md).

**Profile.** A named rule set chosen with `--profile`: `minimal`, `docs` (default) or `strict`.

**Rule.** One of the 38 checks in the catalogue, listed by `svgdog rules`. Each has a failing and a passing example that a test runs.

**Scene.** The geometry of every element in an SVG, as printed by `svgdog inspect`.

**Timing budget.** The upper limit enforced by the `r15` tests, strict only in release builds.

## Requirement numbers

The tests and CI comments refer to "the spec's agent requirements (R1 to R16)". **That specification is not stored in this repository.** The meanings below are inferred from the names and comments of the tests that carry each number. Treat them as a working summary, not as the source text.

| Number | Meaning as exercised by the tests | Test |
|---|---|---|
| R1 | every command finishes without stdin or a terminal | `r1_every_command_completes_without_stdin_or_terminal` |
| R2 | output validates against the published schemas | `r2_output_validates_against_the_published_schemas` |
| R3 | stdout is one JSON object, with and without `--quiet`, no ANSI colour | `r3_stdout_is_json_with_and_without_quiet_and_has_no_ansi` |
| R4 | defined exit codes | `r4_exit_codes` |
| R5 | applying the ops that a finding carries clears that finding | `r5_applying_a_findings_ops_clears_it` |
| R6 | help is short and has an example | `r6_help_is_short_and_has_an_example` |
| R7 | misspellings fail loudly with a suggestion | `r7_misspellings_fail_loudly_with_a_suggestion` |
| R8 | same input, config and version give identical bytes, on every system | `r8_same_input_gives_identical_bytes`, `r8_render_is_deterministic`, `r8_output_matches_the_golden_files` |
| R9 | running `fix` on fixed output changes nothing | `r9_fix_on_fixed_output_is_an_empty_diff` |
| R10 | output is bounded: findings are capped and counts stay complete | `r10_findings_are_capped_and_counts_stay_complete` |
| R11 | bytes outside an edited element are unchanged | `r11_bytes_outside_the_edited_element_are_unchanged` |
| R12 | `--dry-run` reports a diff and findings and writes nothing | `r12_dry_run_reports_a_diff_and_findings_and_writes_nothing` |
| R13 | `inspect` gives a box for every visible element; `preview` output is bounded in width | `r13_inspect_returns_a_bbox_for_every_visible_element`, `r13_preview_is_a_png_of_bounded_width` |
| R14 | `validate` and `inspect` write nothing | `r14_validate_and_inspect_write_nothing` |
| R15 | speed: cold start under 100 ms, a 200 KB file validates in under 1 s (release builds) | `r15_cold_start_is_fast`, `r15_a_200_kb_file_validates_quickly` |
| R16 | ids stay stable when a node is added | `r16_ids_survive_adding_a_node` |

> **Looking for more words?** Words about drawing diagrams are defined in the [user guide](../../user/), and words about how SVGDog works inside in the [operator guide](../../operator/). Each book has its own glossary.
