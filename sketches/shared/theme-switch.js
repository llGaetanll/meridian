/* The switch between the two themes, on every sketch that draws the app.

   Not a control of the app's: it is a thing for looking at sketches with, so
   it wears none of the design language and sits out of the way in a corner.
   The choice is kept in localStorage under one key, so walking from the board
   to the design language does not land in the other theme.

   The theme is an attribute on <html>, which is all theme.css reads. */

(function () {
  var KEY = "meridian-sketch-theme";
  var root = document.documentElement;

  function set(name) {
    if (name === "light") root.setAttribute("data-theme", "light");
    else root.removeAttribute("data-theme");
    try { localStorage.setItem(KEY, name); } catch (e) {}
  }

  var saved = "dark";
  try { saved = localStorage.getItem(KEY) || "dark"; } catch (e) {}
  set(saved);

  function draw() {
    var css = document.createElement("style");
    css.textContent =
      ".theme-switch{position:fixed;right:18px;bottom:18px;z-index:9999;" +
      "display:flex;gap:2px;padding:3px;border-radius:999px;" +
      "border:1px solid var(--border-strong);background:var(--bg-panel);" +
      "box-shadow:var(--shadow-pop);font:inherit}" +
      ".theme-switch button{border:0;border-radius:999px;cursor:pointer;" +
      "padding:6px 13px;font:inherit;font-size:12px;font-weight:600;" +
      "background:none;color:var(--text-faint)}" +
      ".theme-switch button.on{background:var(--accent-tint);color:var(--accent-text)}";
    document.head.appendChild(css);

    var box = document.createElement("div");
    box.className = "theme-switch";
    ["dark", "light"].forEach(function (name) {
      var b = document.createElement("button");
      b.type = "button";
      b.textContent = name === "dark" ? "Dark" : "Light";
      b.onclick = function () {
        set(name);
        box.querySelectorAll("button").forEach(function (x) { x.classList.remove("on"); });
        b.classList.add("on");
      };
      if (saved === name) b.classList.add("on");
      box.appendChild(b);
    });
    document.body.appendChild(box);
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", draw);
  else draw();
})();
