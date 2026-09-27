//! The `shot` subcommand: capture a region of the screen and extract from it.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use inklift_core::Options;
use inklift_shot::Rect;

use crate::{Output, Result};

/// Below this, a drag is a misclick rather than a selection.
const MIN_SELECTION: u32 = 8;

pub const SHOT_USAGE: &str = "\
inklift shot - grab handwriting straight off the screen

USAGE:
    inklift shot [OPTIONS]

With no source flag, the screen freezes and you drag a box around the
handwriting. Esc cancels.

CAPTURE:
        --region X,Y,W,H  Skip the overlay and capture exactly this rectangle
        --full            Capture a whole screen
        --screen <N>      Which screen, with --full            [default: primary]
        --delay <SECS>    Wait before capturing, to open a menu first
        --keep-raw        Also save the untouched capture
        --no-clipboard    Do not copy the result to the clipboard
        --hold-secs <N>   How long to keep serving the clipboard   [default: 3600]

OUTPUT:
    -o, --output <PATH>   Output path            [default: shot-<timestamp>.png]
        --white           Ink on white instead of transparent
        --both            Write both exports

EXTRACTION (same meaning as the main command):
        --k <FLOAT>       Sauvola k                            [default: 0.20]
        --window <PX>     Sauvola window radius                [default: 12]
        --min-area <PX>   Discard components below this        [default: 8]
        --radius <PX>     Paper-estimate radius
        --feather <PX>    Soft-edge reach                      [default: 1]
        --invert          Ink is lighter than its background (dark themes)
        --ink <COLOUR>    Repaint the ink: #RRGGBB, #RGB, black or white
    -q, --quiet           Suppress the summary line
    -h, --help            Show this message

The captured region is printed to stdout as X,Y,W,H so it can be reused with
--region. Everything else goes to stderr.
";

/// Where the pixels come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShotSource {
    /// Freeze the screen and let the user drag a box.
    Interactive,
    /// A rectangle given on the command line.
    Region(Rect),
    /// A whole screen, by index, or the primary one.
    Full(Option<usize>),
}

#[derive(Clone, Debug)]
pub struct ShotConfig {
    pub source: ShotSource,
    pub output: PathBuf,
    pub mode: Output,
    pub options: Options,
    /// Override the pen colour the extractor found.
    pub ink: Option<[f32; 3]>,
    pub clipboard: bool,
    pub keep_raw: bool,
    pub delay_secs: u64,
    /// How long the detached holder keeps serving the clipboard. It also exits
    /// as soon as something else is copied, so this is only an upper bound.
    pub hold_secs: u64,
    pub quiet: bool,
}

/// `shot-YYYYMMDD-HHMMSS.png`, so a directory of captures sorts chronologically.
fn default_output_name() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d, hh, mm, ss) = civil_from_epoch(secs);
    format!("shot-{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}.png")
}

/// Split epoch seconds into a UTC civil date and time.
///
/// Howard Hinnant's days-from-civil algorithm, inverted. Done by hand to avoid
/// pulling a date library in for one filename.
pub fn civil_from_epoch(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as i64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, (rem / 3600) as u32, ((rem % 3600) / 60) as u32, (rem % 60) as u32)
}

