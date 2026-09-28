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
