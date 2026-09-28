// inklift — front end.
//
// The retune loop is the whole point: a slider move re-extracts from the source
// the backend is already holding, never from the screen or the disk. Previews
// run on a proxy so a drag stays fluid; a full-resolution pass follows once the
// user stops moving.

// Everything below needs the injected API. Read it through a guard rather than
// straight off the global: when it is missing (withGlobalTauri off, a plugin not
// registered) an unguarded `window.__TAURI__.core` throws here, on line one, and
// every listener below it is never attached — a window that renders perfectly
// and ignores every click, with nothing in the log. Say so on the page instead.
if (!window.__TAURI__ || !window.__TAURI__.core) {
  document.addEventListener("DOMContentLoaded", () => {
    const p = document.createElement("p");
    p.textContent =
      "inklift cannot reach its backend: the Tauri API was not injected into " +
      "this window. The app needs withGlobalTauri enabled in tauri.conf.json.";
    p.setAttribute("style",
      "position:fixed;inset:auto 16px 16px;z-index:99;margin:0;padding:14px 16px;" +
      "border-radius:12px;font:13px/1.5 system-ui,sans-serif;" +
      "background:#3A1518;color:#FFDCDC;box-shadow:0 0 0 1px rgba(255,120,120,.35)");
    document.body.append(p);
  });
  throw new Error("inklift: window.__TAURI__ missing — is withGlobalTauri on?");
}

const { invoke } = window.__TAURI__.core;

const $ = (id) => document.getElementById(id);
const el = {
  label: $("label"), ground: $("ground"), ink: $("ink"), src: $("src"),
  empty: $("empty"), peekHint: $("peekHint"), chip: $("chip"),
  readout: $("readout"), diag: $("diag"), banner: $("banner"),
  save: $("saveBtn"), copy: $("copyBtn"), toast: $("toast"),
  k: $("k"), m: $("m"), r: $("r"), inv: $("inv"),
  kVal: $("kVal"), mVal: $("mVal"), rVal: $("rVal"),
  eraser: $("eraserBtn"), eSize: $("eSize"), eSoft: $("eSoft"),
  eSizeVal: $("eSizeVal"), eSoftVal: $("eSoftVal"),
  undo: $("undoBtn"), clear: $("clearBtn"), brush: $("brush"),
};

let loaded = false;
let output = "alpha";
let inkChoice = "";
let previewTimer = null, fullTimer = null, inFlight = false, queued = false;
let source = null;      // the loaded image's size; strokes are in its pixels
let erasing = false;
let strokeCount = 0;    // how many strokes Rust holds, as it last said
let stroke = null;      // the stroke being dragged, in source pixels
let strokeBase = null;  // canvas pixels as they were when the stroke began (ImageData)
let strokeCov = null;   // Float32Array: this stroke's per-pixel max coverage so far

/* ── helpers ───────────────────────────────────────────────────────────── */

function toast(msg, bad) {
  el.toast.textContent = msg;
  el.toast.classList.toggle("bad", !!bad);
  el.toast.classList.add("on");
  clearTimeout(toast._t);
  toast._t = setTimeout(() => el.toast.classList.remove("on"), bad ? 5200 : 2600);
}

function params() {
  return {
    // The slider reads as "how much to pick up", so it runs opposite to
    // Sauvola's k: dragging right must find MORE ink, not less.
    k: (46 - Number(el.k.value)) / 100,
    minArea: Number(el.m.value),
    feather: 1,
    // The thickest stroke's half-width, or null for automatic. The backend
    // widens the Sauvola window with it: the window, not the paper radius, is
    // what leaves thick strokes hollow, so it must not be fixed here.
    stroke: Number(el.r.value) > 0 ? Number(el.r.value) : null,
    invert: el.inv.checked,
    ink: inkChoice || null,
  };
}

function segment(id, onPick) {
  const seg = $(id);
  seg.addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    seg.querySelectorAll("button").forEach((x) =>
      x.setAttribute("aria-pressed", String(x === b)));
    onPick(b.dataset.v);
  });
}

