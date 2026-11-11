// Steps and details for one diagram. Without this script the page still shows
// the whole drawing with its numbered steps; the script only adds a way through them.
(function () {
  var svg = document.querySelector("figure svg");
  var bar = document.querySelector(".bar");
  var detail = document.querySelector(".detail");
  var total = __STEPS__;
  var current = 0;

  function show(n) {
    current = n;
    if (n) svg.setAttribute("data-current", n);
    else svg.removeAttribute("data-current");
    if (!bar) return;
    bar.querySelector("[data-go=prev]").disabled = n <= 1;
    bar.querySelector("[data-go=next]").disabled = n >= total;
    bar.querySelector("[data-go=all]").disabled = !n;
    bar.querySelector("output").textContent = n ? "Step " + n + " of " + total : total + " steps";
  }

  if (bar && total) {
    bar.hidden = false;
    bar.addEventListener("click", function (e) {
      var go = e.target.getAttribute && e.target.getAttribute("data-go");
      if (go === "prev") show(Math.max(1, current - 1));
      if (go === "next") show(Math.min(total, current + 1));
      if (go === "all") show(0);
    });
    document.addEventListener("keydown", function (e) {
      if (e.target.closest && e.target.closest("input, textarea, select")) return;
      if (e.key === "ArrowRight") show(Math.min(total, current + 1));
      if (e.key === "ArrowLeft" && current) show(Math.max(1, current - 1));
      if (e.key === "Escape") show(0);
    });
    // A step in the list is also a way to choose it.
    for (var k = 1; k <= total; k++) {
      (function (n) {
        var row = svg.querySelector("#step-" + n);
        if (!row) return;
        row.style.cursor = "pointer";
        row.addEventListener("click", function () { show(current === n ? 0 : n); });
      })(k);
    }
    show(0);
  }

  // A thing with a detail shows it under the drawing when chosen.
  svg.addEventListener("click", function (e) {
    var item = e.target.closest ? e.target.closest(".it") : null;
    var title = item && item.querySelector(":scope > title");
    detail.textContent = title ? title.textContent : "";
  });
})();
