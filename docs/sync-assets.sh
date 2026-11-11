#!/usr/bin/env bash
# Copies diagram assets from the golden test outputs into each book's images/ folder,
# but only the ones that book's pages reference. Golden files are the single source, so the
# pictures in the docs never drift from the renderer.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
for book in user operator tester; do
  dir="$root/docs/guide/$book"
  mkdir -p "$dir/images"
  find "$dir/images" -type f -delete
  grep -rhoE 'images/[A-Za-z0-9._-]+\.(svg|png)' --include='*.md' "$dir" 2>/dev/null | sort -u |
  while read -r ref; do
    name="${ref#images/}"
    [ -f "$root/tests/golden/$name" ] && cp "$root/tests/golden/$name" "$dir/images/$name" \
      || echo "warning: $book references $ref but tests/golden/$name does not exist" >&2
  done
done
if [ -f "$root/tests/golden/orders.svg" ]; then
  mkdir -p "$root/docs/guide/assets" && cp "$root/tests/golden/orders.svg" "$root/docs/guide/assets/orders.svg"
fi
python3 -I "$root/docs/deck/build.py"
echo "assets synced"
