//! Defines the `ZenpatchError` enum for patch parsing and application errors.
//!
//! This enum lists all potential issues that can arise during the processing
//! of patch files, such as invalid formats, file system errors, or context mismatches.
//! It provides detailed variants to pinpoint the source of the error.
//! Corresponds to the TypeScript `DiffError` type.

#[derive(Debug, PartialEq)]
pub enum ZenpatchError {
    InvalidPatchFormat(std::string::String),
    FileNotFound(std::string::String),
    DuplicatePath(std::string::String),
    MissingFile(std::string::String),
    FileExists(std::string::String),
    InvalidLine(std::string::String),
    InvalidContext(usize, std::string::String), // index, context text
    InvalidEOFContext(usize, std::string::String), // index, context text
    IndexOutOfBounds(std::string::String), // General index error message
    IoError(std::string::String), // Wrap std::io::Error messages
    PatchConflict(std::string::String), // Conflict between patch and file content
    ContextNotFound(std::string::String), // Context lines not found in the file
    AmbiguousPatch(std::string::String), // Patch context matches in multiple valid, non-overlapping ways
    AnyhowError(String),
    PatchApplicationFailed(String),
}

impl std::fmt::Display for ZenpatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZenpatchError::InvalidPatchFormat(msg) => write!(f, "Invalid patch format: {}", msg),
            ZenpatchError::FileNotFound(path) => write!(f, "File not found: {}", path),
            ZenpatchError::DuplicatePath(path) => write!(f, "Duplicate path in patch: {}", path),
            ZenpatchError::MissingFile(path) => write!(f, "Missing file mentioned in patch: {}", path),
            ZenpatchError::FileExists(path) => write!(f, "File already exists: {}", path),
            ZenpatchError::InvalidLine(line) => write!(f, "Invalid line in patch: {}", line),
            ZenpatchError::InvalidContext(idx, ctx) => write!(f, "Invalid context at index {}: {}", idx, ctx),
            ZenpatchError::InvalidEOFContext(idx, ctx) => write!(f, "Invalid end-of-file context at index {}: {}", idx, ctx),
            ZenpatchError::IndexOutOfBounds(msg) => write!(f, "Index out of bounds: {}", msg),
            ZenpatchError::IoError(msg) => write!(f, "I/O error: {}", msg),
            ZenpatchError::PatchConflict(msg) => write!(f, "Patch conflict: {}", msg),
            ZenpatchError::ContextNotFound(msg) => write!(f, "Context not found: {}", msg),
            ZenpatchError::AmbiguousPatch(msg) => write!(f, "Ambiguous patch: {}", msg),
            ZenpatchError::AnyhowError(msg) =>write!(f, "Anyhow error: {}", msg),
            ZenpatchError::PatchApplicationFailed(msg) => write!(f, "Patch application: {}", msg),
        }
    }
}

impl std::error::Error for ZenpatchError {}
