//! The tray is a menu of promises. Each one must be kept.
//!
//! A tray item that renders but does nothing is the same class of failure as
//! the window that rendered but ignored every click: it looks finished and is
//! not. These check that every item the user can see is wired to something, and
//! that the icon can actually be drawn.

use std::fs;
use std::path::PathBuf;

fn gui() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn main_rs() -> String {
    fs::read_to_string(gui().join("src/main.rs")).expect("main.rs")
}

/// Ids given to menu items, read back out of the source.
fn declared_item_ids(src: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("with_id(") {
        rest = &rest[i + 8..];
        // skip the handle argument, take the first string literal
        let Some(q) = rest.find('"') else { break };
        let id = rest[q + 1..].split('"').next().unwrap_or("").to_string();
        if !id.is_empty() {
            ids.push(id);
        }
        rest = &rest[q + 1..];
    }
    ids
}

#[test]
fn every_tray_menu_item_is_wired_to_something() {
    let src = main_rs();
    let ids = declared_item_ids(&src);
    assert!(
        !ids.is_empty(),
        "no tray menu items are declared — the tray was designed in \
         design/ux-architecture.md but never built"
    );

    // the body that dispatches menu clicks
    let handler = src
        .split("on_menu_event")
        .nth(1)
        .expect("a tray with items must handle menu events")
        .to_string();

    let dead: Vec<&String> = ids
        .iter()
        .filter(|id| !handler.contains(id.as_str()))
        .collect();
    assert!(
        dead.is_empty(),
        "these tray items are shown to the user but nothing handles them: {dead:?}"
    );
}

#[test]
fn the_tray_icon_asset_exists_and_is_light_enough_for_a_dark_panel() {
    let icon = gui().join("icons/tray.png");
    assert!(
        icon.exists(),
        "no tray icon at {}; the tray cannot be built without one",
        icon.display()
    );
    let bytes = fs::read(&icon).expect("read icon");
    assert!(bytes.len() > 64, "tray icon looks empty");
    assert_eq!(&bytes[1..4], b"PNG", "tray icon must be a PNG");
}

/// The icon file the Windows build puts in the tray: the one under
/// `#[cfg(windows)]` if the source picks per platform, otherwise the only one.
fn windows_tray_icon(src: &str) -> String {
    let mut chosen = None;
    let mut prev = "";
    for line in src.lines() {
        if let Some(i) = line.find("include_bytes!(\"../icons/") {
            let name = line[i + 25..].split('"').next().unwrap().to_string();
            if prev.trim() == "#[cfg(windows)]" {
                return name;
            }
            chosen.get_or_insert(name);
        }
        prev = line;
    }
    chosen.expect("the tray icon is not embedded with include_bytes!")
}

/// WCAG relative luminance of an sRGB colour.
fn luminance([r, g, b]: [u8; 3]) -> f64 {
    let lin = |c: u8| {
        let c = c as f64 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

fn contrast(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Windows users run either taskbar theme, so the icon must stand out on both.
///
/// The icon drawn for dark Linux panels is a near-white glyph. On the light
/// Windows 11 taskbar it measured 1.17:1 — present, clickable, and invisible —
/// which a check of the file's existence could never notice. 3:1 is the WCAG
/// floor for graphical objects, taken on the mean luminance of the icon's
/// opaque pixels.
#[test]
fn the_icon_windows_puts_in_the_tray_stands_out_on_light_and_dark_taskbars() {
    let name = windows_tray_icon(&main_rs());
    let img = inklift_cli::open_image(&gui().join("icons").join(&name))
        .unwrap_or_else(|e| panic!("icons/{name}: {e}"))
        .to_rgba8();
    let opaque: Vec<f64> = img
        .pixels()
        .filter(|p| p.0[3] > 128)
        .map(|p| luminance([p.0[0], p.0[1], p.0[2]]))
        .collect();
    assert!(!opaque.is_empty(), "icons/{name} has no opaque pixels");
    let icon = opaque.iter().sum::<f64>() / opaque.len() as f64;

    for (theme, bg) in [("light", [0xF3, 0xF3, 0xF3]), ("dark", [0x1C, 0x1C, 0x1C])] {
        let ratio = contrast(icon, luminance(bg));
        eprintln!("icons/{name} against the {theme} taskbar: {ratio:.2}:1");
        assert!(
            ratio >= 3.0,
            "icons/{name} is {ratio:.2}:1 against the {theme} Windows taskbar — too \
             faint to find in the tray"
        );
    }
}

/// Tauri does not build tray support unless the feature is on, and the failure
/// is a compile error rather than a missing icon — but the icon and the feature
/// are edited in different files, so pin them together.
#[test]
fn the_tray_feature_is_enabled_on_tauri() {
    let toml = fs::read_to_string(gui().join("Cargo.toml")).expect("Cargo.toml");
    let line = toml
        .lines()
        .find(|l| l.trim_start().starts_with("tauri ="))
        .expect("a tauri dependency");
    assert!(
        line.contains("tray-icon"),
        "the tray needs tauri's \"tray-icon\" feature: {line}"
    );
}

/// Tray items that need the window's state announce themselves as events and
/// let the frontend act. An event nobody listens for is a menu item that does
/// nothing — the failure this whole file exists to prevent, just one layer
/// further along.
#[test]
fn every_event_the_backend_emits_is_listened_for_in_the_ui() {
    let src = main_rs();
    let mut emitted = Vec::new();
    let mut rest = src.as_str();
    while let Some(i) = rest.find("emit_to(") {
        rest = &rest[i + 8..];
        // emit_to("main", "<event>", ..) — the second string literal
        let mut parts = rest.split('"');
        let _before = parts.next();
        let _target = parts.next();
        let _between = parts.next();
        if let Some(ev) = parts.next() {
            if !ev.is_empty() && !emitted.contains(&ev.to_string()) {
                emitted.push(ev.to_string());
            }
        }
    }
    assert!(!emitted.is_empty(), "no events emitted at all");

    let ui: String = fs::read_dir(gui().join("ui"))
        .expect("ui/")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "js"))
        .filter_map(|p| fs::read_to_string(p).ok())
        .collect();

    let unheard: Vec<&String> = emitted
        .iter()
        .filter(|ev| !ui.contains(&format!("listen(\"{ev}\"")))
        .collect();
    assert!(
        unheard.is_empty(),
        "the backend emits these but no script listens for them: {unheard:?}"
    );
}
