// A tiny stand-in for the page: enough DOM for the player to run against.
const fs = require("fs");
const src = fs.readFileSync(process.argv[2], "utf8").replace("__STEPS__", "3");
function el(extra) {
  const attrs = {}, handlers = {};
  return Object.assign({
    hidden: true, disabled: false, textContent: "", style: {},
    setAttribute: (k, v) => (attrs[k] = String(v)), getAttribute: (k) => (k in attrs ? attrs[k] : null), removeAttribute: (k) => delete attrs[k],
    addEventListener: (t, f) => (handlers[t] = f), fire: (t, e) => handlers[t] && handlers[t](e), attrs,
  }, extra);
}
const buttons = { prev: el(), next: el(), all: el() }, output = el();
for (const k in buttons) buttons[k].setAttribute("data-go", k);
const rows = { 1: el(), 2: el(), 3: el() };
const bar = el({ querySelector: (s) => (s === "output" ? output : buttons[s.match(/data-go=(\w+)/)[1]]) });
const detail = el();
const svg = el({ querySelector: (s) => rows[s.replace("#step-", "")] || null });
const document = el({ querySelector: (s) => (s === "figure svg" ? svg : s === ".bar" ? bar : detail) });
new Function("document", src)(document);
const assert = (c, m) => { if (!c) { console.error("FAIL: " + m); process.exit(1); } };
assert(bar.hidden === false && output.textContent === "3 steps" && buttons.prev.disabled && !buttons.next.disabled && buttons.all.disabled, "start: all steps shown");
bar.fire("click", { target: buttons.next });
assert(svg.attrs["data-current"] === "1" && output.textContent === "Step 1 of 3", "next goes to step 1");
document.fire("keydown", { key: "ArrowRight", target: {} }); document.fire("keydown", { key: "ArrowRight", target: {} }); document.fire("keydown", { key: "ArrowRight", target: {} });
assert(svg.attrs["data-current"] === "3" && buttons.next.disabled, "arrow keys stop at the last step");
document.fire("keydown", { key: "ArrowLeft", target: {} });
assert(svg.attrs["data-current"] === "2", "left goes back");
rows[1].fire("click"); assert(svg.attrs["data-current"] === "1", "a step row chooses its step");
rows[1].fire("click"); assert(!("data-current" in svg.attrs), "choosing it again shows everything");
bar.fire("click", { target: buttons.next }); document.fire("keydown", { key: "Escape", target: {} });
assert(!("data-current" in svg.attrs) && output.textContent === "3 steps", "escape shows everything");
const titled = { querySelector: () => ({ textContent: "Owns the order lifecycle." }) };
svg.fire("click", { target: { closest: () => titled } }); assert(detail.textContent === "Owns the order lifecycle.", "a detail is shown");
svg.fire("click", { target: { closest: () => null } }); assert(detail.textContent === "", "and cleared");
console.log("player ok");
