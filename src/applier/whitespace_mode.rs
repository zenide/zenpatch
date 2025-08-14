//! Defines the `WhitespaceMode` enum for controlling patch matching sensitivity.
//!
//! This enum is used by the backtracking patcher to specify how strictly
//! whitespace should be handled when comparing lines.

/// Controls whitespace sensitivity when matching patch context and deletions.
#[derive(Clone, Copy, Debug)]
pub enum WhitespaceMode {
    /// Exact matching, preserving all whitespace (no normalization).
    Strict,
    /// Lenient matching: trims leading/trailing whitespace and collapses internal whitespace runs to single spaces before comparing.
    Lenient,
    /// SuperLenient matching: Lenient plus normalizes special characters like quotes and dashes.
    SuperLenient,
}