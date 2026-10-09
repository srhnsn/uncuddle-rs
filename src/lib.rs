//! Syntax-based blank-line analysis for Rust.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod analysis;
pub mod config;
pub mod diagnostic;
pub mod discovery;

pub fn parse(source: &str) -> Result<syn::File, syn::Error> {
    syn::parse_file(source)
}
