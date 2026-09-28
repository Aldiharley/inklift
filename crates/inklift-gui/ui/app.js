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
};

let loaded = false;
let output = "alpha";
let inkChoice = "";
let previewTimer = null, fullTimer = null, inFlight = false, queued = false;

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
    radius: Number(el.r.value) > 0 ? Number(el.r.value) : null,
    window: 12,
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
    el.ink.src = r.png;
    el.src.src = r.sourcePng;
    el.ink.hidden = false; el.src.hidden = false;
    el.empty.hidden = true; el.peekHint.hidden = false;
    el.save.disabled = false; el.copy.disabled = false;

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

/** Debounced so a dragged slider does not queue a job per pixel. */
function schedulePreview() {
  clearTimeout(previewTimer);
  clearTimeout(fullTimer);
  previewTimer = setTimeout(() => render(false), 60);
}

function adopt(info) {
  loaded = true;
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
  b.title = "Lifting from the screen needs X11; this build has no capture " +
            "backend for your platform yet. Open a file instead.";
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

/* ── peek: hold to compare, which is how you actually verify an alpha ──── */
const peek = (on) => loaded && el.ground.classList.toggle("peek", on);
el.ground.addEventListener("pointerdown", () => peek(true));
["pointerup", "pointerleave", "pointercancel"].forEach((ev) =>
  el.ground.addEventListener(ev, () => peek(false)));
document.addEventListener("keydown", (e) => {
  if (e.code === "Space" && !e.repeat && e.target === document.body) { e.preventDefault(); peek(true); }
  if (e.key === "Enter" && loaded && !e.ctrlKey) el.copy.click();
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
