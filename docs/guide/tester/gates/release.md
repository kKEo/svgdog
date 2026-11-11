# Release gates

This page describes the checks in `.github/workflows/release.yml` that run before SVGDog binaries are published, and how to verify a release.

> **Note.** At the time of writing `release.yml` is new and no `v*` tag exists in the repository, so the `publish` job has not run yet. Treat the description below as the intended behaviour and confirm it on the first real tag (use a pre-release tag such as `v1.0.1-rc.1` first).

## How a release is triggered

| Trigger | What happens |
|---|---|
| Push a tag matching `v[0-9]+.[0-9]+.[0-9]+*` (for example `v1.0.0`) | all jobs run, and `publish` creates a GitHub release |
| Run the workflow by hand (`workflow_dispatch`) | the checks and builds run, but nothing is published |

Use the manual run to rehearse a release: it builds the same five binaries and uploads them as workflow artifacts.

## The gates, in order

| Job | Needs | What it proves |
|---|---|---|
| `check` | nothing | runs `ci.yml` as a called workflow, so the full [CI gates](ci.md) pass on all three systems |
| `version` | nothing | for a tag, the tag name equals `v` plus the version in `Cargo.toml` |
| `build` | `version` | the project builds in release mode for five targets and the package is created |
| `publish` | `check`, `version`, `build` | runs only for a tag; writes `SHA256SUMS` and creates the release |

`build` does not wait for `check`, so binaries may build while tests run. Nothing is published unless `check` also passed, because `publish` needs all three.

### Version gate

The `version` job reads the version with `cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version'`. If the run is for a tag and the tag differs from `v<version>`, it fails with:

```text
Tag v1.0.1 does not match the version in Cargo.toml (1.0.0).
```

To fix it, change `version` in `Cargo.toml` (and commit the updated `Cargo.lock`), or delete and re-create the tag.

### Build matrix

| Target | Runner |
|---|---|
| `x86_64-unknown-linux-musl` | `ubuntu-latest` |
| `aarch64-unknown-linux-musl` | `ubuntu-24.04-arm` |
| `aarch64-apple-darwin` | `macos-latest` |
| `x86_64-apple-darwin` | `macos-latest` |
| `x86_64-pc-windows-msvc` | `windows-latest` |

Each build runs `cargo build --locked --release --target <target>` and packages a folder named `svgdog-<version>-<target>` holding the binary and `README.md`. The package is a `.tar.gz`, or a `.zip` on Windows. The workflow does not run the tests against the cross-built binaries; the tests run once, on native builds, in `check`.

### Checksums and pre-releases

The `publish` job downloads all packages, writes `SHA256SUMS` with `sha256sum svgdog-* > SHA256SUMS`, and runs `gh release create` with `--generate-notes --verify-tag`. A tag that contains a hyphen (`v1.1.0-rc.1`) is published as a pre-release.

## Verify a published release by hand

```bash
# download the archive for your system and SHA256SUMS from the release page, then:
sha256sum --check --ignore-missing SHA256SUMS      # on macOS: shasum -a 256 -c SHA256SUMS
tar -xzf svgdog-1.0.0-x86_64-unknown-linux-musl.tar.gz
./svgdog-1.0.0-x86_64-unknown-linux-musl/svgdog --version     # prints: svgdog 1.0.0
```

Then run a smoke test with a fixture from the repository:

```bash
./svgdog-1.0.0-x86_64-unknown-linux-musl/svgdog render tests/fixtures/orders.yaml -o /tmp/orders.svg
echo $?     # 0 means no findings at or above the failing level
```

## Release checklist

- [ ] `cargo test --locked` and `cargo test --locked --release r15` pass on `master`.
- [ ] `version` in `Cargo.toml` is the number you are about to tag, and `Cargo.lock` is committed.
- [ ] Golden files are current (`cargo test r8_output_matches` passes without `SVGDOG_UPDATE_GOLDEN`).
- [ ] You rehearsed with a manual run or a pre-release tag.
- [ ] After publishing, `SHA256SUMS` verifies and `svgdog --version` prints the tagged version.