pub fn parse_shot_args(argv: &[String]) -> Result<ShotConfig> {
    let mut source: Option<ShotSource> = None;
    let mut screen: Option<usize> = None;
    let mut full = false;
    let mut output: Option<PathBuf> = None;
    let mut mode = Output::Transparent;
    let mut options = Options::default();
    let mut ink: Option<[f32; 3]> = None;
    let mut clipboard = true;
    let mut keep_raw = false;
    let mut delay_secs = 0u64;
    let mut hold_secs = 3600u64;
    let mut quiet = false;

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        let mut value = |name: &str| -> Result<String> {
            i += 1;
            argv.get(i).cloned().ok_or_else(|| format!("{name} needs a value").into())
        };
        match arg {
            "-h" | "--help" => return Err(SHOT_USAGE.into()),
            "-q" | "--quiet" => quiet = true,
            "--white" => mode = Output::White,
            "--both" => mode = Output::Both,
            "--no-clipboard" => clipboard = false,
            "--keep-raw" => keep_raw = true,
            "--full" => full = true,
            "--screen" => screen = Some(value("--screen")?.parse()?),
            "--delay" => delay_secs = value("--delay")?.parse()?,
            "--hold-secs" => hold_secs = value("--hold-secs")?.parse()?,
            "--region" => {
                let text = value("--region")?;
                let rect: Rect = text
                    .parse()
                    .map_err(|e| format!("--region wants X,Y,W,H: {e}"))?;
                source = Some(ShotSource::Region(rect));
            }
            "-o" | "--output" => output = Some(PathBuf::from(value("--output")?)),
            "--k" => options.sauvola_k = value("--k")?.parse()?,
            "--window" => options.sauvola_radius = value("--window")?.parse()?,
            "--min-area" => options.min_area = value("--min-area")?.parse()?,
            "--feather" => options.feather = value("--feather")?.parse()?,
            "--invert" => options.invert = true,
            "--ink" => ink = Some(inklift_core::parse_ink_color(&value("--ink")?)?),
            "--radius" => options.background_radius = Some(value("--radius")?.parse()?),
            other => {
                return Err(format!("unknown option {other}\n\n{SHOT_USAGE}").into());
            }
        }
        i += 1;
    }

    if full {
        if source.is_some() {
            return Err("--full and --region are two different sources; pick one".into());
        }
        source = Some(ShotSource::Full(screen));
    } else if screen.is_some() && source.is_none() {
        return Err("--screen only applies with --full".into());
    }

    Ok(ShotConfig {
        source: source.unwrap_or(ShotSource::Interactive),
        output: output.unwrap_or_else(|| PathBuf::from(default_output_name())),
        mode,
        options,
        ink,
        clipboard,
        keep_raw,
        delay_secs,
        hold_secs,
        quiet,
    })
}

/// `<base>.raw.png` beside whatever the result is called.
pub fn raw_path(output: &Path) -> PathBuf {
    let stem = output.file_stem().map(|s| s.to_string_lossy().into_owned());
    let mut p = output.parent().map(PathBuf::from).unwrap_or_default();
    p.push(format!("{}.raw.png", stem.unwrap_or_else(|| "shot".into())));
    p
}

/// What a capture produced.
#[derive(Clone, Debug)]
pub struct ShotReport {
    /// The region actually captured, in global screen coordinates.
    pub region: Rect,
    pub written: Vec<PathBuf>,
    pub raw: Option<PathBuf>,
    pub coverage: f32,
    pub ink_color: [f32; 3],
    /// `None` when the clipboard was not asked for; `Some(Err)` when it was
    /// asked for and refused, which is reported but not fatal.
    pub clipboard: Option<std::result::Result<(), String>>,
}

/// A capture either happened or the user backed out. Backing out is a normal
/// outcome, not an error, and must not be reported as one.
#[derive(Clone, Debug)]
pub enum ShotOutcome {
    Captured(Box<ShotReport>),
    Cancelled,
}

impl ShotOutcome {
    /// # Panics
    /// If the capture was cancelled. For tests and for callers that supplied
    /// an explicit region, where cancelling is impossible.
    pub fn expect_captured(self) -> ShotReport {
        match self {
            ShotOutcome::Captured(r) => *r,
            ShotOutcome::Cancelled => panic!("capture was cancelled"),
        }
    }
}