/** Build the readouts as DOM nodes. No innerHTML anywhere: a filename or an
 *  error string must never be able to become markup. */
function setReadout(r) {
  const frag = (parts) => {
    const f = document.createDocumentFragment();
    for (const p of parts) {
      if (typeof p === "string") { f.append(document.createTextNode(p)); }
      else { const b = document.createElement("b"); b.textContent = p.b; f.append(b); }
    }
    return f;
  };
  el.readout.replaceChildren(frag([
    "ink ", { b: (r.coverage * 100).toFixed(1) + "%" },
    " · pen ", { b: r.inkHex },
    " · ", { b: r.width + " × " + r.height },
    r.proxy ? " · preview" : "",
  ]));

  const line = (nodes) => { const d = document.createElement("div"); d.append(...nodes); return d; };
  const dot = document.createElement("span");
  dot.className = "dot";
  // a colour token from the backend, set as a property rather than parsed markup
  dot.style.backgroundColor = /^#[0-9A-Fa-f]{6}$/.test(r.inkHex) ? r.inkHex : "transparent";
  el.diag.replaceChildren(
    line([dot, document.createTextNode(r.inkHex)]),
    line([document.createTextNode(r.ms + " ms" + (r.proxy ? " (proxy)" : ""))]),
  );
}

/* ── rendering ─────────────────────────────────────────────────────────── */

async function render(full) {
  if (!loaded) return;
  if (inFlight) { queued = true; return; }
  inFlight = true;
  try {
    const r = await invoke("render", { params: params(), full: !!full });
    await paint(r.png);
    el.src.src = r.sourcePng;
    el.ink.hidden = false; el.src.hidden = false;
    el.empty.hidden = true; el.peekHint.hidden = false;
    el.save.disabled = false; el.copy.disabled = false;
    // Only turned on once there is a painted canvas to erase from — see
    // adopt(), which disables these again the moment a new image starts loading.
    el.eraser.disabled = false; el.eSize.disabled = false; el.eSoft.disabled = false;

    setReadout(r);

    if (r.proxy && !full) {
      el.chip.textContent = "Refining…"; el.chip.classList.add("on");
      clearTimeout(fullTimer);
      fullTimer = setTimeout(() => render(true), 220);
    } else {
      el.chip.classList.remove("on");
    }

    if (r.coverage < 0.003) {
      toast("Almost nothing came out — try flipping, or picking up more.", true);
    }
  } catch (e) {
    toast(String(e), true);
  } finally {
    inFlight = false;
    if (queued) { queued = false; render(false); }
  }
}

/** Draw a rendered PNG into the result canvas. A canvas rather than an <img>
 *  because a stroke has to show while it is being dragged, before Rust has it. */
function paint(uri) {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      el.ink.width = img.naturalWidth;
      el.ink.height = img.naturalHeight;
      el.ink.getContext("2d").drawImage(img, 0, 0);
      // a render that lands mid-drag must not wipe the stroke off the screen
      if (stroke) redrawStroke();
      resolve();
    };
    img.onerror = () => reject(new Error("the rendered result could not be decoded"));
    img.src = uri;
  });
}

/** Debounced so a dragged slider does not queue a job per pixel. */
function schedulePreview() {
  clearTimeout(previewTimer);
  clearTimeout(fullTimer);
  previewTimer = setTimeout(() => render(false), 60);
}

function adopt(info) {
  loaded = true;
  source = { width: info.width, height: info.height };
  strokeCount = 0;   // Rust dropped the old image's strokes with it
  stroke = null;
  strokeBase = null;  // any in-progress cut was against the old image's pixels
  strokeCov = null;
  // Disabled again until the first paint of the new image: the canvas still
  // shows the old one (or is empty, on the very first load) until then, and
  // there is nothing correct for the eraser to cut yet.
  el.eraser.disabled = true; el.eSize.disabled = true; el.eSoft.disabled = true;
  syncEraser();
  el.label.textContent = info.label + (info.region ? `  ${info.region}` : "");
  // Advisory only. The extracted pen is a fact about the source, so the flip is
  // offered rather than applied behind the user's back.
  el.banner.hidden = !(info.looksInverted && !el.inv.checked);
  render(false);
}

