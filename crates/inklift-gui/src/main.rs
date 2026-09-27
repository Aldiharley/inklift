// Prevent a console window from opening alongside the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! inklift — desktop front end.
//!
//! The backend keeps the untouched source image in memory for the lifetime of a
//! result. That is what makes retuning free: every slider move re-extracts from
//! the original pixels rather than going back to the screen or the disk, which
//! is both faster and the only way a capture cannot be contaminated.

use std::sync::Mutex;

use base64::Engine;
use inklift_core::{Grid, Options, extract, looks_inverted, parse_ink_color};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// Longest edge of the preview proxy. Chosen against the measured budget:
/// roughly 150 ms for 900×500 on one core, which keeps a dragged slider fluid.
const PROXY_EDGE: u32 = 900;

/// The source image, held for the life of a result so retuning is free.
struct Source {
    image: image::RgbaImage,
}

#[derive(Default)]
struct App {
    source: Mutex<Option<Source>>,
}

/// What the UI sends back on every slider move.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Params {
    k: f32,
    min_area: usize,
    feather: usize,
    radius: Option<usize>,
    window: usize,
    invert: bool,
    /// `None` keeps the pen that was actually found.
    ink: Option<String>,
}

impl Params {
    fn options(&self) -> Options {
        Options {
            sauvola_k: self.k,
            min_area: self.min_area,
            feather: self.feather,
            background_radius: self.radius,
            sauvola_radius: self.window,
            invert: self.invert,
            ..Default::default()
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[derive(Clone)]
struct Loaded {
    label: String,
    width: u32,
    height: u32,
    region: Option<String>,
    /// True when the source looks light-on-dark. Advisory: the UI offers the
    /// flip, it is never applied behind the user's back.
    looks_inverted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Rendered {
    /// A PNG data URI of the extracted ink, alpha intact.
    png: String,
    /// The untouched source, for hold-to-compare.
    source_png: String,
    coverage: f32,
    ink_hex: String,
    width: u32,
    height: u32,
    /// True when this came from the proxy and a full-resolution pass is worth
    /// running. The UI shows "Refining…" rather than blanking the canvas.
    proxy: bool,
    ms: u64,
}

fn to_planes(img: &image::RgbaImage) -> [Grid; 3] {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut p = [Grid::new(w, h), Grid::new(w, h), Grid::new(w, h)];
    for (i, px) in img.pixels().enumerate() {
        // Transparency composites over white: clear is paper, not black.
        let a = px.0[3] as f32 / 255.0;
        for c in 0..3 {
            p[c].data_mut()[i] = a * (px.0[c] as f32 / 255.0) + (1.0 - a);
        }
    }
    p
}

fn png_data_uri(w: u32, h: u32, rgba: &[u8]) -> Result<String, String> {
    let buf = image::RgbaImage::from_raw(w, h, rgba.to_vec())
        .ok_or("buffer does not match dimensions")?;
    let mut bytes: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(buf)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| format!("could not encode PNG: {e}"))?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&bytes)
    ))
}

fn hex(rgb: [f32; 3]) -> String {
    let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02X}{:02X}{:02X}", c(rgb[0]), c(rgb[1]), c(rgb[2]))
}

fn adopt(state: &State<App>, image: image::RgbaImage, label: String, region: Option<String>) -> Loaded {
    let out = Loaded {
        label,
        width: image.width(),
        height: image.height(),
        region,
        looks_inverted: looks_inverted(&to_planes(&image)),
    };
    *state.source.lock().unwrap() = Some(Source { image });
    out
}

#[tauri::command]
fn open_file(path: String, state: State<App>) -> Result<Loaded, String> {
    // One implementation, shared with the CLI: it decides the format from the
    // file's bytes rather than its name.
    let img = inklift_cli::open_image(std::path::Path::new(&path)).map_err(|e| {
        // Both outcomes are logged for the same reason the pick path logs
        // its own: without a line here, "I opened a file and got an error"
        // leaves nothing to look at afterwards.
        eprintln!("inklift: could not open {path}: {e}");
        format!("could not open that image: {e}")
    })?;
    eprintln!("inklift: opened {path} ({} × {})", img.width(), img.height());
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".into());
    Ok(adopt(&state, img.to_rgba8(), name, None))
}

#[tauri::command]
fn capture(x: i32, y: i32, w: u32, h: u32, state: State<App>) -> Result<Loaded, String> {
    use inklift_shot::{Capturer, Rect, X11Capturer};
    let cap = X11Capturer::new()?;
    let region = Rect::new(x, y, w, h);
    let frame = cap.grab(&region)?;
    let img = image::RgbaImage::from_raw(frame.width(), frame.height(), frame.to_rgba8())
        .ok_or("capture did not produce a usable image")?;
    Ok(adopt(
        &state,
        img,
        format!("Screen region {w} × {h}"),
        Some(region.to_string()),
    ))
}

