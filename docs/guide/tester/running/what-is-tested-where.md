# What is tested where

This page maps each area of SVGDog to the test file and test names that cover it, so you can find the tests for a change, or notice that there are none.

## The three test layers

| Layer | Location | What it checks | How |
|---|---|---|---|
| Unit | `src/ops.rs`, `src/yaml_patch.rs`, `src/preview.rs`, `src/diff.rs`, `src/mcp.rs`, `src/render/route.rs`, `src/svg/selector.rs` | small pure pieces: edit operations, YAML patching, selectors, routing, diff text, base64 | `#[cfg(test)]` modules, 21 tests |
| Command line | `tests/cli.rs` | the whole binary: every command, exit codes, JSON shapes, determinism | spawns the binary, 55 tests |
| MCP | `tests/mcp.rs` | the stdio server as a client would drive it | spawns `svgdog mcp --root-path DIR`, 7 tests |

The unit tests per file are: `ops.rs` 7, `preview.rs` 3, `yaml_patch.rs` 3, `diff.rs` 2, `mcp.rs` 2, `render/route.rs` 2, `svg/selector.rs` 2.

## `tests/cli.rs` by area

The file is split into commented sections. Section names follow the requirement numbers (R1 to R16, see [the glossary](../appendix/glossary.md)) and the build phase in which the feature arrived.

| Section in `tests/cli.rs` | Tests | Examples |
|---|---|---|
| Catalogue | 5 | `catalogue_has_38_rules_in_four_groups`, `every_rule_example_fails_and_passes_as_documented`, `broken_diagram_reports_expected_rules` |
| R1, R2, R3: input and output | 4 | `r2_output_validates_against_the_published_schemas`, `r3_stdout_is_json_with_and_without_quiet_and_has_no_ansi` |
| R4: exit codes | 2 | `r4_exit_codes` |
| R6: self-description | 1 | `r6_help_is_short_and_has_an_example` |
| R7: strict input | 1 | `r7_misspellings_fail_loudly_with_a_suggestion` |
| R8: determinism | 1 | `r8_same_input_gives_identical_bytes` |
| R10: bounded output | 2 | `r10_findings_are_capped_and_counts_stay_complete` |
| R13: inspect geometry | 2 | `r13_inspect_returns_a_bbox_for_every_visible_element` |
| Config, profiles, precedence | 6 | `precedence_flags_then_suppressions_then_overrides_then_rules_then_profile`, `suppression_needs_a_reason_and_silences_one_element` |
| R14, R15: side effects and speed | 2 | `r14_validate_and_inspect_write_nothing`, `r15_a_200_kb_file_validates_quickly` |
| Phase 2: edit and fix | 9 | `r5_applying_a_findings_ops_clears_it`, `r9_fix_on_fixed_output_is_an_empty_diff`, `r11_bytes_outside_the_edited_element_are_unchanged`, `r12_dry_run_reports_a_diff_and_findings_and_writes_nothing` |
| Phase 3: render | 11 | `every_fixture_and_theme_renders_without_errors_or_warnings`, `r16_ids_survive_adding_a_node`, `spec_errors_name_the_path_and_suggest_a_fix`, `edit_changes_a_spec_and_reports_on_the_drawing` |
| Phase 4: preview | 5 | `r13_preview_is_a_png_of_bounded_width`, `preview_background_annotate_and_crop` |
| Phase 5: cross-platform and speed | 4 | `r8_output_matches_the_golden_files`, `r15_cold_start_is_fast`, `sequence_diagrams_put_lifelines_in_order_and_messages_top_to_bottom`, `spec_edits_keep_yaml_comments` |

The section counts add up to 55.

## `tests/mcp.rs`

| Test | What it proves |
|---|---|
| `handshake_lists_one_tool_per_command` | `initialize` and `tools/list` work and list the tools |
| `tools_return_the_same_envelope_as_the_cli` | a tool call returns the same JSON envelope as the CLI, in text and structured content |
| `preview_returns_the_png_as_image_content` | `svgdog_preview` without `output` returns the PNG as image content |
| `paths_outside_the_root_are_refused` | `..` and symlinks cannot escape `--root-path` |
| `bad_calls_fail_clearly_and_the_server_keeps_going` | a bad call is an error result, not a crash |
| `per_request_protocol_version_is_served_without_a_handshake` | the per-request protocol style works |
| `a_missing_root_is_an_input_error` | a missing `--root-path` directory exits with the input-error code |

One more MCP check lives in `src/mcp.rs`: `every_command_has_a_tool_and_every_tool_argument_is_a_real_flag` fails when a CLI command has no tool or a tool names a flag that does not exist.

## Fixtures and golden files

| Path | Role |
|---|---|
| `tests/fixtures/orders.yaml`, `checkout.yaml`, `login.yaml`, `tiers.yaml`, `rack.yaml` | one spec per diagram type (flow, flow with every node kind, sequence, layers, free) |
| `tests/fixtures/orders.svg` | a clean SVG that must validate with no errors or warnings |
| `tests/fixtures/broken.svg` | a deliberately bad SVG that triggers 18 different rules (the test asserts 17 of them by name) |
| `tests/golden/*` | expected outputs, compared byte for byte; see [Golden files](../adding/golden-files.md) |
| `schemas/*.json` | the JSON Schemas that every output must satisfy (R2) |

## Gaps to know about

- The requirement document that defines R1 to R16 is cited by the tests but is not stored in this repository. The numbers in this guide are taken from test names and comments.
- R2 checks outputs against the schemas with the `jsonschema` crate, so a schema change and an output change must land together.
- Nothing in the repository measures code coverage.