/* ── actions ───────────────────────────────────────────────────────────── */

$("openBtn").addEventListener("click", async () => {
  try {
    // Rust owns the picker; see pick_open's comment for why it cannot live here.
    const path = await invoke("pick_open");
    if (!path) return;
    adopt(await invoke("open_file", { path }));
  } catch (e) { toast(String(e), true); }
});

// Grey the button out where the backend has no capture backend, so it does
// not invite a click that can only fail. The tray item is disabled the same way.
invoke("capture_supported").then((ok) => {
  if (ok) return;
  const b = $("grabBtn");
  b.disabled = true;
  b.title = "Lifting from the screen works on Linux (X11) and Windows; this " +
            "build has no capture backend for your platform yet. Open a file instead.";
}).catch(() => {});

$("grabBtn").addEventListener("click", async () => {
  try {
    // The screen is grabbed before the overlay appears, so the overlay can
    // never end up in its own capture. The result arrives as an event, since
    // the drag finishes in the overlay's context rather than this one.
    await invoke("begin_pick");
  } catch (e) { toast(String(e), true); }
});

el.save.addEventListener("click", async () => {
  try {
    const path = await invoke("pick_save", {
      defaultName: output === "white" ? "ink-on-white.png" : "ink.png",
    });
    if (!path) return;
    await invoke("save", { path, params: params(), white: output === "white" });
    toast("Saved");
  } catch (e) { toast(String(e), true); }
});

el.copy.addEventListener("click", async () => {
  try {
    await invoke("copy", { params: params() });
    toast("Copied — paste it anywhere");
  } catch (e) { toast(String(e), true); }
});

$("flipBtn").addEventListener("click", () => {
  el.inv.checked = true;
  el.banner.hidden = true;
  schedulePreview();
});

/* ── controls ──────────────────────────────────────────────────────────── */

el.k.addEventListener("input", () => { el.kVal.textContent = ((46 - +el.k.value) / 100).toFixed(2); schedulePreview(); });
el.m.addEventListener("input", () => { el.mVal.textContent = el.m.value; schedulePreview(); });
el.r.addEventListener("input", () => { el.rVal.textContent = +el.r.value > 0 ? el.r.value + " px" : "auto"; schedulePreview(); });
el.inv.addEventListener("change", () => { el.banner.hidden = true; schedulePreview(); });
el.kVal.textContent = ((46 - +el.k.value) / 100).toFixed(2);

$("inkSw").addEventListener("click", (e) => {
  const b = e.target.closest("button"); if (!b) return;
  $("inkSw").querySelectorAll("button").forEach((x) =>
    x.setAttribute("aria-pressed", String(x === b)));
  inkChoice = b.dataset.ink;
  schedulePreview();
});

segment("groundSeg", (v) => el.ground.setAttribute("data-bg", v));
segment("themeSeg", (v) => document.documentElement.setAttribute("data-theme", v));
segment("outSeg", (v) => { output = v; });

/* ── eraser ────────────────────────────────────────────────────────────── */
//
// The strokes live in Rust, in source pixels, and every render, save and copy
// applies them. The canvas here is only feedback while the pointer is down: it
// cuts the stroke out with the same fall-off as erase.rs, then the normal
// preview → full refresh replaces it with what Rust will actually export.

/** Erase strength at distance `d` from a stroke's centre line. Mirrors
 *  erase.rs's `coverage` exactly — the two must change together, or the live
 *  cut here and what Rust actually exports will disagree. Overlapping
 *  destination-out dabs used to compound (each multiplying alpha by (1 − a)),
 *  which over-erased a soft edge while dragging and then sprang back on
 *  release; this profile is instead the per-stroke maximum, same as Rust. */
