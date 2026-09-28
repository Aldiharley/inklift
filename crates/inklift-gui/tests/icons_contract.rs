//! The action buttons' icons: seven decorative PNGs generated as one sheet and
//! sliced by tools/make_icons.py. They are shown at 18 px and stored at 54 so
//! they stay sharp at 3x; a missing or opaque-cornered file shows up here
//! rather than as a broken image or a white box on the dark theme.

use std::fs;
use std::path::PathBuf;

/// Button id → icon file, in the order the sheet lays them out.
const ICONS: [(&str, &str); 7] = [
    ("openBtn", "open.png"),
    ("grabBtn", "lift.png"),
    ("eraserBtn", "eraser.png"),
    ("undoBtn", "undo.png"),
    ("clearBtn", "clear.png"),
    ("saveBtn", "save.png"),
    ("copyBtn", "copy.png"),
];

fn ui() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui")
}

#[test]
fn every_action_icon_is_a_square_transparent_png() {
    for (_, file) in ICONS {
        let path = ui().join("icons").join(file);
        let img = image::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(img.color().has_alpha(), "{file} has no alpha channel");
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        assert_eq!((w, h), (54, 54), "{file} is {w}x{h}; icons are stored at 54 px, 3x the 18 px they are shown at");
        for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
            assert_eq!(rgba.get_pixel(x, y)[3], 0, "{file} corner ({x},{y}) is not transparent");
        }
        let solid = rgba.pixels().filter(|p| p[3] > 200).count() as f32 / (w * h) as f32;
        assert!(solid > 0.15, "{file} is only {:.0}% solid: is anything drawn?", solid * 100.0);
    }
}

#[test]
fn the_generation_prompt_is_kept_with_the_slicer() {
    // The 2K sheet is not committed, so the prompt is what makes the set
    // repeatable; the slicer must exist beside it.
    let tools = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools");
    let prompt = fs::read_to_string(tools.join("icons-prompt.txt")).expect("tools/icons-prompt.txt");
    assert!(prompt.contains("clay"), "the prompt should name the clay style");
    assert!(tools.join("make_icons.py").is_file(), "tools/make_icons.py is missing");
}
