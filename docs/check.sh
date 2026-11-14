#!/usr/bin/env bash
# Keeps the docs honest: relative links resolve, and every CLI command and MCP tool
# appears in the operator book. Needs target/release/svgdog (cargo build --release).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
bin="${SVGDOG:-$root/target/release/svgdog}"
guide="$root/docs/guide"
fail=0

python3 -I - "$guide" <<'PY' || fail=1
import re, sys, pathlib
root = pathlib.Path(sys.argv[1])
bad = 0
for md in root.rglob("*.md"):
    text = re.sub(r"```.*?```", "", md.read_text(), flags=re.S)
    for target in re.findall(r"\]\(([^)\s]+)", text):
        if re.match(r"^(https?:|mailto:|#)", target):
            continue
        path = target.split("#")[0]
        if not path:
            continue
        dest = (md.parent / path).resolve()
        if not dest.exists():
            print(f"broken link: {md.relative_to(root)} -> {target}")
            bad += 1
sys.exit(1 if bad else 0)
PY

ref="$guide/operator"
for cmd in render validate inspect preview edit fix schema rules examples describe docs theme mcp; do
  grep -rqw -- "svgdog $cmd" "$ref" || { echo "operator book never mentions: svgdog $cmd"; fail=1; }
done
for tool in render validate inspect preview edit fix schema rules examples describe docs theme_list theme_init theme_check; do
  grep -rq -- "svgdog_$tool" "$ref" || { echo "operator book never mentions MCP tool: svgdog_$tool"; fail=1; }
done
if [ -x "$bin" ]; then
  while read -r id; do
    grep -rq -- "$id" "$ref/reference/rules.md" || { echo "rules.md lacks rule: $id"; fail=1; }
  done < <("$bin" rules --format json | python3 -I -c 'import json,sys; d=json.load(sys.stdin); r=d.get("rules") or d.get("data",{}).get("rules",[]); [print(x["id"]) for x in r]')
fi
[ "$fail" = 0 ] && echo "docs check passed"
exit "$fail"
