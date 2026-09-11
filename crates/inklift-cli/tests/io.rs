use inklift_cli::{load_rgb, save_gray_on_white, save_rgba};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("inklift-test-{}-{name}", std::process::id()));
    p
}

/// Pins a deliberate decision: pixels are processed in the space the file is
/// stored in, with no gamma conversion. Illumination division cancels a
/// multiplicative light field in either space, and viewers composite alpha in
/// sRGB, so deriving alpha here makes it correct where it is actually used.
#[test]
fn pixel_values_pass_through_without_gamma_adjustment() {
    let path = tmp("grey.png");
    let (w, h) = (4usize, 4usize);
    let rgba: Vec<u8> = (0..w * h).flat_map(|_| [128u8, 128, 128, 255]).collect();
    save_rgba(&path, w, h, &rgba).expect("write");

    let planes = load_rgb(&path).expect("read");

    assert!(
        (planes[0].get(1, 1) - 128.0 / 255.0).abs() < 1e-4,
        "value was transformed to {}",
        planes[0].get(1, 1)
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_written_png_can_be_read_back_as_planes() {
    let path = tmp("roundtrip.png");
    let (w, h) = (32usize, 16usize);
    let mut rgba = vec![255u8; w * h * 4];
    for y in 0..h {
        for c in 0..3 {
            rgba[(y * w + 8) * 4 + c] = 0;
        }
    }
    save_rgba(&path, w, h, &rgba).expect("write");

    let planes = load_rgb(&path).expect("read");

    assert_eq!((planes[0].width(), planes[0].height()), (w, h));
    assert!(planes[0].get(8, 4) < 0.05, "the dark column did not survive");
    assert!(planes[0].get(20, 4) > 0.95, "the white field did not survive");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_missing_file_is_an_error_not_a_panic() {
    assert!(load_rgb(&tmp("definitely-not-here.png")).is_err());
}

#[test]
fn extraction_writes_a_transparent_png_and_a_white_one() {
    let src = tmp("page.png");
    let out_rgba = tmp("page-alpha.png");
    let out_white = tmp("page-white.png");
    let (w, h) = (96usize, 48usize);

    // A page with a lighting gradient and a dark horizontal stroke.
    let mut rgba = vec![255u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let shade = 235u8 - (x * 30 / w) as u8;
            let dark = (22..26).contains(&y) && (8..88).contains(&x);
            let v = if dark { 30 } else { shade };
            for c in 0..3 {
                rgba[(y * w + x) * 4 + c] = v;
            }
        }
    }
    save_rgba(&src, w, h, &rgba).expect("write source");

    let planes = load_rgb(&src).expect("read source");
    let result = inklift_core::extract(&planes, &inklift_core::Options::default());
    save_rgba(&out_rgba, w, h, &result.to_rgba8()).expect("write rgba");
    save_gray_on_white(&out_white, w, h, &result.to_gray_on_white8()).expect("write white");

    let reread = load_rgb(&out_white).expect("read white");
    assert!(reread[0].get(50, 24) < 0.6, "stroke missing from the white export");
    assert!(reread[0].get(50, 5) > 0.9, "paper was not white in the white export");

    for p in [src, out_rgba, out_white] {
        let _ = std::fs::remove_file(p);
    }
}
