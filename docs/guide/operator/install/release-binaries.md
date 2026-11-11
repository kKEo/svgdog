# Release binaries

The repository has a workflow that builds SVGDog for five platforms and attaches the archives to a GitHub release when a version tag is pushed. This page describes what a release will contain and how to use it. Prebuilt binaries become available from the first `v*` tag; until one is published, [build from source](build-from-source.md).

> **Note.** At the time of writing the repository has no tags, and `.github/workflows/release.yml` is not yet committed, so no release exists. Everything below describes what the workflow is written to produce. Treat it as unverified until the first release is out.

## What a release contains

The workflow (`.github/workflows/release.yml`) builds these targets and names each archive `svgdog-<version>-<target>`:

| Target | Built on | Archive |
| --- | --- | --- |
| `x86_64-unknown-linux-musl` | ubuntu-latest | `.tar.gz` |
| `aarch64-unknown-linux-musl` | ubuntu-24.04-arm | `.tar.gz` |
| `aarch64-apple-darwin` | macos-latest | `.tar.gz` |
| `x86_64-apple-darwin` | macos-latest | `.tar.gz` |
| `x86_64-pc-windows-msvc` | windows-latest | `.zip` |

Each archive unpacks to a folder `svgdog-<version>-<target>/` holding the binary (`svgdog`, or `svgdog.exe`) and `README.md`. The Linux binaries are linked statically with musl, so they run on any distribution without installing libraries. The release also carries a `SHA256SUMS` file listing the checksum of every archive.

The binaries are not signed or notarised; the workflow has no signing step.

## Download and verify

Once a release exists, for version `1.0.0` on Apple silicon:

```bash
gh release download v1.0.0 --repo kKEo/svg-diagram-generator \
  --pattern 'svgdog-1.0.0-aarch64-apple-darwin.tar.gz' --pattern SHA256SUMS

shasum -a 256 -c SHA256SUMS --ignore-missing    # macOS; on Linux: sha256sum -c SHA256SUMS --ignore-missing
tar -xzf svgdog-1.0.0-aarch64-apple-darwin.tar.gz
install -m 755 svgdog-1.0.0-aarch64-apple-darwin/svgdog /usr/local/bin/svgdog
svgdog --version
```

On Windows, extract the `.zip` and put `svgdog.exe` on your `PATH`.

> **Warning.** A macOS binary downloaded with a browser may be quarantined by Gatekeeper because it is not notarised. Downloading with `gh` or `curl` does not set the quarantine flag. If you must use a browser download, remove the flag with `xattr -d com.apple.quarantine <file>`.

## Pre-releases

A tag with a suffix, such as `v1.1.0-rc.1`, is published as a GitHub pre-release. The tag pattern is `v[0-9]+.[0-9]+.[0-9]+*`.

## Pinning in CI

Pin an exact version and check its checksum, so a pipeline never changes behaviour without a commit:

```bash
VERSION=1.0.0
TARGET=x86_64-unknown-linux-musl
curl -fsSL -o svgdog.tar.gz \
  "https://github.com/kKEo/svg-diagram-generator/releases/download/v${VERSION}/svgdog-${VERSION}-${TARGET}.tar.gz"
echo "<expected sha256>  svgdog.tar.gz" | sha256sum -c -
tar -xzf svgdog.tar.gz --strip-components=1 "svgdog-${VERSION}-${TARGET}/svgdog"
```

Take the expected checksum from the release's `SHA256SUMS` once and commit it. The release-asset URL pattern is GitHub's standard form and has not been exercised for this repository, since no release exists yet; if the repository is private, use `gh release download` with a token instead.