function coverage(d, radius, softness) {
  const inner = Math.max(0, Math.min(radius * (1 - softness), radius - 1));
  if (d <= inner) return 1;
  if (d >= radius) return 0;
  const t = (radius - d) / (radius - inner);
  return t * t * (3 - 2 * t);
}

/** Mirrors erase.rs's `distance_to_segment`. */
function distanceToSegment(p, a, b) {
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const len2 = dx * dx + dy * dy;
  const t = len2 > 0
    ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2))
    : 0;
  const cx = a[0] + t * dx - p[0], cy = a[1] + t * dy - p[1];
  return Math.hypot(cx, cy);
}

/** Cut one segment (in canvas pixels) into the canvas, against `strokeBase` —
 *  the snapshot taken when this stroke began — rather than the canvas's
 *  current pixels. Keeping a fixed base and taking the max of `strokeCov`
 *  against each pixel's prior coverage is what makes a self-crossing stroke
 *  behave like erase.rs's keep_mask: the strongest segment wins, once, instead
 *  of every pass cutting a little more out of what the last pass already cut. */
function cutSegment(a, b) {
  const w = strokeBase.width, h = strokeBase.height;
  const s = el.ink.width / source.width;   // the canvas may hold the proxy
  const r = stroke.radius * s;
  const x0 = Math.max(0, Math.floor(Math.min(a[0], b[0]) - r));
  const y0 = Math.max(0, Math.floor(Math.min(a[1], b[1]) - r));
  const x1 = Math.min(w, Math.ceil(Math.max(a[0], b[0]) + r));
  const y1 = Math.min(h, Math.ceil(Math.max(a[1], b[1]) + r));
  if (x0 >= x1 || y0 >= y1) return;
  const bw = x1 - x0;
  // A pixel's coverage is measured from its centre, matching keep_mask's
  // (x + 0.5, y + 0.5) — otherwise this preview's edge sits half a pixel off
  // from what Rust rasterises.
  const patch = new ImageData(bw, y1 - y0);
  for (let y = y0; y < y1; y++) {
    for (let x = x0; x < x1; x++) {
      const d = distanceToSegment([x + 0.5, y + 0.5], a, b);
      const c = coverage(d, r, stroke.softness);
      const i = y * w + x;
      if (c > strokeCov[i]) strokeCov[i] = c;
      const si = i * 4, pi = ((y - y0) * bw + (x - x0)) * 4;
      patch.data[pi] = strokeBase.data[si];
      patch.data[pi + 1] = strokeBase.data[si + 1];
      patch.data[pi + 2] = strokeBase.data[si + 2];
      patch.data[pi + 3] = strokeBase.data[si + 3] * (1 - strokeCov[i]);
    }
  }
  el.ink.getContext("2d").putImageData(patch, x0, y0);
}

/** (Re)snapshot the canvas and cut every segment of the current stroke into
 *  it. Used both to start a stroke and to replay one whenever the canvas is
 *  repainted underneath a drag — a proxy ↔ full swap mid-stroke leaves
 *  `strokeBase` pointing at pixels that no longer exist, so the cut has to
 *  start over against the fresh ones. */
function redrawStroke() {
  const ctx = el.ink.getContext("2d");
  strokeBase = ctx.getImageData(0, 0, el.ink.width, el.ink.height);
  strokeCov = new Float32Array(el.ink.width * el.ink.height);
  const s = el.ink.width / source.width;
  const pts = stroke.points.map((p) => [p[0] * s, p[1] * s]);
  cutSegment(pts[0], pts[0]);
  for (let i = 1; i < pts.length; i++) cutSegment(pts[i - 1], pts[i]);
}

/** Pointer → source-image pixels, whatever size the canvas is shown at. */
function toSource(e) {
  const b = el.ink.getBoundingClientRect();
  return [(e.clientX - b.left) / b.width * source.width,
          (e.clientY - b.top) / b.height * source.height];
}

