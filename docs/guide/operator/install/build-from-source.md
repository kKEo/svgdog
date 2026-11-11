# Build from source

Building from source is the only install path that works today. This page takes about five minutes on a machine that already has Rust, and covers the toolchain, the build, installing the result and checking it.

## Requirements

- **A Rust toolchain that supports edition 2024**, which means Rust 1.85 or newer. `Cargo.toml` declares `edition = "2024"` and no `rust-version`, so an older compiler fails with an edition error. The documentation was verified with rustc 1.99.1.
- **No system libraries.** The direct dependencies listed in `Cargo.toml` (`clap`, `resvg`, `roxmltree`, `rustybuzz`, `kurbo`, `serde_yaml_ng`, `yaml-rust2`, `toml` and a few more) are Rust crates, and the build on a stock macOS machine needed nothing else. Whether a minimal Linux image needs a linker package depends on the image.
- **Network access during the build only**, to fetch crates. Once built, SVGDog needs none.

If you installed Rust through Homebrew's `rustup`, which is keg-only, `cargo` is not on your `PATH` by default:

```bash
export PATH=/opt/homebrew/opt/rustup/bin:$PATH   # only for a Homebrew rustup install
cargo --version
```

## Build

```bash
git clone git@github.com:kKEo/svg-diagram-generator.git
cd svg-diagram-generator
cargo build --release --locked
```

`--locked` makes the build use the versions pinned in `Cargo.lock`, which is what CI does. The binary is `target/release/svgdog`. Windows is not built or tested by this project; the code has no platform-specific parts, so a build from source may work there, but nothing checks it. A clean release build took about a minute on a four-core machine.

## Install it

Either copy the binary to a directory on your `PATH`, or let cargo do it:

```bash
cargo install --path . --locked           # installs to ~/.cargo/bin
cargo install --path . --locked --root-path /opt/tools   # or choose a prefix: /opt/tools/bin/svgdog
```

Both were run for this guide; the second form installs to `<root>/bin/svgdog`.

## Check the install

```bash
svgdog --version
# svgdog 1.0.0

svgdog render tests/fixtures/orders.yaml -o /tmp/orders.svg --format text
# Wrote /tmp/orders.svg (3052 bytes).
#
# 1 file checked: 0 errors, 0 warnings, 0 info, 0 fixable

cmp /tmp/orders.svg tests/golden/orders.svg && echo "byte-identical to the golden file"
```

If the last line prints, your build produces the same bytes as the reference output. The files in `tests/` are only in the repository, not in the binary, so run these from a checkout.

## Run the tests

```bash
cargo test --locked                 # unit, command-line and MCP tests
cargo test --locked --release r15   # timing budgets, strict only in release builds
```

See the [tester guide](../../tester/) for what the tests cover and how to read a failure.

## Upgrade

Pull, rebuild, reinstall. Output can change between versions, so if you commit rendered SVGs, re-render and review the diff as part of the upgrade. See [Determinism and output ids](../how-it-works/determinism.md).

## Uninstall

Delete the binary, or run `cargo uninstall svgdog` if you used `cargo install`. SVGDog leaves no state behind; the only files it ever writes are the outputs you named.
