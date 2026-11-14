#!/usr/bin/env bash
# Regenerates the gallery pages of the user guide from their specs.
#
# The specs live in tests/fixtures/gallery/<name>.yaml (extra theme variants in themes.txt).
# A page shows a spec by putting a marker right before a YAML code block:
#
#   <!-- spec: sequence-cache -->
#   ```yaml
#   (filled in by this script)
#   ```
#
# and shows its picture with ![...](images/sequence-cache.svg).
#
#   docs/regen-gallery.sh           draw every picture, copy the specs into the pages,
#                                   sync the images into the books and check the links
#   docs/regen-gallery.sh --check   only check that the YAML in the pages matches the specs
#                                   (no build, nothing written; exit 1 when they differ)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
mode="${1:-write}"
case "$mode" in write|--check) ;; *) echo "usage: $0 [--check]" >&2; exit 2;; esac

if [ "$mode" = write ]; then
  echo "drawing the pictures (golden test in update mode)"
  (cd "$root" && SVGDOG_UPDATE_GOLDEN=1 cargo test --quiet --test cli r8_output_matches >/dev/null)
fi

python3 -I - "$root" "$mode" <<'PY'
import pathlib, re, sys
root, mode = pathlib.Path(sys.argv[1]), sys.argv[2]
fixtures = root / "tests/fixtures"
gallery = fixtures / "gallery"
block = re.compile(r"(<!-- spec: ([a-z0-9-]+) -->\n```yaml\n)(.*?)(```)", re.S)

def spec_text(name):
    for p in (gallery / f"{name}.yaml", fixtures / f"{name}.yaml"):
        if p.exists():
            lines = p.read_text().splitlines()
            while lines and (not lines[0].strip() or lines[0].lstrip().startswith("#")):
                lines.pop(0)
            return "\n".join(lines) + "\n"
    return None

bad, used = 0, set()
for md in sorted((root / "docs/guide").rglob("*.md")):
    text = md.read_text()
    def fill(m):
        global bad
        name = m.group(2)
        used.add(name)
        want = spec_text(name)
        if want is None:
            print(f"{md.relative_to(root)}: no spec named {name}", file=sys.stderr)
            bad += 1
            return m.group(0)
        if m.group(3) != want and mode == "--check":
            print(f"{md.relative_to(root)}: the YAML for {name} differs from its spec; run docs/regen-gallery.sh", file=sys.stderr)
            bad += 1
        return m.group(1) + want + m.group(4)
    new = block.sub(fill, text)
    if mode == "write" and new != text:
        md.write_text(new)
        print(f"updated {md.relative_to(root)}")
for p in sorted(gallery.glob("*.yaml")):
    if p.stem not in used:
        print(f"warning: tests/fixtures/gallery/{p.name} is shown on no page", file=sys.stderr)
sys.exit(1 if bad else 0)
PY

if [ "$mode" = write ]; then
  "$root/docs/sync-assets.sh"
  "$root/docs/check.sh"
fi
