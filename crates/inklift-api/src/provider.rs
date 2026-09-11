use std::str::FromStr;

/// A hosted image model that can be asked to redraw a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// Google Gemini image models, marketed as Nano Banana.
    Gemini,
    /// OpenAI GPT-Image, the only one of these with a real alpha channel.
    OpenAi,
}

impl Provider {
    pub fn default_model(&self) -> &'static str {
        match self {
            // Nano Banana Pro. `gemini-3.1-flash-image` is the cheaper, faster one.
            Provider::Gemini => "gemini-3-pro-image",
            Provider::OpenAi => "gpt-image-2",
        }
    }

    pub fn key_env(&self) -> &'static str {
        match self {
            Provider::Gemini => "GEMINI_API_KEY",
            Provider::OpenAi => "OPENAI_API_KEY",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Provider::Gemini => "gemini",
            Provider::OpenAi => "openai",
        }
    }
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "gemini" | "google" | "nano-banana" | "nanobanana" => Ok(Provider::Gemini),
            "openai" | "gpt-image" | "gpt" => Ok(Provider::OpenAi),
            other => Err(format!("unknown provider {other:?}; expected gemini or openai")),
        }
    }
}
