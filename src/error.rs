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

#[cfg(test)]
mod tests {
    use super::ZenpatchError;

    #[test]
    fn test_display_invalid_patch_format() {
        let e = ZenpatchError::InvalidPatchFormat("bad".into());
        assert_eq!(e.to_string(), "Invalid patch format: bad");
    }

    #[test]
    fn test_display_file_not_found() {
        let e = ZenpatchError::FileNotFound("missing.txt".into());
        assert_eq!(e.to_string(), "File not found: missing.txt");
    }

    #[test]
    fn test_display_duplicate_path() {
        let e = ZenpatchError::DuplicatePath("dup.txt".into());
        assert_eq!(e.to_string(), "Duplicate path in patch: dup.txt");
    }

    #[test]
    fn test_display_missing_file() {
        let e = ZenpatchError::MissingFile("gone.txt".into());
        assert_eq!(e.to_string(), "Missing file mentioned in patch: gone.txt");
    }

    #[test]
    fn test_display_file_exists() {
        let e = ZenpatchError::FileExists("exists.txt".into());
        assert_eq!(e.to_string(), "File already exists: exists.txt");
    }

    #[test]
    fn test_display_invalid_line() {
        let e = ZenpatchError::InvalidLine("???".into());
        assert_eq!(e.to_string(), "Invalid line in patch: ???");
    }

    #[test]
    fn test_display_invalid_context() {
        let e = ZenpatchError::InvalidContext(5, "ctx".into());
        assert_eq!(e.to_string(), "Invalid context at index 5: ctx");
    }

    #[test]
    fn test_display_invalid_eof_context() {
        let e = ZenpatchError::InvalidEOFContext(10, "eof".into());
        assert_eq!(e.to_string(), "Invalid end-of-file context at index 10: eof");
    }

    #[test]
    fn test_display_index_out_of_bounds() {
        let e = ZenpatchError::IndexOutOfBounds("oob".into());
        assert_eq!(e.to_string(), "Index out of bounds: oob");
    }

    #[test]
    fn test_display_io_error() {
        let e = ZenpatchError::IoError("disk full".into());
        assert_eq!(e.to_string(), "I/O error: disk full");
    }

    #[test]
    fn test_display_patch_conflict() {
        let e = ZenpatchError::PatchConflict("mismatch".into());
        assert_eq!(e.to_string(), "Patch conflict: mismatch");
    }

    #[test]
    fn test_display_context_not_found() {
        let e = ZenpatchError::ContextNotFound("missing ctx".into());
        assert_eq!(e.to_string(), "Context not found: missing ctx");
    }

    #[test]
    fn test_display_ambiguous_patch() {
        let e = ZenpatchError::AmbiguousPatch("multi-match".into());
        assert_eq!(e.to_string(), "Ambiguous patch: multi-match");
    }

    #[test]
    fn test_display_anyhow_error() {
        let e = ZenpatchError::AnyhowError("wrapped".into());
        assert_eq!(e.to_string(), "Anyhow error: wrapped");
    }

    #[test]
    fn test_display_patch_application_failed() {
        let e = ZenpatchError::PatchApplicationFailed("failed".into());
        assert_eq!(e.to_string(), "Patch application: failed");
    }
}
