// The selection overlay.
//
// Deliberately thin. It draws the frozen screen, tracks a drag, and hands two
// raw corners to Rust. Every rule about what that drag *means* — normalising
// the direction, trimming to the screen, treating a sliver as a misclick —
// lives in inklift_shot::resolve_pick, where it is unit-tested. Reimplementing
// any of it here would mean two versions of the truth.

const { invoke } = window.__TAURI__.core;

const frame = document.getElementById("frame");
const box = document.getElementById("box");
const size = document.getElementById("size");
const help = document.getElementById("help");
const shade = {
  top: document.getElementById("top"),
  bottom: document.getElementById("bottom"),
  left: document.getElementById("left"),
  right: document.getElementById("right"),
};

let origin = { x: 0, y: 0, width: 0, height: 0 };
let anchor = null;
let done = false;

/** Window coordinates are local; the backend thinks in screen coordinates. */
const toScreen = (x, y) => ({ x: origin.x + Math.round(x), y: origin.y + Math.round(y) });

function shadeAll() {
  shade.top.style.cssText = "top:0;height:100%";
  shade.bottom.style.cssText = "display:none";
  shade.left.style.cssText = "display:none";
  shade.right.style.cssText = "display:none";
}

/** Dim everything except the selection, using four panes so the selected
 *  pixels are never composited over. */
function shadeAround(l, t, w, h) {
  const W = window.innerWidth, H = window.innerHeight;
  shade.top.style.cssText = `top:0;left:0;width:100%;height:${t}px`;
  shade.bottom.style.cssText = `top:${t + h}px;left:0;width:100%;height:${Math.max(0, H - t - h)}px`;
  shade.left.style.cssText = `top:${t}px;left:0;width:${l}px;height:${h}px`;
  shade.right.style.cssText = `top:${t}px;left:${l + w}px;width:${Math.max(0, W - l - w)}px;height:${h}px`;
}

function draw(x, y) {
  const l = Math.min(anchor.x, x), t = Math.min(anchor.y, y);
  const w = Math.abs(x - anchor.x), h = Math.abs(y - anchor.y);
  box.style.display = "block";
  box.style.cssText += `;left:${l}px;top:${t}px;width:${w}px;height:${h}px`;
  shadeAround(l, t, w, h);

  size.textContent = `${Math.round(w)} × ${Math.round(h)}`;
  size.style.display = "block";
  // keep the readout on screen when dragging toward an edge
  const sx = Math.min(l + w + 9, window.innerWidth - 92);
  const sy = Math.min(Math.max(t + h + 9, 4), window.innerHeight - 28);
  size.style.left = `${Math.max(4, sx)}px`;
  size.style.top = `${sy}px`;
}

async function cancel() {
  if (done) return;
  done = true;
  try { await invoke("cancel_pick"); } catch (_) { /* the window is closing anyway */ }
}

async function finish(x, y) {
  if (done || !anchor) return;
  done = true;
  const a = toScreen(anchor.x, anchor.y);
  const b = toScreen(x, y);
  try {
    await invoke("finish_pick", { x0: a.x, y0: a.y, x1: b.x, y1: b.y });
  } catch (e) {
    console.error(e);
    await invoke("cancel_pick").catch(() => {});
  }
}

window.addEventListener("pointerdown", (e) => {
  if (e.button === 2) { cancel(); return; }
  if (e.button !== 0) return;
  anchor = { x: e.clientX, y: e.clientY };
  help.classList.add("gone");
  draw(e.clientX, e.clientY);
});
window.addEventListener("pointermove", (e) => { if (anchor) draw(e.clientX, e.clientY); });
window.addEventListener("pointerup", (e) => { if (e.button === 0) finish(e.clientX, e.clientY); });
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => { if (e.key === "Escape") cancel(); });
// If the overlay loses focus something else has taken over; do not linger.
window.addEventListener("blur", () => { if (!anchor) cancel(); });

(async () => {
  try {
    const o = await invoke("overlay_frame");
    origin = { x: o.x, y: o.y, width: o.width, height: o.height };
    frame.src = o.png;
    shadeAll();
  } catch (e) {
    console.error(e);
    cancel();
  }
})();