function moveBrush(e) {
  if (!erasing || !source) { el.brush.hidden = true; return; }
  const b = el.ink.getBoundingClientRect();
  const g = el.ground.getBoundingClientRect();
  const d = Number(el.eSize.value) * b.width / source.width;
  el.brush.style.width = el.brush.style.height = d + "px";
  el.brush.style.left = (e.clientX - g.left - d / 2) + "px";
  el.brush.style.top = (e.clientY - g.top - d / 2) + "px";
  el.brush.hidden = false;
}

function syncEraser() {
  el.eraser.setAttribute("aria-pressed", String(erasing));
  el.ground.classList.toggle("erasing", erasing);
  if (!erasing) el.brush.hidden = true;
  el.peekHint.textContent = erasing ? "drag to erase · hold Space to compare" : "hold to compare";
  el.undo.disabled = strokeCount === 0;
  el.clear.disabled = strokeCount === 0;
}

/** Drop the stroke being dragged without sending it to Rust. Rather than try
 *  to undo the pixels cutSegment() already wrote, this just asks for a fresh
 *  render: Rust never heard about the stroke, so the next paint is the canvas
 *  as if it had never been drawn. */
function discardStroke() {
  if (!stroke) return;
  stroke = null;
  strokeBase = null;
  strokeCov = null;
  schedulePreview();
}

function setErasing(on) {
  const next = on && loaded;
  // Turning the eraser off mid-drag — via E, Escape, or the button itself —
  // must not silently commit whatever partial stroke was being cut.
  if (!next) discardStroke();
  erasing = next;
  syncEraser();
}

async function finishStroke() {
  const s = stroke;
  stroke = null;
  strokeBase = null;   // Rust owns this stroke now; the local cut is stale
  strokeCov = null;
  try {
    strokeCount = await invoke("erase_stroke", { stroke: s });
  } catch (e) {
    toast(String(e), true);
  }
  syncEraser();
  // Either way, redraw from Rust: on success the canvas converges on what will
  // export; on failure it drops an erasure that never happened.
  schedulePreview();
}

async function undoStroke() {
  if (!loaded || strokeCount === 0) return;
  try {
    strokeCount = await invoke("undo_erase");
    toast("Stroke undone");
    schedulePreview();
  } catch (e) { toast(String(e), true); }
  syncEraser();
}

async function clearStrokes() {
  if (!loaded || strokeCount === 0) return;
  try {
    await invoke("clear_erase");
    strokeCount = 0;
    toast("Erasing cleared");
    schedulePreview();
  } catch (e) { toast(String(e), true); }
  syncEraser();
}

el.eraser.addEventListener("click", (e) => {
  setErasing(!erasing);
  // A mouse click leaves focus on the button, where Space would toggle it
  // instead of peeking. Keyboard users keep their focus.
  if (e.detail > 0) el.eraser.blur();
});
el.eSize.addEventListener("input", () => { el.eSizeVal.textContent = el.eSize.value + " px"; });
el.eSoft.addEventListener("input", () => { el.eSoftVal.textContent = el.eSoft.value + " %"; });
el.undo.addEventListener("click", undoStroke);
el.clear.addEventListener("click", clearStrokes);

