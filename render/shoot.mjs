/// Shoots one element out of a sketch into a PNG.
///
/// The device renders project a picture of the app onto a screen, and this is
/// where that picture comes from: the sketches, which are the app's own markup
/// in the app's own design language. A screen that changes there changes in the
/// render on the next run, which a screenshot pasted into a folder does not.
///
///     node render/shoot.mjs [outdir]

import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";

const CHROME = "/usr/bin/google-chrome-stable";
const PORT = 9333;

/// Every shot, in one place: the sketch it comes out of, the element inside it,
/// the viewport that element is laid out in, and how many device pixels a CSS
/// pixel becomes. The scale is what keeps a phone screen sharp when it is a
/// few hundred pixels wide in the finished render.
const SHOTS = [
  {
    name: "phone-recurring",
    page: "sketches/mobile/redesign.html",
    sel: "#recurring .phone",
    width: 1600,
    height: 1400,
    scale: 4,
  },
  {
    name: "phone-balances",
    page: "sketches/mobile/redesign.html",
    sel: "#balances .phone",
    width: 1600,
    height: 1400,
    scale: 4,
  },
  {
    name: "desktop-markets",
    page: "sketches/desktop/asset.html",
    sel: "body",
    width: 1680,
    height: 1050,
    scale: 2,
    menubar: "Meridian",
  },
];

const ROOT = resolve(import.meta.dirname, "..");

/// One CDP connection, with the request and response bookkeeping the protocol
/// needs and does not do for you.
class Cdp {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.waiting = new Map();
    ws.addEventListener("message", (e) => {
      const msg = JSON.parse(e.data);
      const pending = this.waiting.get(msg.id);
      if (!pending) return;
      this.waiting.delete(msg.id);
      if (msg.error) pending.reject(new Error(msg.error.message));
      else pending.resolve(msg.result);
    });
  }

  static async open(url) {
    const ws = new WebSocket(url);
    await new Promise((ok, no) => {
      ws.addEventListener("open", ok, { once: true });
      ws.addEventListener("error", no, { once: true });
    });
    return new Cdp(ws);
  }

  send(method, params = {}) {
    const id = ++this.id;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((ok, no) => this.waiting.set(id, { resolve: ok, reject: no }));
  }
}

/// Lifts the element out of the sketch and stands it alone on the page, at the
/// origin, with everything else thrown away.
///
/// A clip out of the page as it stands does not work: a screen is its own
/// `overflow: auto` scroller, and both of the ways Chrome shoots a box larger
/// than the viewport — re-laying the page out, or scrolling to it — come back
/// with that scroller empty. Standing the element alone means the box and the
/// viewport are the same rectangle, which is the case that works.
const ISOLATE = (sel) => `
  (async () => {
    const el = document.querySelector(${JSON.stringify(sel)});
    if (!el) return null;
    const r = el.getBoundingClientRect();

    // The sprite sheet comes too. Every icon in a sketch is a <use> pointing at
    // a <symbol> elsewhere in the document, and a screen torn out without it is
    // a screen with no icons in it.
    const sprites = [...document.querySelectorAll("svg")].filter((s) => s.querySelector("symbol"));

    const kept = el.cloneNode(true);
    document.body.replaceChildren(...sprites, kept);
    document.body.style.cssText =
      "margin:0;padding:0;width:" + r.width + "px;height:" + r.height + "px;overflow:hidden";
    document.documentElement.style.cssText = "margin:0;padding:0;overflow:hidden";

    await new Promise((ok) => requestAnimationFrame(() => requestAnimationFrame(ok)));
    return { width: r.width, height: r.height };
  })()
`;

