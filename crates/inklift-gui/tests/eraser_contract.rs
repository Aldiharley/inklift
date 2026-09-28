//! The eraser only works if every output applies it. Save, Copy and the tray's
//! "Copy again" re-extract in Rust rather than exporting what the window shows,
//! so an output path that skipped the strokes would quietly bring the erased
//! marks back in exactly the file the user keeps. These are source checks: the
//! GUI is a binary with no library for a test to call into.

use std::fs;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The body of `fn name(` in main.rs, up to its closing brace at column 0.
fn body_of(src: &str, name: &str) -> String {
    src.split(&format!("fn {name}("))
        .nth(1)
        .and_then(|s| s.split("\n}").next())
        .unwrap_or_else(|| panic!("main.rs must define fn {name}"))
        .to_string()
}

#[test]
fn every_output_applies_the_erasing() {
    let main_rs = read("src/main.rs");
    assert!(
        body_of(&main_rs, "apply_erasing").contains("keep_mask("),
        "apply_erasing must rasterise the strokes"
    );
    for f in ["render", "finished"] {
        assert!(
            body_of(&main_rs, f).contains("apply_erasing("),
            "fn {f} does not apply the erasing, so its output would bring erased marks back"
        );
    }
    for f in ["save", "copy"] {
        assert!(
            body_of(&main_rs, f).contains("finished("),
            "fn {f} must export through finished(), which applies the erasing"
        );
    }
}

#[test]
fn loading_a_new_image_starts_with_no_erasing() {
    let body = body_of(&read("src/main.rs"), "adopt");
    assert!(
        body.contains("strokes: Vec::new()"),
        "adopt must reset the strokes: they are in the old image's pixels"
    );
}

#[test]
fn the_eraser_commands_are_registered() {
    let main_rs = read("src/main.rs");
    let handler = main_rs
        .split("generate_handler![")
        .nth(1)
        .and_then(|s| s.split(']').next())
        .expect("main.rs must register a command handler");
    for cmd in ["erase_stroke", "undo_erase", "clear_erase"] {
        assert!(handler.contains(cmd), "{cmd} is not registered");
    }
}

#[test]
fn the_ui_calls_every_eraser_command() {
    let app = read("ui/app.js");
    for cmd in ["erase_stroke", "undo_erase", "clear_erase"] {
        assert!(
            app.contains(&format!("invoke(\"{cmd}\"")),
            "{cmd} is registered but the UI never calls it"
        );
    }
}

#[test]
fn every_eraser_control_exists_and_is_wired() {
    let html = read("ui/index.html");
    let app = read("ui/app.js");
    for id in ["eraserBtn", "eSize", "eSoft", "undoBtn", "clearBtn", "brush"] {
        assert!(html.contains(&format!("id=\"{id}\"")), "index.html has no #{id}");
        assert!(
            app.contains(&format!("$(\"{id}\")")),
            "app.js never looks up #{id}, so the control does nothing"
        );
    }
    assert!(
        html.contains("<canvas id=\"ink\""),
        "the result must be a canvas: an <img> cannot show a stroke while it is being dragged"
    );
}
