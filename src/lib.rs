//! A crate for applying text-based patches.
//!
//! This crate provides a single primary function, `apply`, which takes a patch
//! and the original content as string slices and returns the patched content.
//! It is designed for simplicity and robustness, especially for use by AI agents.

pub mod apply;
pub mod applier;
pub mod data;
pub mod error;
pub mod parser;

pub use apply::apply;
pub use error::ZenpatchError;

#[cfg(test)]
pub mod tests;
