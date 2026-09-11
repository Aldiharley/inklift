use inklift_cli::{Output, parse_args, run};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("inklift-run-{}-{name}", std::process::id()));
    p
}

fn write_page(path: &PathBuf) -> (usize, usize) {
    let (w, h) = (96usize, 48usize);
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
    inklift_cli::save_rgba(path, w, h, &rgba).expect("write source");
    (w, h)
}

#[test]
fn run_writes_the_requested_output() {
    let src = tmp("a.png");
    let dst = tmp("a-out.png");
    write_page(&src);
    let cfg = parse_args(&[
        src.to_string_lossy().into_owned(),
        "-o".into(),
        dst.to_string_lossy().into_owned(),
        "-q".into(),
    ])
    .unwrap();

    let report = run(&cfg).expect("run should succeed");

    assert!(dst.exists(), "output was not written");
    assert!(report.coverage > 0.0 && report.coverage < 0.5);
    assert_eq!(report.written.len(), 1);
    for p in [src, dst] {
        let _ = std::fs::remove_file(p);
    }
}

#[test]
fn both_mode_writes_two_files() {
    let src = tmp("b.png");
    write_page(&src);
    let mut cfg = parse_args(&[src.to_string_lossy().into_owned(), "-q".into()]).unwrap();
    cfg.mode = Output::Both;

    let report = run(&cfg).expect("run should succeed");

    assert_eq!(report.written.len(), 2, "expected a transparent and a white export");
    for p in report.written {
        assert!(p.exists(), "{p:?} was reported but not written");
        let _ = std::fs::remove_file(p);
    }
    let _ = std::fs::remove_file(src);
}

#[test]
fn a_missing_input_reports_an_error() {
    let cfg = parse_args(&[tmp("nope.png").to_string_lossy().into_owned()]).unwrap();
    assert!(run(&cfg).is_err());
}