/* ── peek: hold to compare, which is how you actually verify an alpha ──── */
// While erasing, the pointer is busy painting, so comparing moves to Space.
const peek = (on) => loaded && el.ground.classList.toggle("peek", on);
el.ground.addEventListener("pointerdown", (e) => {
  if (!erasing) { peek(true); return; }
  // Before the first render there is no painted canvas to snapshot and cut.
  if (e.button !== 0 || !source || el.ink.hidden) return;
  el.ground.setPointerCapture(e.pointerId);
  stroke = { points: [toSource(e)], radius: Number(el.eSize.value) / 2,
             softness: Number(el.eSoft.value) / 100 };
  redrawStroke();
});
el.ground.addEventListener("pointermove", (e) => {
  moveBrush(e);
  if (!stroke) return;
  const p = toSource(e);
  const last = stroke.points[stroke.points.length - 1];
  // Thin the path: Rust measures distance to each segment, so sparse points
  // lose nothing in accuracy and save a great deal of rasterising.
  if (Math.hypot(p[0] - last[0], p[1] - last[1]) < Math.max(1, stroke.radius / 4)) return;
  stroke.points.push(p);
  const s = el.ink.width / source.width;   // the canvas may hold the proxy
  cutSegment([last[0] * s, last[1] * s], [p[0] * s, p[1] * s]);
});
el.ground.addEventListener("pointerup", () => {
  if (stroke) finishStroke();
  peek(false);
});
el.ground.addEventListener("pointercancel", () => {
  // The OS interrupting a drag must not commit a stroke that was never finished.
  discardStroke();
  peek(false);
});
el.ground.addEventListener("pointerleave", () => { el.brush.hidden = true; peek(false); });
document.addEventListener("keydown", (e) => {
  if (e.code === "Space" && !e.repeat && e.target === document.body) { e.preventDefault(); peek(true); }
  if (e.key === "Enter" && loaded && !e.ctrlKey) el.copy.click();
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && e.key.toLowerCase() === "z") { e.preventDefault(); undoStroke(); }
  if (!e.ctrlKey && !e.metaKey && !e.altKey && !e.repeat && e.key.toLowerCase() === "e" && loaded) setErasing(!erasing);
  if (e.key === "Escape" && erasing) setErasing(false);
});
document.addEventListener("keyup", (e) => { if (e.code === "Space") peek(false); });

/* ── events: the overlay's report, and a dropped file ──────────────────── */
//
// `listen` is a plugin command and so passes through tauri's ACL, unlike this
// app's own commands. A refused listener rejects a promise nobody awaits, which
// is silent — and it is how the overlay hands back a region, so losing it loses
// the feature. Await it and report, so a missing grant shows up at startup
// rather than after a drag the user then has to repeat.
(async () => {
  const { listen } = window.__TAURI__.event;
  const wired = [];
  const failed = [];

  try {
    await listen("picked", (e) => { if (e.payload) adopt(e.payload); });
    wired.push("picked");
  } catch (e) {
    toast("The overlay cannot report back: " + e, true);
  }

  try {
    // the tray owns no state; it announces the choice and the window applies it
    await listen("tray-output", (e) => {
      const v = e.payload;
      output = v;
      $("outSeg").querySelectorAll("button").forEach((b) =>
        b.setAttribute("aria-pressed", String(b.dataset.v === v)));
    });
    wired.push("tray-output");
  } catch (e) {
    failed.push("tray-output: " + e);
  }

  try {
    await listen("tray-copy", async () => {
      if (!loaded) { toast("Nothing has been lifted yet.", true); return; }
      try {
        await invoke("copy", { params: params() });
        toast("Copied — paste it anywhere");
      } catch (err) { toast(String(err), true); }
    });
    wired.push("tray-copy");
  } catch (e) {
    failed.push("tray-copy: " + e);
  }

  try {
    await listen("pick-failed", (e) => toast(String(e.payload), true));
    wired.push("pick-failed");
  } catch (e) {
    failed.push("pick-failed: " + e);
  }

  try {
    await listen("tauri://drag-drop", async (e) => {
      const p = e.payload && e.payload.paths && e.payload.paths[0];
      if (!p) return;
      try { adopt(await invoke("open_file", { path: p })); }
      catch (err) { toast(String(err), true); }
    });
    wired.push("drag-drop");
  } catch (e) {
    // Not fatal — the Open button still works — but it must not vanish into a
    // console nobody reads. The readiness line below names what did attach.
    failed.push("drag-drop: " + e);
  }

  // Everything above is attached by now. Saying so turns a frontend that dies
  // on its first line — a window that renders and ignores every click — from an
  // invisible failure into one line in the log.
  invoke("ui_ready", {
    what: "main window, listeners [" + wired.join(" ") + "]" +
      (failed.length ? " UNAVAILABLE: " + failed.join("; ") : ""),
  }).catch(() => {});
})();
