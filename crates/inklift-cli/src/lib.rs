//! File I/O and the command-line front end for `inklift-core`.
//!
//! Pixels are handed to the core in the space the file stores them in, with no
//! gamma conversion. Dividing by an estimated background cancels a
//! multiplicative light field in either space, and image viewers composite
//! alpha on sRGB-encoded values, so opacity derived here is correct in the
//! tools that will consume it.

#[cfg(feature = "shot")]
mod shot;
#[cfg(feature = "shot")]
pub use shot::{
    SHOT_USAGE, ShotConfig, ShotOutcome, ShotReport, ShotSource, run_clipboard_hold,
    spawn_clipboard_holder, civil_from_epoch, parse_shot_args, raw_path,
    run_shot,
};

mod dotenv;
pub use dotenv::{env_or, parse_env_file, read_env_file};

mod score;
pub use score::{
    SCORE_USAGE, ScoreConfig, Row, Summary, load_mask, normalize_stem, parse_score_args, run_score,
};

use std::path::Path;

use inklift_core::Grid;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Read an image as three `[0, 1]` channel planes.
///
/// Transparency is composited over white rather than discarded. Simply
/// dropping the alpha channel leaves cleared pixels holding whatever RGB sat
/// underneath - usually black - which then reads as solid ink. Treating clear
/// as paper is both what a viewer shows and what the pipeline expects.
pub fn load_rgb(path: &Path) -> Result<[Grid; 3]> {
    let img = image::open(path)?.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut planes = [Grid::new(w, h), Grid::new(w, h), Grid::new(w, h)];
    for (i, px) in img.pixels().enumerate() {
        let a = px.0[3] as f32 / 255.0;
        for c in 0..3 {
            let v = px.0[c] as f32 / 255.0;
            planes[c].data_mut()[i] = a * v + (1.0 - a);
        }
    }
    Ok(planes)
}

/// Read an image as straight RGBA, transparency intact.
///
/// The counterpart to [`load_rgb`], which deliberately flattens onto white
/// because the extraction pipeline wants paper, not holes. The clipboard wants
/// the holes.
pub fn load_rgba(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let img = image::open(path)?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    Ok((w, h, img.into_raw()))
}

/// Write straight (non-premultiplied) RGBA as a PNG.
pub fn save_rgba(path: &Path, width: usize, height: usize, rgba: &[u8]) -> Result<()> {
    let buf = image::RgbaImage::from_raw(width as u32, height as u32, rgba.to_vec())
        .ok_or("RGBA buffer does not match the given dimensions")?;
    buf.save(path)?;
    Ok(())
}

/// Write a single-channel greyscale image as a PNG.
pub fn save_gray_on_white(path: &Path, width: usize, height: usize, gray: &[u8]) -> Result<()> {
    let buf = image::GrayImage::from_raw(width as u32, height as u32, gray.to_vec())
        .ok_or("greyscale buffer does not match the given dimensions")?;
    buf.save(path)?;
    Ok(())
}

use std::path::PathBuf;

use inklift_core::Options;

#[cfg(feature = "api")]
pub const USAGE: &str = "\
inklift - lift handwriting off a photograph

USAGE:
    inklift <IMAGE> [OPTIONS]

OPTIONS:
    -o, --output <PATH>   Where to write the result   [default: <IMAGE>.ink.png]
        --white           Ink on a white background instead of transparent
        --both            Write both exports, suffixed .ink.png and .white.png
        --k <FLOAT>       Sauvola k; raise it to keep less faint ink  [default: 0.20]
        --window <PX>     Sauvola window radius                       [default: 12]
        --min-area <PX>   Discard connected components below this     [default: 8]
        --radius <PX>     Paper-estimate radius; must exceed the stroke half-width
        --feather <PX>    How far soft edges reach past the stroke    [default: 1]
        --invert          The ink is lighter than its background, as in a
                          screenshot of a dark-themed application
        --ink <COLOUR>    Repaint the ink: #RRGGBB, #RGB, black or white.
                          Use --ink white to paste onto a dark slide.
    -q, --quiet           Suppress the summary line
    -h, --help            Show this message

HOSTED MODEL (opt-in, sends the image to a third party):
        --via <NAME>      gemini or openai. Off unless given.
        --model <ID>      Override the provider's default model
        --prompt <TEXT>   Override the instruction sent with the image
        --api-key <KEY>   Override the provider's environment variable
        --timeout <SECS>  Request timeout                          [default: 120]
";

#[cfg(not(feature = "api"))]
pub const USAGE: &str = "\
inklift - lift handwriting off a photograph

USAGE:
    inklift <IMAGE> [OPTIONS]

