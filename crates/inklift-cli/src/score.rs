//! Scoring harness: run the DIBCO measures over a directory of results.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use inklift_core::{Mask, Scores, score};

use crate::{Result, load_rgb};

pub const SCORE_USAGE: &str = "\
inklift-score - score binarization results with the DIBCO measures

USAGE:
    inklift-score <RESULTS_DIR> <GROUND_TRUTH_DIR> [OPTIONS]

Files are paired by filename, ignoring case and a trailing _GT / -gt suffix.

OPTIONS:
        --csv <PATH>       Also write per-image scores as CSV
        --threshold <0-255> Grey level below which a pixel counts as ink  [default: 128]
        --invert           Treat light pixels as ink instead of dark
        --resize           Resample results to the ground-truth size first.
                           Needed for hosted models, which return their own size.
    -h, --help             Show this message
";

#[derive(Clone, Debug)]
pub struct ScoreConfig {
    pub results: PathBuf,
    pub truth: PathBuf,
    pub threshold: u8,
    pub invert: bool,
    pub csv: Option<PathBuf>,
    /// Resample results to the ground-truth size before scoring. Needed for
    /// hosted models, which do not preserve dimensions.
    pub resize: bool,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub name: String,
    pub scores: Scores,
}

#[derive(Clone, Debug)]
pub struct Summary {
    pub rows: Vec<Row>,
    pub mean_fm: f32,
    pub mean_p_fm: f32,
    /// Mean over images that were not reproduced exactly; an exact match has
    /// infinite PSNR and would swallow the average.
    pub mean_psnr: f32,
    pub exact_matches: usize,
    pub mean_drd: f32,
    /// Results with no matching ground truth, reported rather than dropped.
    pub unmatched: Vec<String>,
}

/// Strip a trailing ground-truth marker and lower-case, so `H01_GT` pairs with `H01`.
pub fn normalize_stem(stem: &str) -> String {
    let lower = stem.to_ascii_lowercase();
    for suffix in ["_gt", "-gt", ".gt", "_groundtruth", "-groundtruth"] {
        if let Some(base) = lower.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    lower
}

/// Read an image as an ink mask. Ink is dark unless `invert` says otherwise.
pub fn load_mask(path: &Path, threshold: u8, invert: bool) -> Result<Mask> {
    let planes = load_rgb(path)?;
    let (w, h) = (planes[0].width(), planes[0].height());
    let cut = threshold as f32 / 255.0;
    let mut mask = Mask::new(w, h);
    for i in 0..mask.len() {
        let grey =
            (planes[0].data()[i] + planes[1].data()[i] + planes[2].data()[i]) / 3.0;
        mask.data_mut()[i] = if invert { grey >= cut } else { grey < cut };
    }
    Ok(mask)
}

/// Nearest-neighbour resampling. Deliberately not interpolating: the mask is
/// already a decision, and blending would invent intermediate values.
fn resample_nearest(mask: &Mask, width: usize, height: usize) -> Mask {
    let mut out = Mask::new(width, height);
    if mask.is_empty() || width == 0 || height == 0 {
        return out;
    }
    for y in 0..height {
        let sy = (y * mask.height() / height).min(mask.height() - 1);
        for x in 0..width {
            let sx = (x * mask.width() / width).min(mask.width() - 1);
            out.set(x, y, mask.get(sx, sy));
        }
    }
    out
}

fn index_images(dir: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut out = HashMap::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let is_image = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tif" | "tiff"))
            .unwrap_or(false);
        if !is_image {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.insert(normalize_stem(stem), path);
        }
    }
    Ok(out)
}

/// Score every result that has a matching ground truth.
pub fn run_score(config: &ScoreConfig) -> Result<Summary> {
    let results = index_images(&config.results)?;
    let truths = index_images(&config.truth)?;

    let mut names: Vec<&String> = results.keys().collect();
    names.sort();

    let mut rows = Vec::new();
    let mut unmatched = Vec::new();
    for name in names {
        let Some(gt_path) = truths.get(name) else {
            unmatched.push(name.clone());
            continue;
        };
        let mut predicted = load_mask(&results[name], config.threshold, config.invert)?;
        let truth = load_mask(gt_path, config.threshold, config.invert)?;
        if config.resize && !predicted.same_shape_as(&truth) {
            predicted = resample_nearest(&predicted, truth.width(), truth.height());
        }
        if predicted.len() != truth.len() {
            return Err(format!(
                "{name}: result is {}x{} but ground truth is {}x{}",
                predicted.width(),
                predicted.height(),
                truth.width(),
                truth.height()
            )
            .into());
        }
        rows.push(Row { name: name.clone(), scores: score(&predicted, &truth) });
    }

    let n = rows.len().max(1) as f32;
    let finite: Vec<f32> = rows.iter().map(|r| r.scores.psnr).filter(|v| v.is_finite()).collect();
    let summary = Summary {
        mean_fm: rows.iter().map(|r| r.scores.fm).sum::<f32>() / n,
        mean_p_fm: rows.iter().map(|r| r.scores.p_fm).sum::<f32>() / n,
        mean_psnr: if finite.is_empty() {
            f32::INFINITY
        } else {
            finite.iter().sum::<f32>() / finite.len() as f32
        },
        exact_matches: rows.len() - finite.len(),
        mean_drd: rows.iter().map(|r| r.scores.drd).sum::<f32>() / n,
        rows,
        unmatched,
    };

    if let Some(path) = &config.csv {
        let mut text = String::from("image,fm,p_fm,psnr,drd\n");
        for row in &summary.rows {
            text.push_str(&format!(
                "{},{:.4},{:.4},{:.4},{:.4}\n",
                row.name, row.scores.fm, row.scores.p_fm, row.scores.psnr, row.scores.drd
            ));
        }
        std::fs::write(path, text)?;
    }

    Ok(summary)
}

/// Parse arguments for the scoring binary, excluding the program name.
pub fn parse_score_args(argv: &[String]) -> Result<ScoreConfig> {
    let mut positional: Vec<PathBuf> = Vec::new();
    let mut threshold = 128u8;
    let mut invert = false;
    let mut resize = false;
    let mut csv = None;

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        let mut value = |name: &str| -> Result<String> {
            i += 1;
            argv.get(i).cloned().ok_or_else(|| format!("{name} needs a value").into())
        };
        match arg {
            "-h" | "--help" => return Err(SCORE_USAGE.into()),
            "--invert" => invert = true,
            "--resize" => resize = true,
            "--csv" => csv = Some(PathBuf::from(value("--csv")?)),
            "--threshold" => threshold = value("--threshold")?.parse()?,
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n\n{SCORE_USAGE}").into());
            }
            other => positional.push(PathBuf::from(other)),
        }
        i += 1;
    }

    if positional.len() != 2 {
        return Err(format!(
            "expected a results directory and a ground-truth directory\n\n{SCORE_USAGE}"
        )
        .into());
    }
    Ok(ScoreConfig {
        results: positional[0].clone(),
        truth: positional[1].clone(),
        threshold,
        invert,
        csv,
        resize,
    })
}
