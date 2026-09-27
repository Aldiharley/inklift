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

/// The clipboard carries RGBA, so the holder must read transparency back
/// rather than flattening it the way `load_rgb` deliberately does.
#[test]
fn rgba_can_be_loaded_with_its_transparency_intact() {
    let path = tmp("alpha-load.png");
    let (w, h) = (4usize, 2usize);
    let mut rgba = vec![0u8; w * h * 4];
    rgba[0..4].copy_from_slice(&[10, 20, 30, 255]);
    rgba[4..8].copy_from_slice(&[40, 50, 60, 128]);
    inklift_cli::save_rgba(&path, w, h, &rgba).unwrap();

    let (gw, gh, got) = inklift_cli::load_rgba(&path).unwrap();

    assert_eq!((gw, gh), (4, 2));
    assert_eq!(&got[0..4], &[10, 20, 30, 255]);
    assert_eq!(&got[4..8], &[40, 50, 60, 128], "alpha must survive");
    assert_eq!(got[3 + 8], 0, "a cleared pixel stays cleared");
    let _ = std::fs::remove_file(&path);
}

/// A file's contents decide what it is. Its name is a hint, and often a wrong
/// one: browsers routinely save WebP with a `.jpg` or `.jfif` name, and that is
/// precisely the file a user drops on this tool.
///
/// `image::open` picks its decoder from the path extension, so such a file went
/// to the JPEG decoder and came back as
/// `Format error decoding Jpeg: Error parsing image. Illegal start bytes:5249`
/// — 0x5249 being "RI", the start of a RIFF/WebP container. Unreadable to a
/// user, and wrong: the file is a perfectly good image.
#[test]
fn a_png_named_jpg_loads_anyway() {
    let path = tmp("misnamed.jpg");
    let (w, h) = (6usize, 4usize);
    let rgba: Vec<u8> = (0..w * h).flat_map(|_| [200u8, 100, 50, 255]).collect();
    // written as a PNG, named .jpg
    save_rgba(&path.with_extension("png"), w, h, &rgba).expect("write png");
    std::fs::rename(path.with_extension("png"), &path).expect("rename to .jpg");

    let planes = load_rgb(&path).expect("a PNG must load whatever it is called");
    assert_eq!(planes[0].width(), w);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_webp_named_jpg_loads_anyway() {
    let path = tmp("misnamed-webp.jpg");
    let (w, h) = (8u32, 6u32);
    let rgb: Vec<u8> = (0..w * h).flat_map(|_| [30u8, 60, 90]).collect();
    let mut bytes: Vec<u8> = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut std::io::Cursor::new(&mut bytes))
        .encode(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .expect("encode webp");
    assert_eq!(&bytes[..2], b"RI", "fixture must really be a RIFF container");
    std::fs::write(&path, &bytes).expect("write");

    let planes = load_rgb(&path).expect("a WebP must load even when named .jpg");
    assert_eq!(planes[0].width(), w as usize);
    let _ = std::fs::remove_file(&path);
}

/// The file picker offers these extensions, so the build must be able to read
/// them. Advertising a format the decoder was not compiled with is a promise
/// the app cannot keep.
#[test]
fn every_format_the_picker_offers_can_actually_be_decoded() {
    use image::ImageFormat::*;
    for (fmt, name) in [(Png, "png"), (Jpeg, "jpeg"), (WebP, "webp"), (Bmp, "bmp"), (Tiff, "tiff")] {
        assert!(
            fmt.reading_enabled(),
            "the picker offers .{name} but this build cannot decode it"
        );
    }
}

/// A file that is not an image at all must say so.
///
/// `with_guessed_format` keeps the extension's guess when the bytes identify
/// nothing, so a text file named `.jpg` still reached the JPEG decoder and came
/// back as `Illegal start bytes:3C3F`. That is the decoder's internal
/// vocabulary, not an explanation: 0x3C3F is "<?", the start of a PHP file.
#[test]
fn a_file_that_is_not_an_image_says_so_plainly() {
    let path = tmp("not-an-image.jpg");
    std::fs::write(&path, b"<?php echo 'hello'; ?>").expect("write");

    let err = load_rgb(&path).expect_err("a PHP file is not an image").to_string();
    let _ = std::fs::remove_file(&path);

    assert!(
        !err.contains("Illegal start bytes"),
        "the decoder's internal error reached the user: {err}"
    );
    assert!(
        err.to_lowercase().contains("image"),
        "the message should tell the user it is not a readable image: {err}"
    );
}

/// `image::open` is banned in this workspace.
///
/// It chooses a decoder from the path extension, which is a claim made by
/// whoever named the file rather than by the file. That reads the wrong
/// decoder for the WebP-named-`.jpg` files browsers produce, and reports the
/// failure in the decoder's private vocabulary. `open_image` above is the one
/// way in.
#[test]
fn nothing_in_the_workspace_calls_image_open() {
    // Built at runtime so this test does not match its own source.
    let needle = format!("image{}open(", "::");

    fn walk(dir: &std::path::Path, needle: &str, hits: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.filter_map(std::result::Result::ok) {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n == "target" || n == ".git") {
                    continue;
                }
                walk(&p, needle, hits);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let Ok(body) = std::fs::read_to_string(&p) else { continue };
                for (i, line) in body.lines().enumerate() {
                    if line.contains(needle) && !line.trim_start().starts_with("//") {
                        hits.push(format!("{}:{}", p.display(), i + 1));
                    }
                }
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap().join("crates");
    let mut hits = Vec::new();
    walk(&root, &needle, &mut hits);
    assert!(
        hits.is_empty(),
        "image::open picks its decoder from the filename; use open_image instead: {hits:?}"
    );
}
