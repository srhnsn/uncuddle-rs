//! Syntax-based blank-line analysis for Rust.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod analysis;
pub mod config;
pub mod diagnostic;
pub mod discovery;
pub mod fix;
mod parsing;
pub mod runner;
mod source;

pub fn parse(source: &str) -> Result<syn::File, syn::Error> {
    parsing::parse(source, "2024")
}

pub fn parse_with_edition(source: &str, edition: &str) -> Result<syn::File, syn::Error> {
    parsing::parse(source, edition)
}
