//! File I/O and the command-line front end for `inklift-core`.
//!
//! Pixels are handed to the core in the space the file stores them in, with no
//! gamma conversion. Dividing by an estimated background cancels a
//! multiplicative light field in either space, and image viewers composite
//! alpha on sRGB-encoded values, so opacity derived here is correct in the
//! tools that will consume it.

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
pub fn load_rgb(path: &Path) -> Result<[Grid; 3]> {
    let img = image::open(path)?.to_rgb8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut planes = [Grid::new(w, h), Grid::new(w, h), Grid::new(w, h)];
    for (i, px) in img.pixels().enumerate() {
        for c in 0..3 {
            planes[c].data_mut()[i] = px.0[c] as f32 / 255.0;
        }
    }
    Ok(planes)
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
    -q, --quiet           Suppress the summary line
    -h, --help            Show this message

HOSTED MODEL (opt-in, sends the image to a third party):
        --via <NAME>      gemini or openai. Off unless given.
        --model <ID>      Override the provider's default model
        --prompt <TEXT>   Override the instruction sent with the image
        --api-key <KEY>   Override the provider's environment variable
        --timeout <SECS>  Request timeout                          [default: 120]
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Transparent,
    White,
    Both,
}

/// Settings for the opt-in hosted-model path.
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
    pub options: Options,
    pub quiet: bool,
    /// `None` runs everything locally. Set only by an explicit `--via`.
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
    let mut quiet = false;
    let mut via: Option<inklift_api::Provider> = None;
    let mut model: Option<String> = None;
    let mut prompt: Option<String> = None;
    let mut api_key: Option<String> = None;
    let mut timeout_secs = 120u64;

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
            "--radius" => options.background_radius = Some(value("--radius")?.parse()?),
            "--via" => via = Some(value("--via")?.parse()?),
            "--model" => model = Some(value("--model")?),
            "--prompt" => prompt = Some(value("--prompt")?),
            "--api-key" => api_key = Some(value("--api-key")?),
            "--timeout" => timeout_secs = value("--timeout")?.parse()?,
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
    let output = output.unwrap_or_else(|| default_output(&input));

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
    Ok(Config { input, output, mode, options, quiet, via })
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
    if let Some(via) = &config.via {
        return run_via(config, via);
    }
    let planes = load_rgb(&config.input)?;
    let (w, h) = (planes[0].width(), planes[0].height());
    let result = inklift_core::extract(&planes, &config.options);

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
            let clear = with_suffix(&config.input, ".ink.png");
            let white = with_suffix(&config.input, ".white.png");
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

fn mime_for(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

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