/// The macOS menu bar, drawn over the top of a page before it is shot.
///
/// A laptop display has a notch, and the model has one. Without the strip the
/// notch bites into whatever the app draws along its top edge — on the markets
/// page, the search field. A real Mac never lets that happen: the menu bar owns
/// that band and the notch sits inside it.
const MENUBAR = (app) => `
  (() => {
    const bar = document.createElement("div");
    bar.style.cssText = [
      "position:fixed", "top:0", "left:0", "right:0", "height:25px",
      "z-index:2147483647",
      "background:#0a0d11",
      "display:flex", "align-items:center", "gap:19px",
      "padding:0 18px",
      "font:600 13px/1 -apple-system, SF Pro Text, Inter, sans-serif",
      "color:#e8eaed",
      "letter-spacing:-0.01em",
    ].join(";");

    // An apple, the app, its menus, and a clock on the right. Drawn rather than
    // screenshotted because the strip only has to read as a menu bar at the
    // size a laptop display is on a page.
    const left = document.createElement("div");
    left.style.cssText = "display:flex;align-items:center;gap:19px";

    // Drawn as an SVG rather than set in a font: the Apple glyph lives at a
    // private-use codepoint that only Apple's own fonts carry, and everywhere
    // else it comes out as an empty box.
    const apple = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    apple.setAttribute("viewBox", "0 0 16 20");
    apple.setAttribute("width", "13");
    apple.setAttribute("height", "16");
    apple.style.cssText = "opacity:0.92;display:block";
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("fill", "#e8eaed");
    path.setAttribute("d",
      "M13.2 10.6c0-2.2 1.8-3.3 1.9-3.4-1-1.5-2.6-1.7-3.2-1.7-1.4-.1-2.7.8-3.3.8" +
      "-.7 0-1.7-.8-2.8-.8-1.5 0-2.8.9-3.6 2.2-1.5 2.6-.4 6.5 1.1 8.6.7 1 1.6 2.2 2.7 2.2" +
      "1.1 0 1.5-.7 2.8-.7 1.3 0 1.6.7 2.8.7 1.2 0 1.9-1 2.6-2.1.8-1.2 1.2-2.4 1.2-2.5" +
      "-.1 0-2.2-.9-2.2-3.3zM10.9 3.9c.6-.7 1-1.7.9-2.7-.9 0-2 .6-2.6 1.3-.6.6-1.1 1.7-.9 2.6" +
      "1 .1 2-.5 2.6-1.2z");
    apple.appendChild(path);
    left.appendChild(apple);

    for (const [i, item] of ["${app}", "File", "Edit", "View", "Window", "Help"].entries()) {
      const el = document.createElement("span");
      el.textContent = item;
      el.style.cssText = i === 0
        ? "font-weight:700"
        : "font-weight:500;opacity:0.82";
      left.appendChild(el);
    }

    const right = document.createElement("div");
    right.style.cssText =
      "margin-left:auto;display:flex;align-items:center;gap:15px;font-weight:500;opacity:0.86";
    for (const item of ["\u25B2", "\u25EC", "Thu 2 Aug", "9:41"]) {
      const el = document.createElement("span");
      el.textContent = item;
      right.appendChild(el);
    }

    bar.append(left, right);
    document.body.appendChild(bar);

    // The page moves down by the height of the strip rather than sliding under
    // it, so nothing the app draws is hidden behind the menus.
    document.body.style.paddingTop = "25px";
  })()
`;

async function shoot(cdp, shot, outDir) {
  const url = `file://${resolve(ROOT, shot.page)}`;

  // Laid out at the sketch's own width first, so the element measures the same
  // as it does when a person opens the sketch.
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width: shot.width,
    height: shot.height,
    deviceScaleFactor: shot.scale,
    mobile: false,
  });

  await cdp.send("Page.navigate", { url });
  // The sketches lay out on a webfont and nothing signals when that has
  // landed, so the wait is for `fonts.ready` with a frame either side of it.
  await sleep(400);
  await cdp.send("Runtime.evaluate", {
    expression: "document.fonts.ready",
    awaitPromise: true,
  });
  await sleep(200);

  if (shot.menubar) {
    await cdp.send("Runtime.evaluate", { expression: MENUBAR(shot.menubar) });
    await sleep(120);
  }

  const { result } = await cdp.send("Runtime.evaluate", {
    expression: ISOLATE(shot.sel),
    returnByValue: true,
    awaitPromise: true,
  });
  if (!result.value) throw new Error(`${shot.name}: nothing matches ${shot.sel}`);

  const { width, height } = result.value;

  // The viewport is set to the element, so the shot is the whole of it and the
  // clip is the whole viewport.
  await cdp.send("Emulation.setDeviceMetricsOverride", {
    width: Math.ceil(width),
    height: Math.ceil(height),
    deviceScaleFactor: shot.scale,
    mobile: false,
  });
  await sleep(150);

  const { data } = await cdp.send("Page.captureScreenshot", { format: "png" });

  const out = resolve(outDir, `${shot.name}.png`);
  await writeFile(out, Buffer.from(data, "base64"));
  const px = (n) => Math.round(n * shot.scale);
  console.log(`${shot.name}  ${px(width)}x${px(height)}  ${out}`);
}

const outDir = resolve(process.argv[2] ?? resolve(import.meta.dirname, "screens"));
await mkdir(outDir, { recursive: true });

const chrome = spawn(
  CHROME,
  [
    "--headless=new",
    // The dock and the bar are glass, and `backdrop-filter` is one of the
    // things that silently does nothing when the compositor has no GPU.
    "--use-angle=vulkan",
    "--enable-features=Vulkan",
    "--hide-scrollbars",
    "--force-color-profile=srgb",
    "--no-sandbox",
    `--remote-debugging-port=${PORT}`,
    "--user-data-dir=/tmp/meridian-shoot-profile",
    "about:blank",
  ],
  { stdio: "ignore" },
);

try {
  let target;
  for (let i = 0; i < 60; i++) {
    try {
      const res = await fetch(`http://127.0.0.1:${PORT}/json/list`);
      target = (await res.json()).find((t) => t.type === "page");
      if (target) break;
    } catch {}
    await sleep(100);
  }
  if (!target) throw new Error("chrome never came up on the debugging port");

  const cdp = await Cdp.open(target.webSocketDebuggerUrl);
  await cdp.send("Page.enable");
  await cdp.send("Runtime.enable");

  for (const shot of SHOTS) await shoot(cdp, shot, outDir);
} finally {
  chrome.kill();
}