OPTIONS:
    -o, --output <PATH>   Where to write the result   [default: <IMAGE>.ink.png]
        --white           Ink on a white background instead of transparent
        --both            Write both exports, suffixed .ink.png and .white.png
        --k <FLOAT>       Sauvola k; raise it to keep less faint ink  [default: 0.20]
        --window <PX>     Sauvola window radius                       [default: 12]
        --min-area <PX>   Discard connected components below this     [default: 8]
        --radius <PX>     Paper-estimate radius; must exceed the stroke half-width
        --feather <PX>    How far soft edges reach past the stroke    [default: 1]
        --invert          The ink is lighter than its background, as in a
                          screenshot of a dark-themed application
        --ink <COLOUR>    Repaint the ink: #RRGGBB, #RGB, black or white.
                          Use --ink white to paste onto a dark slide.
    -q, --quiet           Suppress the summary line
    -h, --help            Show this message
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Transparent,
    White,
    Both,
}

/// Settings for the opt-in hosted-model path.
#[cfg(feature = "api")]
#[derive(Clone, Debug)]
pub struct ViaConfig {
    pub provider: inklift_api::Provider,
    pub model: String,
    pub prompt: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub input: PathBuf,
    pub output: PathBuf,
    pub mode: Output,
    /// True when `output` was derived rather than asked for. `--both` needs to
    /// know, because it writes two files and must base their names on an
    /// explicit path when one was given.
    pub output_is_default: bool,
    pub options: Options,
    /// Override the pen colour the extractor found. `None` keeps the real one.
    pub ink: Option<[f32; 3]>,
    pub quiet: bool,
    /// `None` runs everything locally. Set only by an explicit `--via`.
    #[cfg(feature = "api")]
    pub via: Option<ViaConfig>,
}

fn default_output(input: &Path) -> PathBuf {
    let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned());
    let mut out = input.parent().map(PathBuf::from).unwrap_or_default();
    out.push(format!("{}.ink.png", stem.unwrap_or_else(|| "output".into())));
    out
}

/// Parse command-line arguments, excluding the program name.
pub fn parse_args(argv: &[String]) -> Result<Config> {
    if argv.is_empty() {
        return Err(format!("no input image given\n\n{USAGE}").into());
    }

    let mut input: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut mode = Output::Transparent;
    let mut options = Options::default();
    let mut ink: Option<[f32; 3]> = None;
    let mut quiet = false;
    #[cfg(feature = "api")]
    let (mut via, mut model, mut prompt, mut api_key, mut timeout_secs) = (
        None::<inklift_api::Provider>,
        None::<String>,
        None::<String>,
        None::<String>,
        120u64,
    );

    let mut i = 0;
    while i < argv.len() {
        let arg = argv[i].as_str();
        // Every value-taking flag reads the next argument through this, so a
        // trailing flag reports a clear error instead of silently defaulting.
        let mut value = |name: &str| -> Result<String> {
            i += 1;
            argv.get(i)
                .cloned()
                .ok_or_else(|| format!("{name} needs a value").into())
        };
        match arg {
            "-h" | "--help" => return Err(USAGE.into()),
            "-q" | "--quiet" => quiet = true,
            "--white" => mode = Output::White,
            "--both" => mode = Output::Both,
            "-o" | "--output" => output = Some(PathBuf::from(value("--output")?)),
            "--k" => options.sauvola_k = value("--k")?.parse()?,
            "--window" => options.sauvola_radius = value("--window")?.parse()?,
            "--min-area" => options.min_area = value("--min-area")?.parse()?,
            "--feather" => options.feather = value("--feather")?.parse()?,
            "--invert" => options.invert = true,
            "--ink" => ink = Some(inklift_core::parse_ink_color(&value("--ink")?)?),
            "--radius" => options.background_radius = Some(value("--radius")?.parse()?),
            #[cfg(feature = "api")]
            "--via" => via = Some(value("--via")?.parse()?),
            #[cfg(feature = "api")]
            "--model" => model = Some(value("--model")?),
            #[cfg(feature = "api")]
            "--prompt" => prompt = Some(value("--prompt")?),
            #[cfg(feature = "api")]
            "--api-key" => api_key = Some(value("--api-key")?),
            #[cfg(feature = "api")]
            "--timeout" => timeout_secs = value("--timeout")?.parse()?,
            #[cfg(not(feature = "api"))]
            "--via" | "--model" | "--prompt" | "--api-key" | "--timeout" => {
                return Err(format!(
                    "{arg} needs the hosted-model path, which this build does not \
                     include.\nRebuild with: cargo build --release --features inklift-cli/api"
                )
                .into());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n\n{USAGE}").into());
            }
            other if input.is_none() => input = Some(PathBuf::from(other)),
            other => return Err(format!("unexpected extra argument {other}").into()),
        }
        i += 1;
    }

    let input = input.ok_or_else(|| -> Box<dyn std::error::Error> {
        format!("no input image given\n\n{USAGE}").into()
    })?;
    let output_is_default = output.is_none();
    let output = output.unwrap_or_else(|| default_output(&input));

    #[cfg(feature = "api")]
    let via = match via {
        Some(provider) => Some(ViaConfig {
            model: model.unwrap_or_else(|| provider.default_model().to_string()),
            prompt: prompt.unwrap_or_else(|| inklift_api::DEFAULT_PROMPT.to_string()),
            provider,
            api_key,
            timeout_secs,
        }),
        None => {
            // Silently ignoring these would leave the user believing they used
            // a model they never called.
            for (flag, given) in [("--model", model.is_some()), ("--prompt", prompt.is_some())] {
                if given {
                    return Err(format!("{flag} only applies with --via").into());
                }
            }
            None
        }
    };
    Ok(Config {
        input,
        output,
        mode,
        output_is_default,
        options,
        ink,
        quiet,
        #[cfg(feature = "api")]
        via,
    })
}

