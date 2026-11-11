# Release process

A release publishes a version of SVGDog as a GitHub release with binaries for five platforms and a checksum file. This page is for whoever cuts releases. It takes about ten minutes of your time plus the workflow run, and it needs push access to the repository.

> **Note.** `.github/workflows/release.yml` is not yet committed and no tag exists, so this process has never run end to end. What follows is what the workflow file specifies, and the README says the same. The CI workflow it depends on has not yet been seen passing on all three operating systems either.

## Before you start

- The tests pass on `master`. The release workflow runs the same checks again (it calls `ci.yml`), but a failure there blocks the release after you have already tagged.
- `version` in `Cargo.toml` is the version you want, and is committed. The workflow reads it with `cargo metadata` and requires the tag to be `v<version>` exactly.
- If output changed on purpose, the golden files in `tests/golden/` are regenerated and committed. See the [tester guide](../../tester/).
- Note anything that changes rendered output in the release notes; users who commit rendered SVGs will see diffs.

## Steps

```bash
# 1. set the version in Cargo.toml (for example 1.1.0) and commit it
git commit -am "Bump version to 1.1.0"

# 2. tag it, with the v prefix, and push the tag
git tag v1.1.0
git push origin v1.1.0
```

Pushing a tag matching `v[0-9]+.[0-9]+.[0-9]+*` starts the workflow. Do not push to `master` and tag in the same step unless the commit is already on the remote.

## What the workflow does

1. **check** runs `ci.yml` as a reusable workflow: `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked`, and `cargo test --locked --release r15` on Linux and macOS.
2. **version** reads the version from `Cargo.toml` and fails if the tag is not `v<that version>`.
3. **build** compiles `cargo build --locked --release` for four targets (Linux x86_64 and arm64 with musl, macOS Apple silicon and Intel) and packs each as `svgdog-<version>-<target>.tar.gz` containing the binary and `README.md`.
4. **publish** runs only for tags. It writes `SHA256SUMS` for every archive and creates the release with `gh release create ... --generate-notes --verify-tag`. A tag with a suffix, such as `v1.1.0-rc.1`, is published as a pre-release.

The jobs run in that order: build needs version, and publish needs check, version and build.

## Dry run without publishing

Run the workflow by hand from the Actions tab (`workflow_dispatch`). It performs the checks and builds all four binaries and uploads them as workflow artifacts, but the publish job does not run because there is no tag. Use this to test a change to the workflow itself, or to get binaries from a branch.

## After the release

- Download one archive and run `svgdog --version`; it should print the new version.
- Check that `SHA256SUMS` lists all five archives.
- Update any pinned version and checksum in your pipelines (see [Release binaries](../install/release-binaries.md#pinning-in-ci)).

## If something goes wrong

| Problem | What to do |
| --- | --- |
| The `version` job fails with "Tag ... does not match" | Delete the tag (`git push --delete origin v1.1.0`, `git tag -d v1.1.0`), fix `Cargo.toml`, commit, tag again |
| A platform's build fails | Fix forward and cut the next patch version. Do not move a published tag |
| The published release has a mistake | Edit the release notes in GitHub. For a bad binary, publish a new patch version |
| You tagged by mistake | Delete the tag before the workflow publishes; once the release exists, treat it as published |

Deleting a tag that has a published release leaves a release with no tag and confuses anyone who pinned to it. Prefer a new version.