/// Resolve where the pixels should come from, then capture and extract.
pub fn run_shot(config: &ShotConfig) -> Result<ShotOutcome> {
    use inklift_shot::{Capturer, X11Capturer};

    if config.delay_secs > 0 {
        std::thread::sleep(std::time::Duration::from_secs(config.delay_secs));
    }

    let capturer = X11Capturer::new()?;
    let monitors = capturer.monitors()?;

    let region = match config.source {
        ShotSource::Region(r) => r,
        ShotSource::Full(index) => {
            let monitor = match index {
                Some(i) => monitors
                    .get(i)
                    .ok_or_else(|| format!("no screen {i}; {} detected", monitors.len()))?,
                None => monitors
                    .iter()
                    .find(|m| m.primary)
                    .or_else(|| monitors.first())
                    .ok_or("no screens detected")?,
            };
            monitor.bounds
        }
        ShotSource::Interactive => {
            // The user drags on the real screen; nothing covers it, so there is
            // no overlay that could end up in its own screenshot. The capture
            // is taken afterwards, once the outline has been torn down.
            let bounds =
                inklift_shot::virtual_bounds(&monitors).ok_or("no screens detected")?;
            match inklift_shot::pick_live_region(bounds, MIN_SELECTION)? {
                inklift_shot::Outcome::Selected(rect) => rect,
                _ => return Ok(ShotOutcome::Cancelled),
            }
        }
    };

    let frame = capturer.grab(&region)?;

    // Only now that the capture succeeded do any files get created.
    let raw = if config.keep_raw {
        let path = raw_path(&config.output);
        crate::save_rgba(&path, frame.width() as usize, frame.height() as usize, &frame.to_rgba8())?;
        Some(path)
    } else {
        None
    };

    let planes = frame.to_planes();
    if !config.quiet && !config.options.invert && inklift_core::looks_inverted(&planes) {
        eprintln!("note: this capture looks light-on-dark; try --invert");
    }
    let mut result = inklift_core::extract(&planes, &config.options);
    if let Some(rgb) = config.ink {
        result = result.with_ink_color(rgb);
    }
    crate::warn_if_invisible(config.ink, config.mode, config.quiet);
    let (w, h) = (frame.width() as usize, frame.height() as usize);

    let mut written = Vec::new();
    match config.mode {
        Output::Transparent => {
            crate::save_rgba(&config.output, w, h, &result.to_rgba8())?;
            written.push(config.output.clone());
        }
        Output::White => {
            crate::save_gray_on_white(&config.output, w, h, &result.to_gray_on_white8())?;
            written.push(config.output.clone());
        }
        Output::Both => {
            let clear = with_shot_suffix(&config.output, ".ink.png");
            let white = with_shot_suffix(&config.output, ".white.png");
            crate::save_rgba(&clear, w, h, &result.to_rgba8())?;
            crate::save_gray_on_white(&white, w, h, &result.to_gray_on_white8())?;
            written.push(clear);
            written.push(white);
        }
    }

    let clipboard = config.clipboard.then(|| {
        let target = written.first().cloned().unwrap_or_else(|| config.output.clone());
        if inklift_shot::needs_holder() {
            // Setting the clipboard here and exiting would copy nothing: on
            // X11 and Wayland the owning process serves the data, so it has to
            // outlive this command.
            spawn_clipboard_holder(&target, config.hold_secs)
        } else {
            inklift_shot::put_image(w as u32, h as u32, &result.to_rgba8())
        }
    });

    Ok(ShotOutcome::Captured(Box::new(ShotReport {
        region,
        written,
        raw,
        coverage: result.coverage(),
        ink_color: result.ink_color(),
        clipboard,
    })))
}

fn with_shot_suffix(path: &Path, suffix: &str) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    let mut p = path.parent().map(PathBuf::from).unwrap_or_default();
    p.push(format!("{}{suffix}", stem.unwrap_or_else(|| "shot".into())));
    p
}

/// Re-invoke ourselves as a detached process whose only job is to own the
/// clipboard. It exits on its own once something else is copied, so repeated
/// captures retire each other rather than piling up.
pub fn spawn_clipboard_holder(image: &Path, hold_secs: u64) -> std::result::Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot find our own binary: {e}"))?;
    std::process::Command::new(exe)
        .arg("clipboard-hold")
        .arg(image)
        .arg("--hold-secs")
        .arg(hold_secs.to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not start the clipboard holder: {e}"))
}

/// The holder process: load the image, own the clipboard, serve until replaced.
pub fn run_clipboard_hold(argv: &[String]) -> Result<()> {
    let mut path: Option<PathBuf> = None;
    let mut hold_secs = 3600u64;
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--hold-secs" => {
                i += 1;
                hold_secs = argv
                    .get(i)
                    .ok_or("--hold-secs needs a value")?
                    .parse()
                    .map_err(|e| format!("--hold-secs: {e}"))?;
            }
            other if other.starts_with('-') => return Err(format!("unknown option {other}").into()),
            other if path.is_none() => path = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected argument {other}").into()),
        }
        i += 1;
    }
    let path = path.ok_or("clipboard-hold needs an image path")?;
    let (w, h, rgba) = crate::load_rgba(&path)?;
    inklift_shot::hold_image(w, h, &rgba, std::time::Duration::from_secs(hold_secs))?;
    Ok(())
}