#[tauri::command]
fn screens() -> Result<Vec<String>, String> {
    use inklift_shot::{Capturer, X11Capturer};
    Ok(X11Capturer::new()?
        .monitors()?
        .into_iter()
        .map(|m| format!("{} {}", m.name, m.bounds))
        .collect())
}

/// A selection smaller than this on either axis is a misclick, not a capture.
const MIN_SELECTION: u32 = 8;

/// Let the user drag a region on the real screen.
///
/// Returns at once; the drag runs on its own thread and the result arrives as
/// a `picked` event. A blocking pointer grab must never run on the thread that
/// serves this command, because that is the GTK main thread: it would freeze
/// the event loop for the whole drag, and `hide()`/`show()` below dispatch
/// *through* that loop, so they would never be processed.
#[tauri::command]
fn begin_pick(app: tauri::AppHandle) -> Result<(), String> {
    let handle = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = run_pick(&handle) {
            eprintln!("inklift: pick failed: {e}");
            // A failure the user caused must reach the user.
            let _ = handle.emit_to("main", "pick-failed", e);
        }
    });
    Ok(())
}

/// Hide, select on the live screen, capture, restore.
///
/// The old design's rule was "capture first, show second" — a frozen frame the
/// overlay could not contaminate. This is the mirror of it: **hide first,
/// capture last**. Nothing paints a copy of the desktop, so the only thing that
/// could end up wrongly inside a capture is our own window, and it is hidden
/// before the grab and shown again on every path out.
fn run_pick(app: &tauri::AppHandle) -> Result<(), String> {
    use inklift_shot::{Capturer, Outcome, X11Capturer, pick_live_region, virtual_bounds};

    let cap = X11Capturer::new()?;
    let bounds = virtual_bounds(&cap.monitors()?).ok_or("no screens detected")?;

    let main = app.get_webview_window("main");
    if let Some(w) = &main {
        let _ = w.hide();
        // hide() returns once the event loop has queued the change, not once
        // the server has unmapped the window — and certainly not once the
        // desktop beneath has repainted. Two waits, because they are two
        // different unknowns: poll for the unmap, which is observable, then
        // allow a fixed settle for the repaint, which is not.
        //
        // The settle is measured, not guessed: on this machine a covered
        // region read clean 36-60 ms after an unmap, so 140 ms is roughly
        // twice the worst observed. Other tools land in the same range for the
        // same reason (scrot 80 ms, gnome-screenshot 200 ms with a comment
        // admitting there is no reliable signal to wait on instead).
        for _ in 0..100 {
            if !w.is_visible().unwrap_or(false) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(140));

    let outcome = pick_live_region(bounds, MIN_SELECTION);

    // Before anything can fail: a worker that dies with the window hidden looks
    // to the user like the app vanished.
    if let Some(w) = &main {
        let _ = w.show();
    }

    let rect = match outcome? {
        Outcome::Selected(rect) => rect,
        _ => {
            eprintln!("inklift: pick cancelled");
            return Ok(());
        }
    };

    let frame = cap.grab(&rect)?;
    let img = image::RgbaImage::from_raw(frame.width(), frame.height(), frame.to_rgba8())
        .ok_or("capture did not produce a usable image")?;

    let state = app.state::<App>();
    let loaded = adopt(
        &state,
        img,
        format!("Screen region {} × {}", rect.width, rect.height),
        Some(rect.to_string()),
    );
    // This emit IS the handoff: swallowing its error would lose a capture the
    // user already made, with nothing to show why.
    app.emit_to("main", "picked", &loaded)
        .map_err(|e| format!("could not hand the capture to the window: {e}"))?;
    if let Some(w) = &main {
        let _ = w.set_focus();
    }
    eprintln!("inklift: picked {rect}");
    Ok(())
}

/// Extract and hand back a picture. `full` skips the proxy.
#[tauri::command]
fn render(params: Params, full: bool, state: State<App>) -> Result<Rendered, String> {
    let started = std::time::Instant::now();
    let guard = state.source.lock().unwrap();
    let src = guard.as_ref().ok_or("nothing loaded yet")?;

    let long = src.image.width().max(src.image.height());
    let scale = if full || long <= PROXY_EDGE {
        1.0
    } else {
        PROXY_EDGE as f32 / long as f32
    };

    let working = if scale < 1.0 {
        image::imageops::resize(
            &src.image,
            (src.image.width() as f32 * scale).round().max(1.0) as u32,
            (src.image.height() as f32 * scale).round().max(1.0) as u32,
            image::imageops::FilterType::Triangle,
        )
    } else {
        src.image.clone()
    };

    // Every pixel-denominated parameter travels with the proxy, or the preview
    // stops predicting the result.
    let opts = params.options().scaled_for_proxy(scale);
    let mut result = extract(&to_planes(&working), &opts);
    if let Some(text) = &params.ink {
        result = result.with_ink_color(parse_ink_color(text)?);
    }

    let (w, h) = (working.width(), working.height());
    Ok(Rendered {
        png: png_data_uri(w, h, &result.to_rgba8())?,
        source_png: png_data_uri(w, h, working.as_raw())?,
        coverage: result.coverage(),
        ink_hex: hex(result.ink_color()),
        width: w,
        height: h,
        proxy: scale < 1.0,
        ms: started.elapsed().as_millis() as u64,
    })
}

fn finished(params: &Params, src: &Source) -> Result<(u32, u32, Vec<u8>, Vec<u8>, [f32; 3]), String> {
    let mut result = extract(&to_planes(&src.image), &params.options());
    if let Some(text) = &params.ink {
        result = result.with_ink_color(parse_ink_color(text)?);
    }
    Ok((
        src.image.width(),
        src.image.height(),
        result.to_rgba8(),
        result.to_gray_on_white8(),
        result.ink_color(),
    ))
}

#[tauri::command]
fn save(path: String, params: Params, white: bool, state: State<App>) -> Result<String, String> {
    let guard = state.source.lock().unwrap();
    let src = guard.as_ref().ok_or("nothing loaded yet")?;
    let (w, h, rgba, grey, _) = finished(&params, src)?;
    if white {
        image::GrayImage::from_raw(w, h, grey)
            .ok_or("greyscale buffer mismatch")?
            .save(&path)
    } else {
        image::RgbaImage::from_raw(w, h, rgba)
            .ok_or("RGBA buffer mismatch")?
            .save(&path)
    }
    .map_err(|e| format!("could not write {path}: {e}"))?;
    Ok(path)
}

/// Put the full-resolution result on the clipboard.
///
/// No holder process is needed here: the app is resident, so it owns the
/// selection itself for as long as it runs. That is the strongest argument for
/// the tray-resident shape.
#[tauri::command]
fn copy(params: Params, state: State<App>) -> Result<(), String> {
    let guard = state.source.lock().unwrap();
    let src = guard.as_ref().ok_or("nothing loaded yet")?;
    let (w, h, rgba, _, _) = finished(&params, src)?;
    inklift_shot::put_image(w, h, &rgba)
}

/// The file pickers live here rather than in JavaScript.
///
/// `withGlobalTauri` injects only tauri's own `bundle.global.js`; a plugin's
/// `api-iife.js` — the file that would define `window.__TAURI__.dialog` — is
/// never injected by anything in the Rust toolchain. Reaching for it from a
/// bundler-less frontend yields `undefined`, and the button fails the moment it
/// is pressed. Asking Rust keeps one version of the truth and needs no ACL
/// grant, since the app's own commands are not gated.
///
/// `command(async)` puts these on a worker thread: the blocking picker would
/// deadlock the event loop on the main one.
#[tauri::command(async)]
fn pick_open(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp"])
        .blocking_pick_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command(async)]
fn pick_save(app: tauri::AppHandle, default_name: String) -> Option<String> {
    app.dialog()
        .file()
        .set_file_name(&default_name)
        .add_filter("PNG", &["png"])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// The frontend calls this once every listener is attached. It exists because a
/// script that throws on its first line leaves a window that renders correctly
/// and ignores every click, with nothing anywhere to say so. One line in the
/// log is the difference between a five-minute fix and a debugging round.
#[tauri::command]
fn ui_ready(what: String) {
    eprintln!("inklift: {what} wired up");
}

fn main() {
    // WebKitGTK's DMABUF renderer hands back a surface that never paints under
    // virtualised or software GL: the window maps, shows its background, and
    // nothing else is ever composited. Measured on this machine, a WebKit window
    // came out 88.8% near-black with the renderer on and 0% with it off — image,
    // text and all. The selection overlay that first exposed this is gone, but
    // the main window is the same kind of surface and would fail the same way.
    //
    // Set before any GTK or WebKit call, and only when the user has expressed no
    // preference of their own.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: single-threaded here — this runs before the runtime, the
        // webview, and any thread this program spawns.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App::default())
        .invoke_handler(tauri::generate_handler![
            open_file, capture, screens, render, save, copy,
            begin_pick, pick_open, pick_save, ui_ready
        ])
        .setup(|app| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_title("inklift");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("inklift failed to start");
}
