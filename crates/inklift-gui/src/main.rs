// Prevent a console window from opening alongside the app on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! inklift — desktop front end.
//!
//! The backend keeps the untouched source image in memory for the lifetime of a
//! result. That is what makes retuning free: every slider move re-extracts from
//! the original pixels rather than going back to the screen or the disk, which
//! is both faster and the only way the overlay cannot contaminate a capture.

use std::sync::Mutex;

use base64::Engine;
use inklift_core::{Grid, Options, extract, looks_inverted, parse_ink_color};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

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
    let img = image::open(&path).map_err(|e| format!("could not open that image: {e}"))?;
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

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App::default())
        .invoke_handler(tauri::generate_handler![
            open_file, capture, screens, render, save, copy
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
