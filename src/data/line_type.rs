//! Defines the type of line within a patch hunk (Context, Deletion, Insertion).
//!
//! This enum is used by the `Chunk` struct to represent the structure
//! of changes within a file update, including context lines necessary for application.
//! Adheres to the one-item-per-file rule.

/// Represents the type of a line within a patch hunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LineType {
    /// A context line, unchanged between versions (starts with ' ').
    Context,
    /// A line deleted from the original file (starts with '-').
    Deletion,
    /// A line inserted into the new file (starts with '+').
    Insertion,
}

#[cfg(test)]
mod tests {
    // Use fully qualified paths as required by guidelines.

    #[test]
    fn test_line_type_variants() {
        // Test basic enum variant accessibility and equality.
        let context = super::LineType::Context;
        let deletion = super::LineType::Deletion;
        let insertion = super::LineType::Insertion;

        std::assert_eq!(context, super::LineType::Context);
        std::assert_eq!(deletion, super::LineType::Deletion);
        std::assert_eq!(insertion, super::LineType::Insertion);
        std::assert_ne!(context, deletion);
        std::assert_ne!(context, insertion);
        std::assert_ne!(deletion, insertion);
    }

    #[test]
    fn test_line_type_copy_clone() {
        // Test that the enum derives Copy and Clone.
        let context1 = super::LineType::Context;
        let context2 = context1; // Copy
        let context3 = context1.clone(); // Clone

        std::assert_eq!(context1, context2);
        std::assert_eq!(context1, context3);
    }
}