/// What a run produced, for the caller to report or assert on.
#[derive(Clone, Debug)]
pub struct Report {
    /// Fraction of the page carrying meaningful ink.
    pub coverage: f32,
    /// The estimated pen colour, relative to the paper. The hosted path returns
    /// a finished image rather than an opacity field, so there is none to report.
    pub ink_color: Option<[f32; 3]>,
    pub written: Vec<PathBuf>,
    /// Which path produced this, for the summary line and for the record.
    pub source: String,
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
    let mut out = path.parent().map(PathBuf::from).unwrap_or_default();
    out.push(format!("{}{suffix}", stem.unwrap_or_else(|| "output".into())));
    out
}

/// Load, extract, and write whichever exports the config asks for.
pub fn run(config: &Config) -> Result<Report> {
    #[cfg(feature = "api")]
    if let Some(via) = &config.via {
        return run_via(config, via);
    }
    let planes = load_rgb(&config.input)?;
    let (w, h) = (planes[0].width(), planes[0].height());
    // Silently returning an empty page for a dark-themed source is the most
    // confusing failure this tool has; say so instead.
    if !config.quiet && !config.options.invert && inklift_core::looks_inverted(&planes) {
        eprintln!("note: this image looks light-on-dark; try --invert");
    }
    let mut result = inklift_core::extract(&planes, &config.options);
    if let Some(rgb) = config.ink {
        result = result.with_ink_color(rgb);
    }
    warn_if_invisible(config.ink, config.mode, config.quiet);

    let mut written = Vec::new();
    match config.mode {
        Output::Transparent => {
            save_rgba(&config.output, w, h, &result.to_rgba8())?;
            written.push(config.output.clone());
        }
        Output::White => {
            save_gray_on_white(&config.output, w, h, &result.to_gray_on_white8())?;
            written.push(config.output.clone());
        }
        Output::Both => {
            // Two files, so their names are derived - but from whatever the
            // user actually pointed at.
            let base = if config.output_is_default { &config.input } else { &config.output };
            let clear = with_suffix(base, ".ink.png");
            let white = with_suffix(base, ".white.png");
            save_rgba(&clear, w, h, &result.to_rgba8())?;
            save_gray_on_white(&white, w, h, &result.to_gray_on_white8())?;
            written.push(clear);
            written.push(white);
        }
    }

    Ok(Report {
        coverage: result.coverage(),
        ink_color: Some(result.ink_color()),
        written,
        source: "local".into(),
    })
}

#[cfg(feature = "api")]
fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

#[cfg(feature = "api")]
/// Send the page to a hosted model and write back whatever it returns.
///
/// The reply is a finished image, not an opacity field, so it is written
/// verbatim: re-encoding it here would make the comparison measure our
/// post-processing rather than the model.
fn run_via(config: &Config, via: &ViaConfig) -> Result<Report> {
    let bytes = std::fs::read(&config.input)?;
    // A .env beside the project is a convenience; a real exported variable wins.
    let from_file = read_env_file(Path::new(".env"));
    let key = inklift_api::resolve_key(via.provider, via.api_key.as_deref(), |name| {
        env_or(&from_file, name)
    })?;

    let produced = inklift_api::generate(
        via.provider,
        &via.model,
        &key,
        &bytes,
        mime_for(&config.input),
        &via.prompt,
        config.mode == Output::Transparent,
        std::time::Duration::from_secs(via.timeout_secs),
    )?;

    std::fs::write(&config.output, &produced)?;
    let coverage = load_mask(&config.output, 128, false)
        .map(|m| m.coverage())
        .unwrap_or(0.0);

    Ok(Report {
        coverage,
        ink_color: None,
        written: vec![config.output.clone()],
        source: format!("{}:{}", via.provider.name(), via.model),
    })
}

/// A light pen on a white ground is invisible, and the file looks empty rather
/// than wrong. Say so once, rather than letting it be discovered later.
pub fn warn_if_invisible(ink: Option<[f32; 3]>, mode: Output, quiet: bool) {
    if quiet {
        return;
    }
    let Some(rgb) = ink else { return };
    if matches!(mode, Output::White | Output::Both) && inklift_core::luma(rgb) > 0.72 {
        eprintln!(
            "note: a light ink on a white background will be invisible. \
             Use the transparent output, or a darker --ink."
        );
    }
}
