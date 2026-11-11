"""Builds docs/guide/tldr.html from the template, embedding the golden SVGs as data URIs.
Run from anywhere: python3 docs/deck/build.py  (also done by docs/sync-assets.sh)."""
import base64, pathlib

root = pathlib.Path(__file__).resolve().parents[2]
golden = root / "tests" / "golden"
text = (root / "docs" / "deck" / "tldr.template.html").read_text()
for name in ["orders", "checkout", "tiers", "login", "rack"]:
    b64 = base64.b64encode((golden / f"{name}.svg").read_bytes()).decode()
    text = text.replace("{{%s}}" % name, "data:image/svg+xml;base64," + b64)
(root / "docs" / "guide" / "tldr.html").write_text(text)
print("deck built")
