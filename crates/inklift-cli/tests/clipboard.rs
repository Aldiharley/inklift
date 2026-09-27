#![cfg(feature = "shot")]
//! Proves the clipboard survives the capturing process exiting.
//!
//! On X11 and Wayland the clipboard is not a buffer: the owning process serves
//! the data on request, so when a short-lived command exits it takes the
//! contents with it. These tests are the only thing standing between that and
//! a tool that cheerfully reports "copied to clipboard" and copies nothing.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn have_display() -> bool {
    std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok()
}

fn sample_png(path: &std::path::Path, w: u32, h: u32) {
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            rgba.extend_from_slice(&[(x % 256) as u8, (y % 256) as u8, 0x7f, 255]);
        }
    }
    inklift_cli::save_rgba(path, w as usize, h as usize, &rgba).unwrap();
}

/// Wait for the clipboard to carry an image of exactly this size.
///
/// Deliberately not "any image": there is one system clipboard, and a holder
/// left over from an earlier capture may still own it with different content.
/// Accepting whatever is there first would make this test fail for reasons
/// that have nothing to do with the code under test.
fn wait_for_clipboard_image(expect: (usize, usize), within: Duration) -> Option<(usize, usize)> {
    let deadline = Instant::now() + within;
    let mut last = None;
    while Instant::now() < deadline {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(img) = cb.get_image() {
                last = Some((img.width, img.height));
                if last == Some(expect) {
                    return last;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    last
}

#[test]
fn an_image_stays_on_the_clipboard_after_the_command_exits() {
    if !have_display() {
        eprintln!("skipped: no display");
        return;
    }
    let mut png = std::env::temp_dir();
    png.push(format!("inklift-clip-{}.png", std::process::id()));
    sample_png(&png, 48, 32);

    // Exactly what `shot` does: hand the file to a detached holder and leave.
    let mut child = Command::new(env!("CARGO_BIN_EXE_inklift"))
        .arg("clipboard-hold")
        .arg(&png)
        .arg("--hold-secs")
        .arg("20")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("holder should start");

    let got = wait_for_clipboard_image((48, 32), Duration::from_secs(10));
    let _ = child.kill();
    let _ = std::fs::remove_file(&png);

    assert_eq!(
        got,
        Some((48, 32)),
        "the image should still be on the clipboard while the holder runs"
    );
}

#[test]
fn the_holder_rejects_a_missing_file_instead_of_lingering() {
    let out = Command::new(env!("CARGO_BIN_EXE_inklift"))
        .arg("clipboard-hold")
        .arg("/definitely/not/here.png")
        .output()
        .expect("should run");

    assert!(!out.status.success(), "a missing file must fail, not hold forever");
}

#[test]
fn the_holder_needs_a_path() {
    let out = Command::new(env!("CARGO_BIN_EXE_inklift"))
        .arg("clipboard-hold")
        .output()
        .expect("should run");
    assert!(!out.status.success());
}
