//! Optional hosted-model path, so the local pipeline can be measured against
//! an image model on the same images with the same scorer.
//!
//! Kept in its own crate: `inklift-core` stays dependency-free, and nothing
//! here is reachable unless the user explicitly asks for it.

mod base64;
mod client;
mod provider;
mod wire;

pub use base64::{b64_decode, b64_encode};
pub use client::{generate, key_from_env, resolve_key};
pub use provider::Provider;
pub use wire::{DEFAULT_PROMPT, Request, build_request, interpret, parse_response};
